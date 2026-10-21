import type { SearchScope, SourceId } from '@/config/sources'

export interface ComicItem {
  source: SourceId
  id: string
  title: string
  /** 列表项可能拿不到作者(niacg 为空) */
  author?: string
  thumbnail: string
  category: string
  categoryId: number
  tags: string[]
  likes: string
  link: string
}

export interface ComicDetail {
  source: SourceId
  id: string
  title: string
  thumbnail: string
  category: string
  categoryId: number
  author: string
  works: string[]
  characters: string[]
  tags: string[]
  likes: string
  episodes: Episode[]
  images: string[]
  /** 哔咔服务器端点赞/收藏状态;niacg 恒 false */
  isLiked?: boolean
  isFavourite?: boolean
}

export interface Episode {
  order: number
  title: string
}

export interface PaginationInfo {
  current: number
  total: number
  hasNext: boolean
  hasPrev: boolean
}

export interface ComicImagesResult {
  images: string[]
  pagination: PaginationInfo
}

export interface SearchParams {
  source: SearchScope
  keyword: string
  classid: number
  show?: string
  tempid?: string
}

export interface CategoryOption {
  id: number
  label: string
}

export interface ParsedTitle {
  author: string
  workName: string
  chapters: number[]
  hasExtra: boolean
  isComplete: boolean
  rawTitle: string
  item: ComicItem
}

export interface SplitGroup {
  author: string
  workName: string
  items: ComicItem[]
  knownChapters: number[]
  missingChapters: number[]
  hasExtra: boolean
  isComplete: boolean
  isStandalone: boolean
}

export interface SearchResult {
  items: ComicItem[]
  pagination: PaginationInfo
  pageUrlTemplate: string | null
}

export interface CategoryListResult {
  items: ComicItem[]
  pagination: PaginationInfo
}

export type BlacklistMode = 'fuzzy' | 'exact' | 'single'

export interface BlacklistEntry {
  tag: string
  mode: BlacklistMode
  createdAt?: number
}

export interface FollowedAuthor {
  author: string
  followedAt: number
  lastCheckedAt: number
}

export interface ViewHistoryEntry {
  source: SourceId
  comicId: string
  title: string
  thumbnail: string
  categoryId: number
  author: string
  viewedAt: number
}

export interface FavoriteEntry {
  source: SourceId
  comicId: string
  title: string
  thumbnail: string
  categoryId: number
  author: string
  createdAt: number
}

export interface BikaAccountStatus {
  loggedIn: boolean
  email: string
}

/** 视频卡片(fs1/Jable 系) */
export interface VideoCard {
  /** 番号(小写),详情 id;中字变体带 -chinese-subtitle 后缀 */
  code: string
  title: string
  thumbnail: string
  views: string
  likes: string
  /** 中文字幕变体 */
  chinese?: boolean
}

export interface VideoListResult {
  items: VideoCard[]
  pagination: PaginationInfo
}

export interface VideoDetail {
  code: string
  title: string
  tags: string[]
  m3u8: string
  /** 播放来源:missav(优先)或 fs1(回退) */
  provider?: string
  /** 中文字幕版本 */
  chinese?: boolean
}

export interface NovelChapter {
  id: string
  order: number
  title: string
}

export interface NovelSeries {
  id: string
  title: string
  author: string
  chapters: NovelChapter[]
  addedAt: number
}

export interface NovelHit {
  id: string
  title: string
}

export interface NovelPage {
  title: string
  author: string
  text: string
  totalPages: number
}
