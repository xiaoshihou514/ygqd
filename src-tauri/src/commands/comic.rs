//! comic 域 command(重构笔 3:按源分组)

use crate::db;
use crate::models::*;
use crate::sources;
use crate::sources::bika::BikaApi;
use crate::AppState;
use tauri::State;

/// niacg 漫画源地址
const BASE: &str = "https://www.niacg.com";

#[tauri::command]
pub(crate) async fn fetch_category(
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
pub(crate) async fn fetch_bika_categories(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
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

/// 哔咔排行榜;tt 为 "H24"(日)/"D7"(周)/"D30"(月)
#[tauri::command]
pub(crate) async fn fetch_bika_leaderboard(
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
pub(crate) async fn search_comics(
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
pub(crate) async fn fetch_comic(
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
pub(crate) async fn fetch_comic_images(
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
pub(crate) async fn bika_login(
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
pub(crate) async fn bika_logout(state: State<'_, AppState>) -> Result<(), String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::clear_bika_account(&conn)?;
    }
    *state.bika_token.lock().map_err(|e| e.to_string())? = None;
    state.clear_cache();
    Ok(())
}

#[tauri::command]
pub(crate) fn bika_status(state: State<'_, AppState>) -> Result<BikaAccountStatus, String> {
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
pub(crate) async fn fetch_bika_addresses(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    state.bika_snapshot().fetch_addresses().await
}

/// 哔咔漫画点赞/取消点赞(toggle)
#[tauri::command]
pub(crate) async fn bika_switch_like(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let bika = state.bika_snapshot();
    state.bika_call(|token| bika.switch_like(token, &id)).await
}

/// 哔咔漫画收藏/取消收藏(toggle)
#[tauri::command]
pub(crate) async fn bika_switch_favourite(
    id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let bika = state.bika_snapshot();
    state
        .bika_call(|token| bika.switch_favourite(token, &id))
        .await
}

#[tauri::command]
pub(crate) fn fetch_favorites(state: State<'_, AppState>) -> Result<Vec<FavoriteEntry>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::favorites(&conn)
}

#[tauri::command]
pub(crate) fn is_favorite(
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
pub(crate) async fn add_favorite(
    entry: RecordFavorite,
    state: State<'_, AppState>,
) -> Result<(), String> {
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
pub(crate) async fn remove_favorite(
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
pub(crate) async fn sync_bika_favorites(state: State<'_, AppState>) -> Result<usize, String> {
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
pub(crate) fn get_bika_address(state: State<'_, AppState>) -> Result<String, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::bika_address(&conn)
}

/// 设置分流地址并重建哔咔客户端;空字符串表示恢复系统 DNS
#[tauri::command]
pub(crate) async fn set_bika_address(
    address: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
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

/// 批量查询漫画已看集合(入集中已看的 id,基于浏览历史)
#[tauri::command]
pub(crate) fn fetch_comic_viewed_ids(
    source: Source,
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::comic_viewed_ids(&conn, source, &ids)
}
