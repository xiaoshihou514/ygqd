import { invoke } from '@tauri-apps/api/core'
import type { BlacklistEntry, ComicItem, ComicDetail, ComicMetadata, HomeSection, PaginationInfo, SearchParams, SearchResult, FollowedAuthor, ViewHistoryEntry } from '../types'

export const fetchHomeRecommendations = () => invoke<HomeSection[]>('fetch_home')
export const fetchCategoryList = (category: number, page: number) => invoke<{ items: ComicItem[]; pagination: PaginationInfo }>('fetch_category', { category, page })
export const searchComics = (params: SearchParams, page = 0, cacheBuster?: string) => invoke<SearchResult>('search_comics', { keyword: params.keyword, classid: params.classid, show: params.show || 'title,text,keyboard,ftitle', page, cacheBuster })
export const fetchComicDetail = (categoryId: number, id: string) => invoke<ComicDetail>('fetch_comic', { categoryId, id })
export const fetchComicMetadata = (categoryId: number, id: string) => invoke<ComicMetadata>('fetch_comic_metadata', { categoryId, id })
export const fetchFollowedAuthors = () => invoke<FollowedAuthor[]>('fetch_follows')
export const followAuthor = (author: string) => invoke<FollowedAuthor>('follow_author', { author })
export const unfollowAuthor = (author: string) => invoke<void>('unfollow_author', { author })
export const fetchViewHistory = (limit = 50) => invoke<ViewHistoryEntry[]>('fetch_history', { limit })
export const recordViewHistory = (entry: { comicId: string; title: string; thumbnail: string; categoryId: number; author: string }) => invoke<void>('record_history', { entry })
export const fetchBlacklist = () => invoke<BlacklistEntry[]>('fetch_blacklist')
export const addBlacklistEntry = (tag: string, mode: string) => invoke<BlacklistEntry>('add_blacklist', { tag, mode })
export const removeBlacklistEntry = (tag: string) => invoke<void>('remove_blacklist', { tag })
export const updateBlacklistEntry = (tag: string, mode: string) => invoke<void>('update_blacklist', { tag, mode })
export const fetchImage = (url: string) => invoke<ArrayBuffer>('fetch_image', { url })
