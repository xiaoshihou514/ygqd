//! comic 域 command(重构笔 3:按源分组)
//!
//! 只读抓取类 command 统一经 `state.cache.get_or_fetch`:命中 300 秒缓存
//! 直接返回,相同参数的并发请求合并为一次上游请求。
//! 纯数据库 command 标注 `async`,由 tokio 线程池执行而非主线程。

use crate::models::*;
use crate::sources::bika::BikaApi;
use crate::{db, fetch_image_with_client, lock, reqwest_error, sources, AppState, BASE};
use tauri::ipc::Response;
use tauri::State;

/// 单页结果(niacg 图片页一次给全)
const SINGLE_PAGE: PaginationInfo = PaginationInfo {
    current: 0,
    total: 1,
    has_next: false,
    has_prev: false,
};

/// 封面/图片代理:按源选用带对应请求头的客户端
#[tauri::command]
pub(crate) async fn fetch_image(
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

#[tauri::command]
pub(crate) async fn fetch_category(
    source: Source,
    category: String,
    page: i32,
    state: State<'_, AppState>,
) -> Result<CategoryListResult, String> {
    let key = format!("{source}:list-{category}-{page}");
    state
        .cache
        .get_or_fetch(key, async {
            Ok(match source {
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
            })
        })
        .await
}

#[tauri::command]
pub(crate) async fn fetch_bika_categories(
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let bika = state.bika_snapshot();
    state
        .cache
        .get_or_fetch(
            "bika:categories".into(),
            state.bika_call(|token| bika.categories(token)),
        )
        .await
}

/// 哔咔排行榜;tt 为 "H24"(日)/"D7"(周)/"D30"(月)
#[tauri::command]
pub(crate) async fn fetch_bika_leaderboard(
    tt: String,
    state: State<'_, AppState>,
) -> Result<CategoryListResult, String> {
    let bika = state.bika_snapshot();
    state
        .cache
        .get_or_fetch(
            format!("bika:leaderboard-{tt}"),
            state.bika_call(|token| bika.leaderboard(token, &tt)),
        )
        .await
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
    state
        .cache
        .get_or_fetch(key, async {
            let mut out = match source {
                Source::Bika => search_bika(&state, &keyword, &show, page).await?,
                Source::Niacg => search_niacg(&state, keyword, classid, show, page).await?,
            };
            out.pagination.current = page;
            out.pagination.has_prev = page > 0;
            Ok(out)
        })
        .await
}

async fn search_bika(
    state: &AppState,
    keyword: &str,
    show: &str,
    page: i32,
) -> Result<SearchResult, String> {
    let bika = state.bika_snapshot();
    let page = page + 1;
    let result = match show {
        "tags" => {
            state
                .bika_call(|token| bika.comics(token, "", keyword, "", page))
                .await?
        }
        "writer" => {
            state
                .bika_call(|token| bika.comics(token, "", "", keyword, page))
                .await?
        }
        _ => {
            state
                .bika_call(|token| bika.search(token, keyword, page))
                .await?
        }
    };
    Ok(SearchResult {
        items: result.items,
        pagination: result.pagination,
        page_url_template: None,
    })
}

async fn search_niacg(
    state: &AppState,
    keyword: String,
    classid: i32,
    show: String,
    page: i32,
) -> Result<SearchResult, String> {
    // 全局限速:niacg 搜索间隔不足 5 秒会返回无结果的提示页;
    // 放行权持有到本次搜索(含翻页请求)结束
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
        return Ok(r);
    }
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
        .and_then(reqwest::Response::error_for_status)
        .map_err(reqwest_error)?
        .text()
        .await
        .map_err(reqwest_error)?;
    let first = sources::niacg::search(&html);
    let total_pages = first.pagination.total + 1;
    if page == 0 {
        return Ok(SearchResult {
            pagination: PaginationInfo {
                current: 0,
                total: total_pages,
                has_next: total_pages > 1,
                has_prev: false,
            },
            ..first
        });
    }
    let template = first
        .page_url_template
        .ok_or("搜索结果分页地址缺失,请回到第一页重试")?;
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
    Ok(result)
}

#[tauri::command]
pub(crate) async fn fetch_comic(
    source: Source,
    category_id: i32,
    id: String,
    state: State<'_, AppState>,
) -> Result<ComicDetail, String> {
    let key = format!("{source}:detail-{category_id}-{id}");
    state
        .cache
        .get_or_fetch(key, async {
            Ok(match source {
                Source::Bika => {
                    // 详情与章节互不依赖,并发请求省一个往返
                    let bika = state.bika_snapshot();
                    let (detail, episodes) = tokio::join!(
                        state.bika_call(|token| bika.detail(token, &id)),
                        state.bika_call(|token| bika.episodes(token, &id)),
                    );
                    let mut detail = detail?;
                    detail.episodes = episodes?;
                    detail
                }
                Source::Niacg => sources::niacg::detail(
                    &state
                        .niacg_get(&format!("/moehome-{category_id}-{id}.html"))
                        .await?,
                    category_id,
                    id.clone(),
                ),
            })
        })
        .await
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
            let images = state
                .cache
                .get_or_fetch(format!("niacg:images-{category_id}-{id}"), async {
                    Ok(sources::niacg::images(
                        &state
                            .niacg_get(&format!("/moeupup-{category_id}-{id}.html"))
                            .await?,
                    ))
                })
                .await?;
            Ok(ComicImagesResult {
                images,
                pagination: SINGLE_PAGE,
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
    state.with_db(|c| db::save_bika_account(c, &email, &password, &token))?;
    *lock(&state.bika_token) = Some(token);
    state.cache.clear();
    Ok(BikaAccountStatus {
        logged_in: true,
        email,
    })
}

#[tauri::command(async)]
pub(crate) fn bika_logout(state: State<'_, AppState>) -> Result<(), String> {
    state.with_db(db::clear_bika_account)?;
    *lock(&state.bika_token) = None;
    state.cache.clear();
    Ok(())
}

#[tauri::command(async)]
pub(crate) fn bika_status(state: State<'_, AppState>) -> Result<BikaAccountStatus, String> {
    Ok(match state.with_db(db::bika_account)? {
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

#[tauri::command(async)]
pub(crate) fn fetch_favorites(state: State<'_, AppState>) -> Result<Vec<FavoriteEntry>, String> {
    state.with_db(db::favorites)
}

#[tauri::command(async)]
pub(crate) fn is_favorite(
    source: Source,
    comic_id: String,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    state.with_db(|c| db::is_favorite(c, source, &comic_id))
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
        let id = &entry.comic_id;
        state
            .bika_call(|token| bika.switch_favourite(token, id))
            .await?;
    }
    state.with_db(|c| db::add_favorite(c, &entry))
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
    state.with_db(|c| db::remove_favorite(c, source, &comic_id))
}

/// 把哔咔服务器端的收藏拉取并合并到本地表(官方 app 收的也能看到);
/// 返回本次同步新增的条数
#[tauri::command]
pub(crate) async fn sync_bika_favorites(state: State<'_, AppState>) -> Result<usize, String> {
    // 只取条数,不必把整表读出来
    let count = || {
        state.with_db(|c| {
            c.query_row("SELECT COUNT(*) FROM favorites", [], |r| r.get::<_, i64>(0))
                .map(|n| n as usize)
                .map_err(|e| e.to_string())
        })
    };
    let before = count()?;
    let bika = state.bika_snapshot();
    let mut page = 1;
    loop {
        let result = state
            .bika_call(|token| bika.favourite_list(token, page))
            .await?;
        // 每页一个事务:逐条自动提交在移动端每条都要落盘一次
        state.with_db(|c| {
            let tx = c.unchecked_transaction().map_err(|e| e.to_string())?;
            for item in &result.items {
                let entry = RecordFavorite {
                    source: Source::Bika,
                    comic_id: item.id.clone(),
                    title: item.title.clone(),
                    thumbnail: item.thumbnail.clone(),
                    category_id: item.category_id,
                    author: item.author.clone(),
                };
                db::add_favorite(&tx, &entry)?;
            }
            tx.commit().map_err(|e| e.to_string())
        })?;
        if !result.pagination.has_next {
            break;
        }
        page += 1;
    }
    Ok(count()?.saturating_sub(before))
}

/// 当前分流地址,空字符串表示不分流
#[tauri::command(async)]
pub(crate) fn get_bika_address(state: State<'_, AppState>) -> Result<String, String> {
    state.with_db(db::bika_address)
}

/// 设置分流地址并重建哔咔客户端;空字符串表示恢复系统 DNS
#[tauri::command(async)]
pub(crate) fn set_bika_address(address: String, state: State<'_, AppState>) -> Result<(), String> {
    let address = address.trim();
    let api = BikaApi::new((!address.is_empty()).then_some(address))?;
    state.with_db(|c| db::save_bika_address(c, address))?;
    *state
        .bika
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = api;
    state.cache.clear();
    Ok(())
}

/// 批量查询漫画已看集合(入集中已看的 id,基于浏览历史)
#[tauri::command(async)]
pub(crate) fn fetch_comic_viewed_ids(
    source: Source,
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    state.with_db(|c| db::comic_viewed_ids(c, source, &ids))
}
