import { invoke, type InvokeArgs } from '@tauri-apps/api/core'
import type { BikaRankTT, SourceId } from '@/config/sources'
import type { BikaAccountStatus, BlacklistEntry, CategoryListResult, ComicDetail, ComicImagesResult, FavoriteEntry, FollowTargetEntry, FollowTargetItem, FollowedAuthor, ItemHistoryEntry, SearchParams, SearchResult, VideoDetail, VideoListResult, ViewHistoryEntry } from '../types'

/**
 * 幂等读命令的在途去重:同一 command + 参数在前一次 IPC 未返回时复用同一 Promise
 * (封面在多个列表/卡片同时挂载、页面快速重入时尤为常见),避免重复抓上游
 * 与重复撞 niacg 搜索限速闸。返回后立即出表——不做结果缓存(缓存由 Rust 侧
 * 300 秒 TTL 负责),因此「重新获取」语义(如视频 403 换签名)不受影响。
 *
 * 复用方拿到的是结构化克隆:页面可能就地修改返回对象(追加、改状态),
 * 不能让多个调用方共享同一引用。ArrayBuffer 只读消费(转 Blob),直接共享。
 */
const inflight = new Map<string, Promise<unknown>>()

function cloneShared<T>(value: T): T {
  if (value === null || typeof value !== 'object' || value instanceof ArrayBuffer) return value
  try {
    return structuredClone(value)
  } catch {
    return value
  }
}

function invokeShared<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  const key = `${cmd}\u0000${args === undefined ? '' : JSON.stringify(args)}`
  const pending = inflight.get(key) as Promise<T> | undefined
  if (pending) return pending.then(cloneShared)
  const request = invoke<T>(cmd, args)
  inflight.set(key, request)
  const release = () => {
    if (inflight.get(key) === request) inflight.delete(key)
  }
  request.then(release, release)
  return request
}

export const fetchCategoryList = (source: SourceId, category: string, page: number) => invokeShared<CategoryListResult>('fetch_category', { source, category, page })
export const fetchBikaCategories = () => invokeShared<string[]>('fetch_bika_categories')
export const fetchBikaLeaderboard = (tt: BikaRankTT) => invokeShared<CategoryListResult>('fetch_bika_leaderboard', { tt })
export const searchComics = (source: SourceId, params: SearchParams, page = 0, cacheBuster?: string) => invokeShared<SearchResult>('search_comics', { source, keyword: params.keyword, classid: params.classid, show: params.show || 'title,text,keyboard,ftitle', page, cacheBuster })
export const fetchComicDetail = (source: SourceId, categoryId: number, id: string) => invokeShared<ComicDetail>('fetch_comic', { source, categoryId, id })
export const fetchComicImages = (source: SourceId, categoryId: number, id: string, epOrder = 1, page = 0) => invokeShared<ComicImagesResult>('fetch_comic_images', { source, categoryId, id, epOrder, page })
export const fetchImage = (url: string, source: SourceId = 'niacg') => invokeShared<ArrayBuffer>('fetch_image', { url, source })
export const fetchVideoImage = (url: string) => invokeShared<ArrayBuffer>('fetch_video_image', { url })
export const bikaLogin = (email: string, password: string) => invoke<BikaAccountStatus>('bika_login', { email, password })
export const bikaLogout = () => invoke<void>('bika_logout')
export const bikaStatus = () => invoke<BikaAccountStatus>('bika_status')
export const fetchBikaAddresses = () => invoke<string[]>('fetch_bika_addresses')
export const getBikaAddress = () => invoke<string>('get_bika_address')
export const setBikaAddress = (address: string) => invoke<void>('set_bika_address', { address })
export const fetchFollowedAuthors = () => invoke<FollowedAuthor[]>('fetch_follows')
export const followAuthor = (author: string) => invoke<FollowedAuthor>('follow_author', { author })
export const unfollowAuthor = (author: string) => invoke<void>('unfollow_author', { author })
export const bikaSwitchLike = (id: string) => invoke<void>('bika_switch_like', { id })
export const bikaSwitchFavourite = (id: string) => invoke<void>('bika_switch_favourite', { id })
export const fetchFavorites = () => invoke<FavoriteEntry[]>('fetch_favorites')
export const isFavorite = (source: SourceId, comicId: string) => invoke<boolean>('is_favorite', { source, comicId })
export const addFavorite = (entry: { source: SourceId; comicId: string; title: string; thumbnail: string; categoryId: number; author: string }) => invoke<void>('add_favorite', { entry })
export const removeFavorite = (source: SourceId, comicId: string) => invoke<void>('remove_favorite', { source, comicId })
export const syncBikaFavorites = () => invoke<number>('sync_bika_favorites')
export const fetchViewHistory = (limit = 50) => invoke<ViewHistoryEntry[]>('fetch_history', { limit })
export const recordViewHistory = (entry: { source: string; comicId: string; title: string; thumbnail: string; categoryId: number; author: string }) => invoke<void>('record_history', { entry })
export const fetchBlacklist = () => invoke<BlacklistEntry[]>('fetch_blacklist')
export const addBlacklistEntry = (tag: string, mode: string) => invoke<BlacklistEntry>('add_blacklist', { tag, mode })
export const removeBlacklistEntry = (tag: string) => invoke<void>('remove_blacklist', { tag })
export const updateBlacklistEntry = (tag: string, mode: string) => invoke<void>('update_blacklist', { tag, mode })
export const fetchVideoList = (page = 1) => invokeShared<VideoListResult>('fetch_video_list', { page })
/** sort 为空/undefined 走各源默认排序;Rust 侧 page 必填(i32),缺省按第 1 页 */
export const searchVideos = (keyword: string, page?: number, sort?: string) => invokeShared<VideoListResult>('search_videos', { keyword, page: page ?? 1, sort: sort || undefined })
export const fetchVideoDetail = (code: string) => invokeShared<VideoDetail>('fetch_video_detail', { code })
export const recordItemHistory = (
  source: string,
  kind: string,
  itemId: string,
  title: string,
  progress?: string,
  thumbnail?: string,
) => invoke<void>('record_item_history', { source, kind, itemId, title, progress, thumbnail })
export const fetchItemHistory = (source?: string, kind?: string, limit?: number) =>
  invoke<ItemHistoryEntry[]>('fetch_item_history', { source, kind, limit })
export const videoFollow = (source: string, kind: string, targetId: string, displayName: string) =>
  invoke<void>('video_follow', { source, kind, targetId, displayName })
/** 小说板块已删除,后端 novel_followed_authors 命令随之移除:恒为空列表,
 *  不再发一次必然失败的 IPC(保留导出供统一备份沿用字段)。 */
export const novelFollowedAuthorsReadonly = (): Promise<{ authorId: string; authorName: string }[]> =>
  Promise.resolve([])
export const videoUnfollow = (source: string, kind: string, targetId: string) =>
  invoke<void>('video_unfollow', { source, kind, targetId })
export const videoFollowList = (source: string, kind: string) =>
  invoke<FollowTargetEntry[]>('video_follow_list', { source, kind })
export const videoFollowTargets = (source: string, kind: string) =>
  invokeShared<FollowTargetItem[]>('video_follow_targets', { source, kind })
export const videoFollowWorks = (
  source: string,
  kind: string,
  targetId: string,
  page?: number,
) => invokeShared<VideoListResult>('video_follow_works', { source, kind, targetId, page })
export const recordVideoWatched = (code: string) => invoke<void>('record_video_watched', { code })
export const fetchVideoWatchedIds = (codes: string[]) =>
  codes.length ? invoke<string[]>('fetch_video_watched_ids', { codes }) : Promise.resolve<string[]>([])
export const fetchComicViewedIds = (source: SourceId, ids: string[]) =>
  ids.length ? invoke<string[]>('fetch_comic_viewed_ids', { source, ids }) : Promise.resolve<string[]>([])
export const diagRead = () => invoke<string>('diag_read')
export const diagClear = () => invoke<void>('diag_clear')
