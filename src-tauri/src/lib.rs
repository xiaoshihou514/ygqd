mod db;
mod models;
mod sources;

use models::*;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use rusqlite::Connection;
use sources::bika::BikaApi;
use std::{
    collections::HashMap,
    future::Future,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{ipc::Response, Manager, State};

const BASE: &str = "https://www.niacg.com";
/// niacg 限制两次搜索至少间隔 5 秒,违反时返回无结果的提示页
const NIACG_SEARCH_INTERVAL: Duration = Duration::from_millis(5200);
struct AppState {
    niacg: reqwest::Client,
    bika: BikaApi,
    bika_token: Mutex<Option<String>>,
    db: Mutex<Connection>,
    cache: Mutex<HashMap<String, (Instant, serde_json::Value)>>,
    /// niacg 搜索全局限速:串行化并保证间隔,避免触发 5 秒限制拿到空结果
    niacg_search_gate: tokio::sync::Mutex<Instant>,
}
impl AppState {
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
        let token = self.bika.sign_in(&email, &password).await?;
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
fn rewrite(items: &mut [ComicItem]) {
    for v in items {
        if v.source == "niacg" {
            v.thumbnail = v.thumbnail.clone();
        }
    }
}

#[tauri::command]
async fn fetch_category(
    source: String,
    category: String,
    page: i32,
    state: State<'_, AppState>,
) -> Result<CategoryListResult, String> {
    let key = format!("{source}:list-{category}-{page}");
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let mut out = if source == "bika" {
        state
            .bika_call(|token| state.bika.comics(token, &category, "", "", page + 1))
            .await?
    } else {
        sources::niacg::list(
            &state
                .niacg_get(&format!("/listinfo-{category}-{page}.html"))
                .await?,
        )
    };
    rewrite(&mut out.items);
    state.put(key, &out);
    Ok(out)
}

#[tauri::command]
async fn fetch_bika_categories(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let key = "bika:categories";
    if let Some(v) = state.cached(key) {
        return Ok(v);
    }
    let out = state
        .bika_call(|token| state.bika.categories(token))
        .await?;
    state.put(key.into(), &out);
    Ok(out)
}

#[tauri::command]
async fn search_comics(
    source: String,
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
    let mut out = if source == "bika" {
        let result = if show == "tags" {
            state
                .bika_call(|token| state.bika.comics(token, "", &keyword, "", page + 1))
                .await?
        } else if show == "writer" {
            state
                .bika_call(|token| state.bika.comics(token, "", "", &keyword, page + 1))
                .await?
        } else {
            state
                .bika_call(|token| state.bika.search(token, &keyword, page + 1))
                .await?
        };
        SearchResult {
            items: result.items,
            pagination: result.pagination,
            page_url_template: None,
        }
    } else {
        // 全局限速:niacg 搜索间隔不足 5 秒会返回无结果的提示页
        let _search_turn = state.niacg_search_turn().await;
        if show == "tags" {
            let html = state
                .niacg_get(&format!(
                    "/tags-{}-{page}.html",
                    url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect::<String>()
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
    };
    out.pagination.current = page;
    out.pagination.has_prev = page > 0;
    rewrite(&mut out.items);
    state.put(key, &out);
    Ok(out)
}

#[tauri::command]
async fn fetch_comic(
    source: String,
    category_id: i32,
    id: String,
    state: State<'_, AppState>,
) -> Result<ComicDetail, String> {
    let key = format!("{source}:detail-{category_id}-{id}");
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let out = if source == "bika" {
        let mut detail = state
            .bika_call(|token| state.bika.detail(token, &id))
            .await?;
        detail.episodes = state
            .bika_call(|token| state.bika.episodes(token, &id))
            .await?;
        detail
    } else {
        sources::niacg::detail(
            &state
                .niacg_get(&format!("/moehome-{category_id}-{id}.html"))
                .await?,
            category_id,
            id.clone(),
        )
    };
    state.put(key, &out);
    Ok(out)
}

#[tauri::command]
async fn fetch_comic_images(
    source: String,
    category_id: i32,
    id: String,
    ep_order: i32,
    page: i32,
    state: State<'_, AppState>,
) -> Result<ComicImagesResult, String> {
    if source == "bika" {
        return state
            .bika_call(|token| state.bika.pictures(token, &id, ep_order, page + 1))
            .await;
    }
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

#[tauri::command]
async fn fetch_image(
    source: String,
    url: String,
    state: State<'_, AppState>,
) -> Result<Response, String> {
    let client = if source == "bika" {
        &state.bika.client
    } else {
        &state.niacg
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
    let token = state.bika.sign_in(&email, &password).await?;
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
    tauri::Builder::default().setup(|app|{let dir=app.path().app_data_dir()?;std::fs::create_dir_all(&dir)?;let mut headers=HeaderMap::new();headers.insert(USER_AGENT,HeaderValue::from_static("Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36"));headers.insert(REFERER,HeaderValue::from_static(BASE));let niacg=reqwest::Client::builder().default_headers(headers).cookie_store(true).build()?;let bika=BikaApi::new()?;let conn=db::open(&dir.join("ygqd.db")).map_err(std::io::Error::other)?;app.manage(AppState{niacg,bika,bika_token:Mutex::new(None),db:Mutex::new(conn),cache:Mutex::new(HashMap::new()),niacg_search_gate:tokio::sync::Mutex::new(Instant::now()-Duration::from_secs(3600))});Ok(())}).invoke_handler(tauri::generate_handler![fetch_category,fetch_bika_categories,search_comics,fetch_comic,fetch_comic_images,fetch_image,bika_login,bika_logout,bika_status,fetch_follows,follow_author,unfollow_author,fetch_history,record_history,fetch_blacklist,add_blacklist,remove_blacklist,update_blacklist]).run(tauri::generate_context!()).expect("failed to run app")
}
