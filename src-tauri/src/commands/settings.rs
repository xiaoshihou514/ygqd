//! settings 域 command(重构笔 3:按源分组)

use crate::db;
use crate::models::*;
use crate::AppState;
use tauri::State;

/// 记录全局历史(视频/小说;漫画继续走 record_history 存量)
#[tauri::command]
pub(crate) fn record_item_history(
    source: String,
    kind: String,
    item_id: String,
    title: String,
    progress: Option<String>,
    thumbnail: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::item_history_record(
        &conn,
        &source,
        &kind,
        &item_id,
        &title,
        progress.as_deref(),
        thumbnail.as_deref(),
    )
}

/// 全局历史查询(source/kind 可选过滤;None=全板块时间线)
#[tauri::command]
pub(crate) fn fetch_item_history(
    source: Option<String>,
    kind: Option<String>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<db::ItemHistoryEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::item_history_fetch(
        &conn,
        source.as_deref(),
        kind.as_deref(),
        limit.unwrap_or(100),
    )
}

#[tauri::command]
pub(crate) fn fetch_follows(state: State<'_, AppState>) -> Result<Vec<FollowedAuthor>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::follows(&conn)
}

#[tauri::command]
pub(crate) fn follow_author(
    author: String,
    state: State<'_, AppState>,
) -> Result<FollowedAuthor, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::follow(&conn, &author)
}

#[tauri::command]
pub(crate) fn unfollow_author(author: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::unfollow(&conn, &author)
}

#[tauri::command]
pub(crate) fn fetch_history(
    limit: i64,
    state: State<'_, AppState>,
) -> Result<Vec<ViewHistoryEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::history(&conn, limit)
}

#[tauri::command]
pub(crate) fn record_history(
    entry: RecordHistory,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::record(&conn, entry)
}

#[tauri::command]
pub(crate) fn fetch_blacklist(state: State<'_, AppState>) -> Result<Vec<BlacklistEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::blacklist(&conn)
}

#[tauri::command]
pub(crate) fn add_blacklist(
    tag: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<BlacklistEntry, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::add_blacklist(&conn, &tag, &mode)
}

#[tauri::command]
pub(crate) fn remove_blacklist(tag: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::remove_blacklist(&conn, &tag)
}

#[tauri::command]
pub(crate) fn update_blacklist(
    tag: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::update_blacklist(&conn, &tag, &mode)
}
