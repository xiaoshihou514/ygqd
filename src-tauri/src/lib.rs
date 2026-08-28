mod db;
mod models;
mod parser;

use models::*;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use rusqlite::Connection;
use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::{ipc::Response, Manager, State};

const BASE: &str = "https://www.niacg.com";
struct AppState {
    client: reqwest::Client,
    db: Mutex<Connection>,
    cache: Mutex<HashMap<String, (Instant, serde_json::Value)>>,
}
impl AppState {
    async fn get(&self, path: &str) -> Result<String, String> {
        self.client
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
}
fn proxied(url: &str) -> String {
    url.to_string()
}
fn rewrite(items: &mut [ComicItem]) {
    for v in items {
        v.thumbnail = proxied(&v.thumbnail)
    }
}

#[tauri::command]
async fn fetch_home(state: State<'_, AppState>) -> Result<Vec<HomeSection>, String> {
    let key = "home";
    if let Some(v) = state.cached(key) {
        return Ok(v);
    }
    let html = state.get("/").await?;
    let out = parser::home(&html);
    state.put(key.into(), &out);
    Ok(out)
}
#[tauri::command]
async fn fetch_category(
    category: i32,
    page: i32,
    state: State<'_, AppState>,
) -> Result<CategoryListResult, String> {
    let key = format!("list-{category}-{page}");
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let mut out = parser::list(
        &state
            .get(&format!("/listinfo-{category}-{page}.html"))
            .await?,
    );
    rewrite(&mut out.items);
    state.put(key, &out);
    Ok(out)
}
#[tauri::command]
async fn search_comics(
    keyword: String,
    classid: i32,
    show: String,
    page: i32,
    cache_buster: Option<String>,
    state: State<'_, AppState>,
) -> Result<SearchResult, String> {
    let key = format!(
        "search-{classid}-{keyword}-{show}-{page}-{}",
        cache_buster.unwrap_or_default()
    );
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let mut out = if show == "tags" {
        let html = state
            .get(&format!(
                "/tags-{}-{page}.html",
                url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect::<String>()
            ))
            .await?;
        let mut r = parser::search(&html);
        let max = parser::tag_max_page(&html);
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
            .client
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
        let first = parser::search(&html);
        if page == 0 {
            first
        } else {
            let template = first
                .page_url_template
                .clone()
                .ok_or("Search pagination URL unavailable")?;
            parser::search(
                &state
                    .get(&template.replace("{}", &(page + 1).to_string()))
                    .await?,
            )
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
    category_id: i32,
    id: String,
    state: State<'_, AppState>,
) -> Result<ComicDetail, String> {
    let key = format!("detail-{category_id}-{id}");
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let mut out = parser::detail(
        &state
            .get(&format!("/moehome-{category_id}-{id}.html"))
            .await?,
        category_id,
        id.clone(),
    );
    out.images = parser::images(
        &state
            .get(&format!("/moeupup-{category_id}-{id}.html"))
            .await?,
    );
    state.put(key, &out);
    Ok(out)
}
#[tauri::command]
async fn fetch_image(url: String, state: State<'_, AppState>) -> Result<Response, String> {
    let bytes = state
        .client
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
    tauri::Builder::default().setup(|app|{let dir=app.path().app_data_dir()?;std::fs::create_dir_all(&dir)?;let mut headers=HeaderMap::new();headers.insert(USER_AGENT,HeaderValue::from_static("Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36"));headers.insert(REFERER,HeaderValue::from_static(BASE));let client=reqwest::Client::builder().default_headers(headers).cookie_store(true).build()?;let conn=db::open(&dir.join("ygqd.db")).map_err(std::io::Error::other)?;app.manage(AppState{client,db:Mutex::new(conn),cache:Mutex::new(HashMap::new())});Ok(())}).invoke_handler(tauri::generate_handler![fetch_home,fetch_category,search_comics,fetch_comic,fetch_image,fetch_follows,follow_author,unfollow_author,fetch_history,record_history,fetch_blacklist,add_blacklist,remove_blacklist,update_blacklist]).run(tauri::generate_context!()).expect("failed to run app")
}
