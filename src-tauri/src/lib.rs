mod db;
mod gateway;
mod models;
mod sources;
mod video_proxy;

use gateway::GatewayManager;
use models::NovelPage;
use models::*;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use rusqlite::Connection;
use sources::bika::BikaApi;
use sources::video::{VideoApi, VideoDetail, VideoListResult};
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
struct AppState {
    niacg: reqwest::Client,
    video: VideoApi,
    gateway: GatewayManager,
    /// 哔咔客户端在切换分流地址时整体重建,读锁短持有,clone 后异步使用
    bika: RwLock<BikaApi>,
    bika_token: Mutex<Option<String>>,
    db: Mutex<Connection>,
    cache: Mutex<HashMap<String, (Instant, serde_json::Value)>>,
    /// niacg 搜索全局限速:串行化并保证间隔,避免触发 5 秒限制拿到空结果
    niacg_search_gate: tokio::sync::Mutex<Instant>,
}
impl AppState {
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
#[tauri::command]
async fn fetch_category(
    source: Source,
    category: String,
    page: i32,
    state: State<'_, AppState>,
) -> Result<CategoryListResult, String> {
    let key = format!("{source}:list-{category}-{page}");
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let out = match source {
        Source::Bika => {
            let bika = state.bika_snapshot();
            state
                .bika_call(|token| bika.comics(token, &category, "", "", page + 1))
                .await?
        }
        Source::Niacg => sources::niacg::list(
            &state
                .niacg_get(&format!("/listinfo-{category}-{page}.html"))
                .await?,
        ),
    };
    state.put(key, &out);
    Ok(out)
}

#[tauri::command]
async fn fetch_bika_categories(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let key = "bika:categories";
    if let Some(v) = state.cached(key) {
        return Ok(v);
    }
    let out = {
        let bika = state.bika_snapshot();
        state.bika_call(|token| bika.categories(token)).await?
    };
    state.put(key.into(), &out);
    Ok(out)
}

/// 通过 WebView 网关抓取页面/接口(用于 CF 严防站点: hlib/missav)。
/// 首次调用会创建可见网关窗口,若遇人机挑战需用户点一次。
#[tauri::command]
async fn gateway_fetch(
    app: AppHandle,
    state: State<'_, AppState>,
    label: String,
    url: String,
) -> Result<String, String> {
    gateway::gateway_fetch(&app, &state.gateway, &label, &url).await
}

/// 显示/隐藏网关窗口(挑战已过后隐藏)
#[tauri::command]
fn gateway_set_visible(app: AppHandle, label: String, visible: bool) -> Result<(), String> {
    let window = app
        .get_webview_window(&format!("gateway_{label}"))
        .ok_or("网关窗口不存在")?;
    if visible {
        window.show().map_err(|e| e.to_string())
    } else {
        window.hide().map_err(|e| e.to_string())
    }
}

/// 添加小说系列到书架:输入 hlib 系列链接或纯 ID。
/// 首次会弹出网关窗口,需用户完成一次登录/安全验证。
#[tauri::command]
async fn novel_add(
    app: AppHandle,
    state: State<'_, AppState>,
    input: String,
) -> Result<NovelSeries, String> {
    let id = sources::novel::parse_series_id(&input)?;
    let mut series = sources::novel::fetch_series(&app, &id).await?;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::novel_upsert(&conn, &series)?;
        series.added_at = db::now();
    }
    Ok(series)
}

#[tauri::command]
fn novel_shelf(state: State<'_, AppState>) -> Result<Vec<NovelSeries>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::novel_shelf(&conn)
}

#[tauri::command]
fn novel_remove(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::novel_remove(&conn, &id)
}

/// 读取章节某页正文;网关直连 hlib,未登录/未验证时返回引导错误
#[tauri::command]
async fn novel_read(app: AppHandle, chapter_id: String, page: i64) -> Result<NovelPage, String> {
    sources::novel::fetch_article_page(&app, &chapter_id, page).await
}

#[tauri::command]
fn novel_progress(
    series_id: String,
    state: State<'_, AppState>,
) -> Result<Option<(i64, i64)>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::novel_progress(&conn, &series_id)
}

#[tauri::command]
fn novel_set_progress(
    series_id: String,
    chapter_order: i64,
    page: i64,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::novel_set_progress(&conn, &series_id, chapter_order, page)
}

/// 网关桥回传(由网关 webview 内的 JS 调用,不暴露给主窗口前端)
#[tauri::command]
fn gateway_result(state: State<'_, AppState>, result: gateway::GatewayReply) {
    state.gateway.resolve(result);
}

/// 视频列表(fs1 最新上市),page 从 1 开始
#[tauri::command]
async fn fetch_video_list(
    page: i32,
    state: State<'_, AppState>,
) -> Result<VideoListResult, String> {
    let key = format!("video:list-{page}");
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let out = state.video.list(page).await?;
    state.put(key, &out);
    Ok(out)
}

/// 视频搜索(番号/标题),page 从 1 开始
#[tauri::command]
async fn search_videos(
    keyword: String,
    page: i32,
    state: State<'_, AppState>,
) -> Result<VideoListResult, String> {
    let key = format!("video:search-{keyword}-{page}");
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let out = state.video.search(&keyword, page).await?;
    state.put(key, &out);
    Ok(out)
}

/// 视频详情(元数据走 fs1,m3u8 优先 missav 网关,回退 fs1)
#[tauri::command]
async fn fetch_video_detail(
    app: AppHandle,
    state: State<'_, AppState>,
    code: String,
) -> Result<VideoDetail, String> {
    let key = format!("video:detail-{code}");
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let mut out = state.video.detail(&app, &code).await?;
    // 播放器只接触代理地址(CDN 校验 Referer/TLS,直连必 403)
    out.m3u8 = video_proxy::wrap(&out.m3u8);
    state.put(key, &out);
    Ok(out)
}

/// 哔咔排行榜;tt 为 "H24"(日)/"D7"(周)/"D30"(月)
#[tauri::command]
async fn fetch_bika_leaderboard(
    tt: String,
    state: State<'_, AppState>,
) -> Result<CategoryListResult, String> {
    let key = format!("bika:leaderboard-{tt}");
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let out = {
        let bika = state.bika_snapshot();
        state
            .bika_call(|token| bika.leaderboard(token, &tt))
            .await?
    };
    state.put(key, &out);
    Ok(out)
}

#[tauri::command]
async fn search_comics(
    source: Source,
    keyword: String,
    classid: i32,
    show: String,
    page: i32,
    cache_buster: Option<String>,
    state: State<'_, AppState>,
) -> Result<SearchResult, String> {
    let key = format!(
        "{source}:search-{classid}-{keyword}-{show}-{page}-{}",
        cache_buster.unwrap_or_default()
    );
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let mut out = match source {
        Source::Bika => {
            let bika = state.bika_snapshot();
            let result = if show == "tags" {
                state
                    .bika_call(|token| bika.comics(token, "", &keyword, "", page + 1))
                    .await?
            } else if show == "writer" {
                state
                    .bika_call(|token| bika.comics(token, "", "", &keyword, page + 1))
                    .await?
            } else {
                state
                    .bika_call(|token| bika.search(token, &keyword, page + 1))
                    .await?
            };
            SearchResult {
                items: result.items,
                pagination: result.pagination,
                page_url_template: None,
            }
        }
        Source::Niacg => {
            // 全局限速:niacg 搜索间隔不足 5 秒会返回无结果的提示页
            let _search_turn = state.niacg_search_turn().await;
            if show == "tags" {
                let html = state
                    .niacg_get(&format!(
                        "/tags-{}-{page}.html",
                        url::form_urlencoded::byte_serialize(keyword.as_bytes())
                            .collect::<String>()
                    ))
                    .await?;
                let mut r = sources::niacg::search(&html);
                let max = sources::niacg::tag_max_page(&html);
                r.pagination = PaginationInfo {
                    current: page,
                    total: max + 1,
                    has_next: page < max,
                    has_prev: page > 0,
                };
                r
            } else {
                let body = [
                    ("classid", classid.to_string()),
                    ("keyboard", keyword),
                    ("show", show),
                    ("tempid", "1".into()),
                    ("Submit", "".into()),
                ];
                let html = state
                    .niacg
                    .post(format!("{BASE}/e/search/index.php"))
                    .form(&body)
                    .send()
                    .await
                    .map_err(|e| e.to_string())?
                    .error_for_status()
                    .map_err(|e| e.to_string())?
                    .text()
                    .await
                    .map_err(|e| e.to_string())?;
                let first = sources::niacg::search(&html);
                let total_pages = first.pagination.total + 1;
                if page == 0 {
                    SearchResult {
                        pagination: PaginationInfo {
                            current: 0,
                            total: total_pages,
                            has_next: total_pages > 1,
                            has_prev: false,
                        },
                        ..first
                    }
                } else {
                    let template = first
                        .page_url_template
                        .clone()
                        .ok_or("Search pagination URL unavailable")?;
                    let mut result = sources::niacg::search(
                        &state
                            .niacg_get(&template.replace("{}", &(page + 1).to_string()))
                            .await?,
                    );
                    result.pagination = PaginationInfo {
                        current: page,
                        total: total_pages,
                        has_next: page + 1 < total_pages,
                        has_prev: true,
                    };
                    result
                }
            }
        }
    };
    out.pagination.current = page;
    out.pagination.has_prev = page > 0;
    state.put(key, &out);
    Ok(out)
}

#[tauri::command]
async fn fetch_comic(
    source: Source,
    category_id: i32,
    id: String,
    state: State<'_, AppState>,
) -> Result<ComicDetail, String> {
    let key = format!("{source}:detail-{category_id}-{id}");
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let out = match source {
        Source::Bika => {
            let bika = state.bika_snapshot();
            let mut detail = state.bika_call(|token| bika.detail(token, &id)).await?;
            detail.episodes = state.bika_call(|token| bika.episodes(token, &id)).await?;
            detail
        }
        Source::Niacg => sources::niacg::detail(
            &state
                .niacg_get(&format!("/moehome-{category_id}-{id}.html"))
                .await?,
            category_id,
            id.clone(),
        ),
    };
    state.put(key, &out);
    Ok(out)
}

#[tauri::command]
async fn fetch_comic_images(
    source: Source,
    category_id: i32,
    id: String,
    ep_order: i32,
    page: i32,
    state: State<'_, AppState>,
) -> Result<ComicImagesResult, String> {
    match source {
        Source::Bika => {
            let bika = state.bika_snapshot();
            state
                .bika_call(|token| bika.pictures(token, &id, ep_order, page + 1))
                .await
        }
        Source::Niacg => {
            let key = format!("niacg:images-{category_id}-{id}");
            if let Some(images) = state.cached::<Vec<String>>(&key) {
                return Ok(ComicImagesResult {
                    images,
                    pagination: PaginationInfo {
                        current: 0,
                        total: 1,
                        has_next: false,
                        has_prev: false,
                    },
                });
            }
            let images = sources::niacg::images(
                &state
                    .niacg_get(&format!("/moeupup-{category_id}-{id}.html"))
                    .await?,
            );
            state.put(key, &images);
            Ok(ComicImagesResult {
                pagination: PaginationInfo {
                    current: 0,
                    total: 1,
                    has_next: false,
                    has_prev: false,
                },
                images,
            })
        }
    }
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
async fn bika_login(
    email: String,
    password: String,
    state: State<'_, AppState>,
) -> Result<BikaAccountStatus, String> {
    let token = state.bika_snapshot().sign_in(&email, &password).await?;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::save_bika_account(&conn, &email, &password, &token)?;
    }
    *state.bika_token.lock().map_err(|e| e.to_string())? = Some(token);
    state.clear_cache();
    Ok(BikaAccountStatus {
        logged_in: true,
        email,
    })
}

#[tauri::command]
async fn bika_logout(state: State<'_, AppState>) -> Result<(), String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::clear_bika_account(&conn)?;
    }
    *state.bika_token.lock().map_err(|e| e.to_string())? = None;
    state.clear_cache();
    Ok(())
}

#[tauri::command]
fn bika_status(state: State<'_, AppState>) -> Result<BikaAccountStatus, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let account = db::bika_account(&conn)?;
    Ok(match account {
        Some(account) => BikaAccountStatus {
            logged_in: true,
            email: account.email,
        },
        None => BikaAccountStatus {
            logged_in: false,
            email: String::new(),
        },
    })
}

/// 从哔咔官方 init 端点同步可用分流地址
#[tauri::command]
async fn fetch_bika_addresses(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    state.bika_snapshot().fetch_addresses().await
}

/// 哔咔漫画点赞/取消点赞(toggle)
#[tauri::command]
async fn bika_switch_like(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let bika = state.bika_snapshot();
    state.bika_call(|token| bika.switch_like(token, &id)).await
}

/// 哔咔漫画收藏/取消收藏(toggle)
#[tauri::command]
async fn bika_switch_favourite(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let bika = state.bika_snapshot();
    state
        .bika_call(|token| bika.switch_favourite(token, &id))
        .await
}

#[tauri::command]
fn fetch_favorites(state: State<'_, AppState>) -> Result<Vec<FavoriteEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::favorites(&conn)
}

#[tauri::command]
fn is_favorite(
    source: Source,
    comic_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::is_favorite(&conn, source, &comic_id)
}

/// 收藏漫画:写入本地表;哔咔源同时同步到服务器(toggle 方向由
/// 调用方当前状态决定,前端保证仅在未收藏时调用)
#[tauri::command]
async fn add_favorite(entry: RecordFavorite, state: State<'_, AppState>) -> Result<(), String> {
    if entry.source == Source::Bika {
        let bika = state.bika_snapshot();
        let id = entry.comic_id.clone();
        state
            .bika_call(|token| bika.switch_favourite(token, &id))
            .await?;
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::add_favorite(&conn, &entry)
}

/// 取消收藏:本地删除;哔咔源同时从服务器移除
#[tauri::command]
async fn remove_favorite(
    source: Source,
    comic_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if source == Source::Bika {
        let bika = state.bika_snapshot();
        state
            .bika_call(|token| bika.switch_favourite(token, &comic_id))
            .await?;
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::remove_favorite(&conn, source, &comic_id)
}

/// 把哔咔服务器端的收藏拉取并合并到本地表(官方 app 收的也能看到);
/// 返回本次同步新增的条数
#[tauri::command]
async fn sync_bika_favorites(state: State<'_, AppState>) -> Result<usize, String> {
    let before = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::favorites(&conn)?.len()
    };
    let bika = state.bika_snapshot();
    let mut page = 1;
    loop {
        let result = state
            .bika_call(|token| bika.favourite_list(token, page))
            .await?;
        let has_next = result.pagination.has_next;
        {
            let conn = state.db.lock().map_err(|e| e.to_string())?;
            for item in &result.items {
                let entry = RecordFavorite {
                    source: Source::Bika,
                    comic_id: item.id.clone(),
                    title: item.title.clone(),
                    thumbnail: item.thumbnail.clone(),
                    category_id: item.category_id,
                    author: item.author.clone(),
                };
                db::add_favorite(&conn, &entry)?;
            }
        }
        if !has_next {
            break;
        }
        page += 1;
    }
    let after = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::favorites(&conn)?.len()
    };
    Ok(after.saturating_sub(before))
}

/// 当前分流地址,空字符串表示不分流
#[tauri::command]
fn get_bika_address(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::bika_address(&conn)
}

/// 设置分流地址并重建哔咔客户端;空字符串表示恢复系统 DNS
#[tauri::command]
async fn set_bika_address(address: String, state: State<'_, AppState>) -> Result<(), String> {
    let address = address.trim().to_string();
    let api = BikaApi::new(if address.is_empty() {
        None
    } else {
        Some(&address)
    })?;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::save_bika_address(&conn, &address)?;
    }
    *state.bika.write().map_err(|e| e.to_string())? = api;
    state.clear_cache();
    Ok(())
}

#[tauri::command]
fn fetch_follows(state: State<'_, AppState>) -> Result<Vec<FollowedAuthor>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::follows(&conn)
}
#[tauri::command]
fn follow_author(author: String, state: State<'_, AppState>) -> Result<FollowedAuthor, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::follow(&conn, &author)
}
#[tauri::command]
fn unfollow_author(author: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::unfollow(&conn, &author)
}
#[tauri::command]
fn fetch_history(limit: i64, state: State<'_, AppState>) -> Result<Vec<ViewHistoryEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::history(&conn, limit)
}
#[tauri::command]
fn record_history(entry: RecordHistory, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::record(&conn, entry)
}
#[tauri::command]
fn fetch_blacklist(state: State<'_, AppState>) -> Result<Vec<BlacklistEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::blacklist(&conn)
}
#[tauri::command]
fn add_blacklist(
    tag: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<BlacklistEntry, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::add_blacklist(&conn, &tag, &mode)
}
#[tauri::command]
fn remove_blacklist(tag: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::remove_blacklist(&conn, &tag)
}
#[tauri::command]
fn update_blacklist(tag: String, mode: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::update_blacklist(&conn, &tag, &mode)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .register_uri_scheme_protocol("stream", video_proxy::register)
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&dir)?;
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
            app.manage(AppState {
                niacg,
                video,
                gateway: GatewayManager::new(),
                bika: RwLock::new(bika),
                bika_token: Mutex::new(None),
                db: Mutex::new(conn),
                cache: Mutex::new(HashMap::new()),
                niacg_search_gate: tokio::sync::Mutex::new(Instant::now() - Duration::from_secs(3600)),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            fetch_category,
            fetch_bika_categories,
            fetch_bika_leaderboard,
            fetch_video_list,
            search_videos,
            fetch_video_detail,
            gateway_fetch,
            gateway_set_visible,
            gateway_result,
            novel_add,
            novel_shelf,
            novel_remove,
            novel_read,
            novel_progress,
            novel_set_progress,
            bika_switch_like,
            bika_switch_favourite,
            fetch_favorites,
            is_favorite,
            add_favorite,
            remove_favorite,
            sync_bika_favorites,
            search_comics,
            fetch_comic,
            fetch_comic_images,
            fetch_image,
            bika_login,
            bika_logout,
            bika_status,
            fetch_bika_addresses,
            get_bika_address,
            set_bika_address,
            fetch_follows,
            follow_author,
            unfollow_author,
            fetch_history,
            record_history,
            fetch_blacklist,
            add_blacklist,
            remove_blacklist,
            update_blacklist
        ])
        .run(tauri::generate_context!())
        .expect("failed to run app")
}
