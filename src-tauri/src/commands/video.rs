//! video 域 command(重构笔 3:按源分组)

use crate::db;
use crate::diagnostics::diag;
use crate::fetch_image_with_client;
use crate::fetch_image_with_wreq;
use crate::models::*;
use crate::sources;
use crate::AppState;
use tauri::ipc::Response;
use tauri::AppHandle;
use tauri::State;

use crate::sources::video::{VideoDetail, VideoListResult};
use crate::stream_proxy;

/// 视频列表(fs1 最新上市),page 从 1 开始
#[tauri::command]
pub(crate) async fn fetch_video_list(
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
pub(crate) async fn search_videos(
    app: AppHandle,
    state: State<'_, AppState>,
    keyword: String,
    page: i32,
) -> Result<VideoListResult, String> {
    let key = format!("video:search-{keyword}-{page}");
    if let Some(v) = state.cached(&key) {
        return Ok(v);
    }
    let out = match state.video.search(&keyword, page).await {
        Ok(v) => {
            diagnostics::diag(
                &app,
                &format!("search ok kw={keyword} items={}", v.items.len()),
            );
            v
        }
        Err(e) => {
            diagnostics::diag(&app, &format!("search FAIL kw={keyword} err={e}"));
            return Err(e);
        }
    };
    state.put(key, &out);
    Ok(out)
}

/// 视频详情(元数据与播放优先走 fs1,不可用时由 missav 直连兜底)
#[tauri::command]
pub(crate) async fn fetch_video_detail(
    state: State<'_, AppState>,
    code: String,
) -> Result<VideoDetail, String> {
    let key = format!("video:detail-{code}");
    if let Some(mut v) = state.cached::<VideoDetail>(&key) {
        // 缓存存原始地址;代理端口每次启动随机,读取时重新包装
        v.m3u8 = stream_proxy::hls_url(&v.m3u8);
        return Ok(v);
    }
    let out = state.video.detail(&code).await?;
    // 播放器只接触本地代理地址(CDN 校验 Referer/拒绝浏览器 TLS,直连必 403)
    let mut playable = out.clone();
    playable.m3u8 = stream_proxy::hls_url(&out.m3u8);
    state.put(key, &out);
    Ok(playable)
}

/// 视频封面按图床分流:fourhoi(missav 卡片封面)防盗链仅放行白名单
/// Referer 与空 Referer,fs1 等 Referer 一律 403——对它用 missav 的
/// wreq 客户端(Chrome 指纹、默认无 Referer);其余图床沿用 fs1 客户端
/// 请求头(fs1 自家封面需其 Referer)。
#[tauri::command]
pub(crate) async fn fetch_video_image(
    app: AppHandle,
    url: String,
    state: State<'_, AppState>,
) -> Result<Response, String> {
    let result = if sources::video::is_fourhoi_url(&url) {
        fetch_image_with_wreq(state.video.missav.clone(), url.clone()).await
    } else {
        fetch_image_with_client(state.video.client.clone(), url.clone()).await
    };
    if let Err(e) = &result {
        diagnostics::diag(&app, &format!("video image FAIL {url}: {e}"));
    }
    result
}

#[tauri::command]
pub(crate) fn record_video_watched(code: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::record_video_watched(&conn, &code)
}

/// 批量查询视频已看集合(入集中已看的 code)
#[tauri::command]
pub(crate) fn fetch_video_watched_ids(
    codes: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::video_watched_ids(&conn, &codes)
}

/// 关注/取关视频目标(厂商/女优;source: fs1|missav, kind: maker|actress)
#[tauri::command]
pub(crate) fn video_follow(
    source: String,
    kind: String,
    target_id: String,
    display_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::follow_target_add(&conn, &source, &kind, &target_id, &display_name)
}

#[tauri::command]
pub(crate) fn video_unfollow(
    source: String,
    kind: String,
    target_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::follow_target_remove(&conn, &source, &kind, &target_id)
}

/// 关注目标列表(按来源+类型过滤)
#[tauri::command]
pub(crate) fn video_follow_list(
    source: String,
    kind: String,
    state: State<'_, AppState>,
) -> Result<Vec<db::FollowTargetEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::follow_targets(&conn, &source, &kind)
}

/// 可关注目标列表(fs1 女优 / missav 厂商 / missav 女优排行)
#[tauri::command]
pub(crate) async fn video_follow_targets(
    source: String,
    kind: String,
    state: State<'_, AppState>,
) -> Result<Vec<sources::video::FollowTargetItem>, String> {
    match (source.as_str(), kind.as_str()) {
        ("fs1", "actress") => state.video.fs1_models_list().await,
        ("missav", "maker") => state.video.missav_makers_list().await,
        ("missav", "actress") => state.video.missav_actresses_list().await,
        _ => Err(format!("不支持的关注类型 {source}/{kind}")),
    }
}

/// 关注对象的作品流(详情页视频列表,复用现有解析;串行闸保站点友好)
#[tauri::command]
pub(crate) async fn video_follow_works(
    source: String,
    kind: String,
    target_id: String,
    page: Option<i32>,
    state: State<'_, AppState>,
) -> Result<sources::video::VideoListResult, String> {
    let page = page.unwrap_or(1).max(1);
    match (source.as_str(), kind.as_str()) {
        ("fs1", "actress") => state.video.fs1_model_works(&target_id, page).await,
        // 关注(maker/actress)与详情页元数据条目(男优/系列/导演/标签/类型)
        // 共用同一作品流通道,路由映射见 missav_list_url
        (
            "missav",
            kind @ ("maker" | "actress" | "actor" | "series" | "director" | "label" | "genre"),
        ) => {
            state
                .video
                .missav_follow_works(kind, &target_id, page)
                .await
        }
        _ => Err(format!("不支持的作品流类型 {source}/{kind}")),
    }
}
