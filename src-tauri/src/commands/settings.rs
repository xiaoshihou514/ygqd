//! settings 域 command(重构笔 3:按源分组)
//!
//! 全部为纯数据库 command:标注 `async` 由 tokio 线程池执行,
//! 避免 SQLite 读写(及等锁)阻塞主线程。

use crate::db;
use crate::models::*;
use crate::AppState;
use tauri::State;

/// 记录全局历史(视频/小说;漫画继续走 record_history 存量)
#[tauri::command(async)]
pub(crate) fn record_item_history(
    source: String,
    kind: String,
    item_id: String,
    title: String,
    progress: Option<String>,
    thumbnail: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.with_db(|c| {
        db::item_history_record(
            c,
            &source,
            &kind,
            &item_id,
            &title,
            progress.as_deref(),
            thumbnail.as_deref(),
        )
    })
}

/// 全局历史查询(source/kind 可选过滤;None=全板块时间线)
#[tauri::command(async)]
pub(crate) fn fetch_item_history(
    source: Option<String>,
    kind: Option<String>,
    limit: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<db::ItemHistoryEntry>, String> {
    state.with_db(|c| {
        db::item_history_fetch(c, source.as_deref(), kind.as_deref(), limit.unwrap_or(100))
    })
}

#[tauri::command(async)]
pub(crate) fn fetch_follows(state: State<'_, AppState>) -> Result<Vec<FollowedAuthor>, String> {
    state.with_db(db::follows)
}

#[tauri::command(async)]
pub(crate) fn follow_author(
    author: String,
    state: State<'_, AppState>,
) -> Result<FollowedAuthor, String> {
    state.with_db(|c| db::follow(c, &author))
}

#[tauri::command(async)]
pub(crate) fn unfollow_author(author: String, state: State<'_, AppState>) -> Result<(), String> {
    state.with_db(|c| db::unfollow(c, &author))
}

#[tauri::command(async)]
pub(crate) fn fetch_history(
    limit: i64,
    state: State<'_, AppState>,
) -> Result<Vec<ViewHistoryEntry>, String> {
    state.with_db(|c| db::history(c, limit))
}

#[tauri::command(async)]
pub(crate) fn record_history(
    entry: RecordHistory,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.with_db(|c| db::record(c, entry))
}

#[tauri::command(async)]
pub(crate) fn fetch_blacklist(state: State<'_, AppState>) -> Result<Vec<BlacklistEntry>, String> {
    state.with_db(db::blacklist)
}

#[tauri::command(async)]
pub(crate) fn add_blacklist(
    tag: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<BlacklistEntry, String> {
    state.with_db(|c| db::add_blacklist(c, &tag, &mode))
}

#[tauri::command(async)]
pub(crate) fn remove_blacklist(tag: String, state: State<'_, AppState>) -> Result<(), String> {
    state.with_db(|c| db::remove_blacklist(c, &tag))
}

#[tauri::command(async)]
pub(crate) fn update_blacklist(
    tag: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.with_db(|c| db::update_blacklist(c, &tag, &mode))
}
