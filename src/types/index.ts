import type { SearchScope, SourceId } from '@/config/sources'

export interface ComicItem {
  source: SourceId
  id: string
  title: string
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

export interface BikaAccountStatus {
  loggedIn: boolean
  email: string
}
