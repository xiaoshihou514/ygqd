mod db;
mod gateway;
mod models;
mod sources;
mod stream_proxy;

use gateway::GatewayManager;
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
#[cfg(debug_assertions)]
fn spawn_novel_selftest(app: AppHandle) {
    // 临时侦察:YGQD_TMP_FETCH=空格分隔的多 URL,逐个经网关抓取落盘
    // (取证完成后移除)
    if let Ok(urls) = std::env::var("YGQD_TMP_FETCH") {
        let probe_app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(10));
            tauri::async_runtime::block_on(async {
                let manager = probe_app.state::<GatewayManager>();
                let manager_ref = manager.inner();
                for (i, url) in urls.split(' ').enumerate() {
                    let r = gateway::gateway_fetch(&probe_app, manager_ref, "hlib", url).await;
                    let body = r.unwrap_or_else(|e| format!("ERR {e}"));
                    if let Some(dir) = gateway::diag_dir() {
                        let _ = std::fs::write(dir.join(format!("recon_{i}.html")), &body);
                    }
                    gateway::diag(
                        &probe_app,
                        &format!("[临时抓取] {i} url={url} bytes={}", body.len()),
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(6)).await;
                }
                gateway::diag(&probe_app, "[临时抓取] 完成");
            });
        });
        return;
    }
    let Ok(keyword) = std::env::var("YGQD_SELFTEST_NOVEL") else {
        return;
    };
    tauri::async_runtime::spawn(async move {
        let log = |msg: String| gateway::diag(&app, &format!("[自检] {msg}"));
        tokio::time::sleep(Duration::from_secs(3)).await;
        let started = Instant::now();
        let hits = match sources::novel::search_series(&app, &keyword).await {
            Ok(hits) => hits,
            Err(e) => return log(format!("novel_search FAIL: {e}")),
        };
        log(format!(
            "novel_search OK 条数={} 作者非空={} 带标签={} 耗时={}s",
            hits.len(),
            hits.iter().filter(|h| !h.author.is_empty()).count(),
            hits.iter().filter(|h| !h.tags.is_empty()).count(),
            started.elapsed().as_secs()
        ));
        let Some(first) = hits.first() else { return };
        let series = match sources::novel::fetch_series(&app, &first.id).await {
            Ok(series) => series,
            Err(e) => return log(format!("fetch_series FAIL id={}: {e}", first.id)),
        };
        log(format!(
            "fetch_series OK id={} 章节数={} 标题字符数={} 作者非空={} 简介非空={} cover空={}",
            series.id,
            series.chapters.len(),
            series.title.chars().count(),
            !series.author.is_empty(),
            !series.introduction.is_empty(),
            series.cover.is_empty()
        ));
        let Some(chapter) = series.chapters.first() else {
            return;
        };
        // 站点对连续抓取敏感,章节请求前留间隔
        tokio::time::sleep(Duration::from_secs(10)).await;
        match sources::novel::fetch_article_page(&app, &chapter.id, 1).await {
            Ok(page) => log(format!(
                "novel_read OK id={} 正文字符数={} 总页数={} 标题非空={}",
                chapter.id,
                page.text.chars().count(),
                page.total_pages,
                !page.title.is_empty()
            )),
            Err(e) => log(format!("novel_read FAIL id={}: {e}", chapter.id)),
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
            gateway::init_diag_dir(dir.clone());
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
            })?;
            app.manage(GatewayManager::new());
            let proxy_port = stream_proxy::spawn()?;
            stream_proxy::set_app(app.handle().clone());
            gateway::diag(app.handle(), &format!("app setup done, proxy port={proxy_port}"));
            #[cfg(debug_assertions)]
            spawn_novel_selftest(app.handle().clone());
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
            commands::gateway::gateway_fetch,
            commands::gateway::gateway_open,
            commands::gateway::gateway_set_visible,
            commands::gateway::gateway_finish,
            commands::gateway::diag_read,
            commands::gateway::diag_clear,
            commands::gateway::gateway_result,
            commands::settings::record_item_history,
            commands::settings::fetch_item_history,
            commands::video::video_follow,
            commands::video::video_unfollow,
            commands::video::video_follow_list,
            commands::video::video_follow_targets,
            commands::video::video_follow_works,
            commands::novel::novel_blacklist_list,
            commands::novel::novel_blacklist_add,
            commands::novel::novel_blacklist_remove,
            commands::novel::novel_recommend,
            commands::novel::novel_ranking,
            commands::novel::novel_followed_author_works,
            commands::novel::novel_followed_authors,
            commands::novel::novel_series_info,
            commands::novel::novel_add,
            commands::novel::novel_search,
            commands::novel::novel_search_page,
            commands::novel::novel_latest,
            commands::novel::novel_shelf,
            commands::novel::novel_remove,
            commands::novel::novel_read,
            commands::novel::novel_prefetch,
            commands::novel::novel_progress,
            commands::novel::novel_set_progress,
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
