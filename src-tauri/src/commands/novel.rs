//! novel 域 command(重构笔 3:按源分组)

use crate::db;
use crate::models::*;
use crate::sources;
use crate::AppState;
use std::time::Duration;
use tauri::Manager;
use tauri::State;

use tauri::AppHandle;

/// 系列详情预览:按 hlib 系列 ID 抓取系列信息(章节列表/简介),
/// 不入库——供「浏览 → 决策 → 加书架」漏斗的浏览阶段使用。
/// 首次会弹出网关窗口(登录/安全验证与 novel_add 同一套会话)。
#[tauri::command]
pub(crate) async fn novel_series_info(app: AppHandle, id: String) -> Result<NovelSeries, String> {
    sources::novel::fetch_series(&app, &id).await
}

#[tauri::command]
pub(crate) fn novel_blacklist_list(
    state: State<'_, AppState>,
) -> Result<Vec<db::NovelBlacklistEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::novel_blacklist_list(&conn)
}

#[tauri::command]
pub(crate) fn novel_blacklist_add(
    keyword: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::novel_blacklist_add(&conn, &keyword)
}

#[tauri::command]
pub(crate) fn novel_blacklist_remove(
    keyword: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::novel_blacklist_remove(&conn, &keyword)
}

/// 推荐流:hlib 首页聚合(最新+推荐位混合,进入即看到内容)
#[tauri::command]
pub(crate) async fn novel_recommend(
    app: AppHandle,
    state: State<'_, AppState>,
    page: Option<i64>,
) -> Result<sources::novel::NovelLatestPage, String> {
    state.hlib_throttle().await;
    let key = format!("novel_recommend_p{}", page.unwrap_or(1));
    // 访问预算:手动刷新数据缓存 30 分钟(批次 2 默认)
    if let Some(hit) =
        state.cached_ttl::<sources::novel::NovelLatestPage>(&key, Duration::from_secs(1800))
    {
        return Ok(hit);
    }
    let res = sources::novel::latest_series(&app, page.unwrap_or(1)).await;
    if let Ok(mut page_data) = res {
        page_data.hits = filter_blacklisted(&state, page_data.hits);
        state.put(key.clone(), &page_data);
        Ok(page_data)
    } else {
        res
    }
}

/// 系列排行榜(日/周/月;/rank/s 需登录,登录后网关会话可用)
#[tauri::command]
pub(crate) async fn novel_ranking(
    app: AppHandle,
    state: State<'_, AppState>,
    period: Option<String>,
    page: Option<i64>,
) -> Result<sources::novel::NovelLatestPage, String> {
    state.hlib_throttle().await;
    let period = period.unwrap_or_else(|| "day".into());
    let key = format!("novel_ranking_{}_p{}", period, page.unwrap_or(1));
    if let Some(hit) =
        state.cached_ttl::<sources::novel::NovelLatestPage>(&key, Duration::from_secs(1800))
    {
        return Ok(hit);
    }
    // 周期切换控件在榜单页上自适应发现(见 rank_series)
    let page_data = sources::novel::rank_series(
        &app,
        sources::novel::RankPeriod::parse(&period),
        page.unwrap_or(1),
    )
    .await?;
    state.put(key, &page_data);
    Ok(page_data)
}

/// 关注作者的作品流:/u/{author_id} 页面的 /n/ 单篇列表
#[tauri::command]
pub(crate) async fn novel_followed_author_works(
    app: AppHandle,
    state: State<'_, AppState>,
    author_id: String,
) -> Result<Vec<sources::novel::NovelHit>, String> {
    state.hlib_throttle().await;
    let key = format!("novel_author_works_{author_id}");
    if let Some(hit) =
        state.cached_ttl::<Vec<sources::novel::NovelHit>>(&key, Duration::from_secs(1800))
    {
        return Ok(hit);
    }
    let res = sources::novel::author_page_works(&app, &author_id).await;
    if let Ok(works) = &res {
        state.put(key.clone(), works);
    }
    // 自查补漏(需求 5):关注作者作品流同样过黑名单
    res.map(|works| filter_blacklisted(&state, works))
}

/// 关注作者列表(复用漫画源 followed_authors 表;authorId 以作者名充当)
#[tauri::command]
pub(crate) fn novel_followed_authors(
    state: State<'_, AppState>,
) -> Result<Vec<FollowedAuthor>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::follows(&conn)
}

/// 添加小说系列到书架:输入 hlib 系列链接或纯 ID。
/// 首次会弹出网关窗口,需用户完成一次登录/安全验证。
#[tauri::command]
pub(crate) async fn novel_add(
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

/// 站内搜索系列(端点自适应;首次需网关完成验证/登录)
#[tauri::command]
pub(crate) async fn novel_search(
    app: AppHandle,
    keyword: String,
) -> Result<Vec<sources::novel::NovelHit>, String> {
    sources::novel::search_series(&app, &keyword).await
}

/// 发现:首页最新系列
/// 分页搜索:返回该页命中与总页数(前端「加载更多」用)
#[tauri::command]
pub(crate) async fn novel_search_page(
    app: AppHandle,
    keyword: String,
    page: Option<i64>,
    sort: Option<String>,
    state: State<'_, AppState>,
) -> Result<sources::novel::NovelSearchPage, String> {
    let page_data =
        sources::novel::search_series_page(&app, &keyword, page.unwrap_or(1), sort.as_deref())
            .await?;
    let mut page_data = page_data;
    page_data.hits = filter_blacklisted(&state, page_data.hits);
    Ok(page_data)
}

#[tauri::command]
pub(crate) async fn novel_latest(
    app: AppHandle,
    page: Option<i64>,
) -> Result<sources::novel::NovelLatestPage, String> {
    // page 可选:前端不传视为第 1 页(旧调用兼容)
    let page = page.unwrap_or(1);
    sources::novel::latest_series(&app, page).await
}

#[tauri::command]
pub(crate) fn novel_shelf(state: State<'_, AppState>) -> Result<Vec<NovelSeries>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::novel_shelf(&conn)
}

#[tauri::command]
pub(crate) fn novel_remove(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::novel_remove(&conn, &id)
}

/// 读取章节某页正文;网关直连 hlib,未登录/未验证时返回引导错误
#[tauri::command]
pub(crate) async fn novel_read(
    app: AppHandle,
    state: State<'_, AppState>,
    chapter_id: String,
    page: i64,
) -> Result<NovelPage, String> {
    // 问题 9:章节内容缓存(24 小时,读过的不重抓——减少网关抓取
    // 次数 = 减少 CF 弹窗次数)
    let key = format!("article_{chapter_id}_p{page}");
    if let Some(hit) = state.cached_ttl::<NovelPage>(&key, Duration::from_secs(24 * 3600)) {
        return Ok(hit);
    }
    state.hlib_throttle().await;
    let res = sources::novel::fetch_article_page(&app, &chapter_id, page).await;
    if let Ok(np) = &res {
        state.put(key, np);
    }
    res
}

/// 章节预取(问题 9):后台抓取指定页入缓存,翻页时命中缓存零等待。
/// 预取经 hlib 闸串行(不与主抓取并发竞争)。
#[tauri::command]
pub(crate) async fn novel_prefetch(
    app: AppHandle,
    state: State<'_, AppState>,
    chapter_id: String,
    page: i64,
) -> Result<(), String> {
    let key = format!("article_{chapter_id}_p{page}");
    if state
        .cached_ttl::<NovelPage>(&key, Duration::from_secs(24 * 3600))
        .is_some()
    {
        return Ok(());
    }
    let handle = app.clone();
    let key_owned = key;
    tauri::async_runtime::spawn(async move {
        let st = handle.state::<AppState>();
        st.hlib_throttle().await;
        if let Ok(np) = sources::novel::fetch_article_page(&handle, &chapter_id, page).await {
            st.put(key_owned, &np);
        }
    });
    Ok(())
}

#[tauri::command]
pub(crate) fn novel_progress(
    series_id: String,
    state: State<'_, AppState>,
) -> Result<Option<(i64, i64)>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::novel_progress(&conn, &series_id)
}

#[tauri::command]
pub(crate) fn novel_set_progress(
    series_id: String,
    chapter_order: i64,
    page: i64,
    title: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::novel_set_progress(&conn, &series_id, chapter_order, page, title.as_deref())
}

fn filter_blacklisted(
    state: &State<'_, AppState>,
    hits: Vec<sources::novel::NovelHit>,
) -> Vec<sources::novel::NovelHit> {
    let keywords: Vec<String> = match state.db.lock() {
        Ok(conn) => db::novel_blacklist_list(&conn)
            .unwrap_or_default()
            .into_iter()
            .map(|e| e.keyword)
            .collect(),
        Err(_) => Vec::new(),
    };
    if keywords.is_empty() {
        return hits;
    }
    let lower_kw: Vec<String> = keywords.iter().map(|k| k.to_lowercase()).collect();
    hits.into_iter()
        .filter(|h| {
            let title = h.title.to_lowercase();
            !lower_kw.iter().any(|k| title.contains(k))
        })
        .collect()
}

// ── 关注作者(批次 2 契约:复用 follow_target 表,kind=author/source=hlib) ──
// novel_followed_authors/novel_followed_author_works 已在批次 2 实施(见上),
// 此处补写接口两件(需求 1:frontend 调用 novel_follow_author 报命令不存在)

#[tauri::command]
pub(crate) fn novel_follow_author(
    author_id: String,
    author_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::follow_target_add(&conn, "hlib", "author", &author_id, &author_name)
}

#[tauri::command]
pub(crate) fn novel_unfollow_author(
    author_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::follow_target_remove(&conn, "hlib", "author", &author_id)
}
