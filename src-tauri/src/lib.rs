mod cache;
mod commands;
mod db;
mod diagnostics;
mod models;
mod sources;
mod stream_proxy;

use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use rusqlite::Connection;
use sources::bika::BikaApi;
use sources::video::VideoApi;
use std::{
    future::Future,
    sync::{Mutex, MutexGuard, PoisonError, RwLock},
    time::{Duration, Instant},
};
use tauri::{ipc::Response, Manager};

/// niacg 漫画源地址
pub(crate) const BASE: &str = "https://www.niacg.com";
/// niacg 限制两次搜索至少间隔 5 秒,违反时返回无结果的提示页
const NIACG_SEARCH_INTERVAL: Duration = Duration::from_millis(5200);
/// niacg 请求超时:无超时时一次挂起的搜索会一直占住搜索闸,后续搜索全部卡死
const NIACG_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const NIACG_TIMEOUT: Duration = Duration::from_secs(30);

/// 取互斥锁;某次持锁 panic 造成的中毒不应让后续所有 command 永久失败,
/// 受保护数据(SQLite 连接/token)在 panic 后仍可用,直接取回内部值
pub(crate) fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// 网络错误统一映射为简短中文提示(前端直接展示该字符串)
fn net_error(
    timeout: bool,
    connect: bool,
    status: Option<u16>,
    raw: &dyn std::fmt::Display,
) -> String {
    if timeout {
        "请求超时,请检查网络后重试".into()
    } else if connect {
        "无法连接服务器,请检查网络后重试".into()
    } else if let Some(code) = status {
        format!("服务器返回错误 (HTTP {code})")
    } else {
        format!("网络请求失败: {raw}")
    }
}

pub(crate) fn reqwest_error(e: reqwest::Error) -> String {
    net_error(
        e.is_timeout(),
        e.is_connect(),
        e.status().map(|s| s.as_u16()),
        &e,
    )
}

pub(crate) fn wreq_error(e: wreq::Error) -> String {
    net_error(
        e.is_timeout(),
        e.is_connect(),
        e.status().map(|s| s.as_u16()),
        &e,
    )
}

struct AppState {
    niacg: reqwest::Client,
    video: VideoApi,
    /// 哔咔客户端在切换分流地址时整体重建,读锁短持有,clone 后异步使用
    bika: RwLock<BikaApi>,
    bika_token: Mutex<Option<String>>,
    /// 串行化 401 重登:并发请求同时 401 时只真正登录一次
    bika_relogin_gate: tokio::sync::Mutex<()>,
    /// SQLite 连接;一律经 `with_db` 访问,不跨 await 持锁
    db: Mutex<Connection>,
    /// 300 秒 TTL 进程内缓存(容量有界 + 相同请求并发合并)
    cache: cache::TtlCache,
    /// niacg 搜索全局限速:串行化并保证间隔,避免触发 5 秒限制拿到空结果。
    /// 值为上一次搜索放行时刻,None 表示本进程尚未搜索过
    niacg_search_gate: tokio::sync::Mutex<Option<Instant>>,
}

impl AppState {
    /// SQLite 连接互斥锁的统一入口(锁仅在闭包内持有)
    fn with_db<T>(&self, f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
        f(&lock(&self.db))
    }

    /// 取当前哔咔客户端快照(clone 便宜,reqwest::Client 内部为 Arc)
    fn bika_snapshot(&self) -> BikaApi {
        self.bika
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// 获取下一次 niacg 搜索的放行权,必要时等待。
    /// 返回的 guard 由调用方持有到本次搜索请求结束(tokio 锁 FIFO 公平,
    /// 跨 await 持有即串行化语义),间隔从上一次放行时刻起算
    async fn niacg_search_turn(&self) -> tokio::sync::MutexGuard<'_, Option<Instant>> {
        let mut last = self.niacg_search_gate.lock().await;
        if let Some(prev) = *last {
            let elapsed = prev.elapsed();
            if elapsed < NIACG_SEARCH_INTERVAL {
                tokio::time::sleep(NIACG_SEARCH_INTERVAL - elapsed).await;
            }
        }
        *last = Some(Instant::now());
        last
    }

    async fn niacg_get(&self, path: &str) -> Result<String, String> {
        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{BASE}{path}")
        };
        self.niacg
            .get(url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(reqwest_error)?
            .text()
            .await
            .map_err(reqwest_error)
    }

    /// 优先内存,其次数据库恢复哔咔 token
    fn bika_token(&self) -> Result<Option<String>, String> {
        if let Some(token) = lock(&self.bika_token).clone() {
            return Ok(Some(token));
        }
        let saved = self.with_db(|c| Ok(db::bika_account(c)?.map(|account| account.token)))?;
        if let Some(token) = &saved {
            *lock(&self.bika_token) = Some(token.clone());
        }
        Ok(saved)
    }

    /// 用数据库中的凭据重新登录并更新 token。`stale` 为刚被 401 拒绝的 token:
    /// 排队期间若已有其他请求完成重登(内存 token 已不同),直接复用新 token
    async fn bika_relogin(&self, stale: &str) -> Result<String, String> {
        let _gate = self.bika_relogin_gate.lock().await;
        if let Some(current) = lock(&self.bika_token).clone() {
            if current != stale {
                return Ok(current);
            }
        }
        let (email, password) = self.with_db(|c| {
            let account = db::bika_account(c)?.ok_or("哔咔登录已失效,请重新登录")?;
            Ok((account.email, account.password))
        })?;
        let token = self.bika_snapshot().sign_in(&email, &password).await?;
        self.with_db(|c| db::save_bika_account(c, &email, &password, &token))?;
        *lock(&self.bika_token) = Some(token.clone());
        Ok(token)
    }

    /// 执行哔咔 API 调用,401 时自动重登一次
    async fn bika_call<T, F, Fut>(&self, call: F) -> Result<T, String>
    where
        F: Fn(String) -> Fut,
        Fut: Future<Output = Result<T, String>>,
    {
        let token = self
            .bika_token()?
            .ok_or("哔咔未登录,请在设置中登录哔咔账号")?;
        match call(token.clone()).await {
            Err(e) if e.starts_with("401") => {
                let token = self.bika_relogin(&token).await?;
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
        .and_then(reqwest::Response::error_for_status)
        .map_err(reqwest_error)?
        .bytes()
        .await
        .map_err(reqwest_error)?;
    // Bytes 独占时转 Vec 零拷贝
    Ok(Response::new(Vec::<u8>::from(bytes)))
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
        .and_then(wreq::Response::error_for_status)
        .map_err(wreq_error)?
        .bytes()
        .await
        .map_err(wreq_error)?;
    Ok(Response::new(Vec::<u8>::from(bytes)))
}

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
                .connect_timeout(NIACG_CONNECT_TIMEOUT)
                .timeout(NIACG_TIMEOUT)
                .build()?;
            let video = VideoApi::new()?;
            let conn = db::open(&dir.join("ygqd.db")).map_err(std::io::Error::other)?;
            let address = db::bika_address(&conn).unwrap_or_default();
            let bika = BikaApi::new(if address.is_empty() {
                None
            } else {
                Some(&address)
            })?;
            let proxy_port = stream_proxy::spawn()?;
            stream_proxy::set_app(app.handle().clone());
            diagnostics::diag(app.handle(), &format!("app setup done, proxy port={proxy_port}"));
            app.manage(AppState {
                niacg,
                video,
                bika: RwLock::new(bika),
                bika_token: Mutex::new(None),
                bika_relogin_gate: tokio::sync::Mutex::new(()),
                db: Mutex::new(conn),
                cache: cache::TtlCache::new(cache::CACHE_TTL, cache::CACHE_CAPACITY),
                niacg_search_gate: tokio::sync::Mutex::new(None),
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
            commands::comic::fetch_image,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn net_error_messages_are_concise() {
        assert_eq!(
            net_error(true, false, None, &"x"),
            "请求超时,请检查网络后重试"
        );
        assert_eq!(
            net_error(false, true, None, &"x"),
            "无法连接服务器,请检查网络后重试"
        );
        // 前端按 '403' 子串识别上游拒绝,状态码须保留在文案中
        assert!(net_error(false, false, Some(403), &"x").contains("403"));
        assert_eq!(net_error(false, false, None, &"boom"), "网络请求失败: boom");
    }

    #[test]
    fn lock_recovers_from_poison() {
        let m = std::sync::Arc::new(Mutex::new(1));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("poison");
        })
        .join();
        assert!(m.is_poisoned());
        assert_eq!(*lock(&m), 1);
    }
}
