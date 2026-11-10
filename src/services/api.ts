import { invoke } from '@tauri-apps/api/core'
import type { BikaRankTT, SourceId } from '@/config/sources'
import type { AuthorFollow, BikaAccountStatus, BlacklistEntry, CategoryListResult, ComicDetail, ComicImagesResult, ComicItem, FavoriteEntry, FollowTargetEntry, FollowTargetItem, FollowedAuthor, ItemHistoryEntry, NovelBlacklistEntry, NovelHit, NovelLatestPage, NovelPage, NovelSearchPage, NovelSeries, SearchParams, SearchResult, VideoDetail, VideoListResult, ViewHistoryEntry } from '../types'

export const fetchCategoryList = (source: SourceId, category: string, page: number) => invoke<CategoryListResult>('fetch_category', { source, category, page })
export const fetchBikaCategories = () => invoke<string[]>('fetch_bika_categories')
export const fetchBikaLeaderboard = (tt: BikaRankTT) => invoke<CategoryListResult>('fetch_bika_leaderboard', { tt })
export const searchComics = (source: SourceId, params: SearchParams, page = 0, cacheBuster?: string) => invoke<SearchResult>('search_comics', { source, keyword: params.keyword, classid: params.classid, show: params.show || 'title,text,keyboard,ftitle', page, cacheBuster })
export const fetchComicDetail = (source: SourceId, categoryId: number, id: string) => invoke<ComicDetail>('fetch_comic', { source, categoryId, id })
export const fetchComicImages = (source: SourceId, categoryId: number, id: string, epOrder = 1, page = 0) => invoke<ComicImagesResult>('fetch_comic_images', { source, categoryId, id, epOrder, page })
export const fetchImage = (url: string, source: SourceId = 'niacg') => invoke<ArrayBuffer>('fetch_image', { url, source })
export const fetchVideoImage = (url: string) => invoke<ArrayBuffer>('fetch_video_image', { url })
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
export const gatewayOpen = (label: string) => invoke<void>('gateway_open', { label })
export const fetchVideoList = (page = 1) => invoke<VideoListResult>('fetch_video_list', { page })
export const searchVideos = (keyword: string, page = 1) => invoke<VideoListResult>('search_videos', { keyword, page })
export const fetchVideoDetail = (code: string) => invoke<VideoDetail>('fetch_video_detail', { code })
export const novelAdd = (input: string) => invoke<NovelSeries>('novel_add', { input })
export const novelSeriesInfo = (id: string) => invoke<NovelSeries>('novel_series_info', { id })
export const novelSearch = (keyword: string) => invoke<NovelHit[]>('novel_search', { keyword })
export const novelSearchPage = (
  keyword: string,
  page: number,
  sort?: 'score' | 'date' | 'total' | 'char' | 'like',
) => invoke<NovelSearchPage>('novel_search_page', { keyword, page, sort })
export const novelRecommend = (page?: number) =>
  invoke<NovelLatestPage>('novel_recommend', { page })
export const novelRanking = (period: 'day' | 'week' | 'month', page?: number) =>
  invoke<NovelLatestPage>('novel_ranking', { period, page })
export const novelFollowAuthor = (authorId: string, authorName: string) =>
  invoke<void>('novel_follow_author', { authorId, authorName })
export const novelUnfollowAuthor = (authorId: string) =>
  invoke<void>('novel_unfollow_author', { authorId })
export const novelFollowedAuthors = () =>
  invoke<AuthorFollow[]>('novel_followed_authors')
export const novelFollowedAuthorWorks = (authorId: string) =>
  invoke<NovelHit[]>('novel_followed_author_works', { authorId })
export const novelBlacklistList = () =>
  invoke<NovelBlacklistEntry[]>('novel_blacklist_list')
export const novelBlacklistAdd = (keyword: string) =>
  invoke<void>('novel_blacklist_add', { keyword })
export const novelBlacklistRemove = (keyword: string) =>
  invoke<void>('novel_blacklist_remove', { keyword })
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
export const videoUnfollow = (source: string, kind: string, targetId: string) =>
  invoke<void>('video_unfollow', { source, kind, targetId })
export const videoFollowList = (source: string, kind: string) =>
  invoke<FollowTargetEntry[]>('video_follow_list', { source, kind })
export const videoFollowTargets = (source: string, kind: string) =>
  invoke<FollowTargetItem[]>('video_follow_targets', { source, kind })
export const videoFollowWorks = (
  source: string,
  kind: string,
  targetId: string,
  page?: number,
) => invoke<VideoListResult>('video_follow_works', { source, kind, targetId, page })
export const novelLatest = (page?: number) =>
  invoke<NovelLatestPage>('novel_latest', { page })
export const novelShelf = () => invoke<NovelSeries[]>('novel_shelf')
export const novelRemove = (id: string) => invoke<void>('novel_remove', { id })
export const novelRead = (chapterId: string, page: number) => invoke<NovelPage>('novel_read', { chapterId, page })
export const novelPrefetch = (chapterId: string, page: number) => invoke<void>('novel_prefetch', { chapterId, page })
export const novelProgress = (seriesId: string) => invoke<[number, number] | null>('novel_progress', { seriesId })
export const novelSetProgress = (seriesId: string, chapterOrder: number, page: number, title?: string) => invoke<void>('novel_set_progress', { seriesId, chapterOrder, page, title })
export const recordVideoWatched = (code: string) => invoke<void>('record_video_watched', { code })
export const fetchVideoWatchedIds = (codes: string[]) => invoke<string[]>('fetch_video_watched_ids', { codes })
export const fetchComicViewedIds = (source: SourceId, ids: string[]) => invoke<string[]>('fetch_comic_viewed_ids', { source, ids })
export const gatewayFinish = (label: string) => invoke<void>('gateway_finish', { label })
export const diagRead = () => invoke<string>('diag_read')
export const diagClear = () => invoke<void>('diag_clear')
