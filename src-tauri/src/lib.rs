mod db;
mod diagnostics;
mod models;
mod sources;
mod stream_proxy;

use models::Source;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use rusqlite::Connection;
use sources::bika::BikaApi;
use sources::video::VideoApi;
use std::{
    collections::HashMap,
    future::Future,
    sync::{Mutex, RwLock},
    time::{Duration, Instant},
};
use tauri::{ipc::Response, AppHandle, Manager, State};

const BASE: &str = "https://www.niacg.com";
/// niacg 限制两次搜索至少间隔 5 秒,违反时返回无结果的提示页
const NIACG_SEARCH_INTERVAL: Duration = Duration::from_millis(5200);
mod commands;

struct AppState {
    niacg: reqwest::Client,
    video: VideoApi,
    proxy_port: u16,
    /// 哔咔客户端在切换分流地址时整体重建,读锁短持有,clone 后异步使用
    bika: RwLock<BikaApi>,
    bika_token: Mutex<Option<String>>,
    db: Mutex<Connection>,
    cache: Mutex<HashMap<String, (Instant, serde_json::Value)>>,
    /// niacg 搜索全局限速:串行化并保证间隔,避免触发 5 秒限制拿到空结果
    niacg_search_gate: tokio::sync::Mutex<Instant>,
    /// hlib 抓取全局限速:shared.rs 的 RateGate 统一封装(对齐站点「连续抓取敏感」)
    hlib_gate: crate::sources::shared::RateGate,
}
impl AppState {
    /// 连接互斥锁的短路封装(P2 样板收敛:30+ 处 command 的
    /// `state.db.lock().map_err(|e| e.to_string())?` 收敛为一行)
    /// 渐进迁移期:存量 30+ 处尚未全部接入,标注 dead_code 消除告警
    #[allow(dead_code)]
    fn with_db<T>(
        &self,
        f: impl FnOnce(&rusqlite::Connection) -> Result<T, String>,
    ) -> Result<T, String> {
        let conn = self.db.lock().map_err(|e| e.to_string())?;
        f(&conn)
    }

    /// 取当前哔咔客户端快照(clone 便宜,reqwest::Client 内部为 Arc)
    fn bika_snapshot(&self) -> BikaApi {
        self.bika.read().expect("bika client lock poisoned").clone()
    }
    /// 获取下一次 niacg 搜索的放行权,必要时等待
    async fn niacg_search_turn(&self) -> tokio::sync::MutexGuard<'_, Instant> {
        let mut last = self.niacg_search_gate.lock().await;
        let elapsed = last.elapsed();
        if elapsed < NIACG_SEARCH_INTERVAL {
            tokio::time::sleep(NIACG_SEARCH_INTERVAL - elapsed).await;
        }
        *last = Instant::now();
        last
    }
    async fn niacg_get(&self, path: &str) -> Result<String, String> {
        self.niacg
            .get(if path.starts_with("http") {
                path.to_string()
            } else {
                format!("{BASE}{path}")
            })
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())
    }
    fn cached<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let cache = self.cache.lock().ok()?;
        let (at, v) = cache.get(key)?;
        if at.elapsed() > Duration::from_secs(300) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }
    /// TTL 参数化的缓存读取(批次 2 访问预算:手动刷新数据 30 分钟)
    fn cached_ttl<T: serde::de::DeserializeOwned>(&self, key: &str, ttl: Duration) -> Option<T> {
        let cache = self.cache.lock().ok()?;
        let (at, v) = cache.get(key)?;
        if at.elapsed() > ttl {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }

    /// hlib 抓取统一闸:串行化并保证间隔(对齐站点反爬敏感度)。
    /// 与 niacg_search_gate 同架构;锁跨 await 持有(串行化语义)。
    async fn hlib_throttle(&self) {
        self.hlib_gate.throttle().await;
    }
    fn put<T: serde::Serialize>(&self, key: String, value: &T) {
        if let Ok(v) = serde_json::to_value(value) {
            if let Ok(mut c) = self.cache.lock() {
                c.insert(key, (Instant::now(), v));
            }
        }
    }
    fn clear_cache(&self) {
        if let Ok(mut c) = self.cache.lock() {
            c.clear();
        }
    }
    /// 优先内存,其次数据库恢复哔咔 token
    async fn bika_token(&self) -> Result<Option<String>, String> {
        if let Some(token) = self.bika_token.lock().ok().and_then(|t| t.clone()) {
            return Ok(Some(token));
        }
        let conn = self.db.lock().map_err(|e| e.to_string())?;
        let saved = db::bika_account(&conn)?.map(|account| account.token);
        drop(conn);
        if let Some(token) = saved.clone() {
            *self.bika_token.lock().map_err(|e| e.to_string())? = Some(token);
        }
        Ok(saved)
    }
    /// 用数据库中的凭据重新登录并更新 token
    async fn bika_relogin(&self) -> Result<String, String> {
        let (email, password) = {
            let conn = self.db.lock().map_err(|e| e.to_string())?;
            let account = db::bika_account(&conn)?.ok_or("哔咔登录已失效,请重新登录")?;
            (account.email, account.password)
        };
        let token = self.bika_snapshot().sign_in(&email, &password).await?;
        {
            let conn = self.db.lock().map_err(|e| e.to_string())?;
            db::save_bika_account(&conn, &email, &password, &token)?;
        }
        *self.bika_token.lock().map_err(|e| e.to_string())? = Some(token.clone());
        Ok(token)
    }
    /// 执行哔咔 API 调用,401 时自动重登一次
    async fn bika_call<T, F, Fut>(&self, call: F) -> Result<T, String>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        let token = self
            .bika_token()
            .await?
            .ok_or("哔咔未登录,请在设置中登录哔咔账号")?;
        match call(token.clone()).await {
            Err(e) if e.starts_with("401") => {
                let token = self.bika_relogin().await?;
                call(token).await
            }
            other => other,
        }
    }
}
// ── 通用图片代理(漫画/视频共用) ──

pub(crate) async fn fetch_image_with_client(
    client: reqwest::Client,
    url: String,
) -> Result<Response, String> {
    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    Ok(Response::new(bytes.to_vec()))
}

/// wreq 版图片抓取(与 reqwest 版同构):供 fourhoi 图床使用——
/// missav 客户端默认无 Referer,绕过其防盗链白名单
pub(crate) async fn fetch_image_with_wreq(
    client: wreq::Client,
    url: String,
) -> Result<Response, String> {
    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .bytes()
        .await
        .map_err(|e| e.to_string())?;
    Ok(Response::new(bytes.to_vec()))
}

#[tauri::command]
async fn fetch_image(
    source: Source,
    url: String,
    state: State<'_, AppState>,
) -> Result<Response, String> {
    let client = match source {
        Source::Bika => state.bika_snapshot().client,
        Source::Niacg => state.niacg.clone(),
    };
    fetch_image_with_client(client, url).await
}

/// 小说链路无头自检(仅 debug 构建,环境变量 YGQD_SELFTEST_NOVEL=关键词 门控):
/// 搜索 → 首条系列目录 → 首章第 1 页,diag 只记条数/字节/字段非空数,
/// 不落正文内容。用于无界面端到端验证网关 + 会话 cookie 是否可用。

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            diagnostics::init_diag_dir(dir.clone());
            let mut headers = HeaderMap::new();
            headers.insert(
                USER_AGENT,
                HeaderValue::from_static(
                    "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36",
                ),
            );
            headers.insert(REFERER, HeaderValue::from_static(BASE));
            let niacg = reqwest::Client::builder()
                .default_headers(headers)
                .cookie_store(true)
                .build()?;
            let video = VideoApi::new()?;
            let conn = db::open(&dir.join("ygqd.db")).map_err(std::io::Error::other)?;
            let address = db::bika_address(&conn).unwrap_or_default();
            let bika = BikaApi::new(if address.is_empty() {
                None
            } else {
                Some(&address)
            })?;;
            let proxy_port = stream_proxy::spawn()?;
            stream_proxy::set_app(app.handle().clone());
            diagnostics::diag(app.handle(), &format!("app setup done, proxy port={proxy_port}"));
            app.manage(AppState {
                niacg,
                video,
                proxy_port,
                bika: RwLock::new(bika),
                bika_token: Mutex::new(None),
                db: Mutex::new(conn),
                cache: Mutex::new(HashMap::new()),
                niacg_search_gate: tokio::sync::Mutex::new(Instant::now() - Duration::from_secs(3600)),
            hlib_gate: crate::sources::shared::RateGate::new(std::time::Duration::from_secs(3)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::comic::fetch_category,
            commands::comic::fetch_bika_categories,
            commands::comic::fetch_bika_leaderboard,
            commands::video::fetch_video_list,
            commands::video::search_videos,
            commands::video::fetch_video_detail,
            commands::gateway::diag_read,
            commands::gateway::diag_clear,
            commands::settings::record_item_history,
            commands::settings::fetch_item_history,
            commands::video::video_follow,
            commands::video::video_unfollow,
            commands::video::video_follow_list,
            commands::video::video_follow_targets,
            commands::video::video_follow_works,
            commands::comic::bika_switch_like,
            commands::comic::bika_switch_favourite,
            commands::comic::fetch_favorites,
            commands::video::record_video_watched,
            commands::video::fetch_video_watched_ids,
            commands::comic::fetch_comic_viewed_ids,
            commands::comic::is_favorite,
            commands::comic::add_favorite,
            commands::comic::remove_favorite,
            commands::comic::sync_bika_favorites,
            commands::comic::search_comics,
            commands::comic::fetch_comic,
            commands::comic::fetch_comic_images,
            fetch_image,
            commands::video::fetch_video_image,
            commands::comic::bika_login,
            commands::comic::bika_logout,
            commands::comic::bika_status,
            commands::comic::fetch_bika_addresses,
            commands::comic::get_bika_address,
            commands::comic::set_bika_address,
            commands::settings::fetch_follows,
            commands::settings::follow_author,
            commands::settings::unfollow_author,
            commands::settings::fetch_history,
            commands::settings::record_history,
            commands::settings::fetch_blacklist,
            commands::settings::add_blacklist,
            commands::settings::remove_blacklist,
            commands::settings::update_blacklist
        ])
        .run(tauri::generate_context!())
        .expect("failed to run app")
}
