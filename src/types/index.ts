import type { SearchScope, SourceId } from '@/config/sources'
export type { SourceId, SearchScope }

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
  /** 播放来源:fs1(优先)或 missav(兜底) */
  provider?: string
  /** 中文字幕版本 */
  chinese?: boolean
  /** 发行厂商(missav 详情页解析;fs1 无此元数据为空) */
  studio?: string
  /** 女优名列表(兼容字段,与 actresses 同源) */
  actors?: string[]
  /** 以下来自 missav 元数据区(fs1 线路按同番号从 missav 补齐) */
  makers?: VideoMetaLink[]
  actresses?: VideoMetaLink[]
  /** 男优 */
  maleActors?: VideoMetaLink[]
  series?: VideoMetaLink[]
  directors?: VideoMetaLink[]
  labels?: VideoMetaLink[]
  releaseDate?: string | null
}

/** 影片元数据条目:kind + id 对应 missav 列表页,用于相似影片与关注 */
export interface VideoMetaLink {
  /** maker / actress / actor / series / director / label / genre */
  kind: string
  id: string
  name: string
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
  /** 系列封面 URL(hlib 站点不提供,恒为空串;展示层走占位图标) */
  cover?: string
  /** 简介(来源页面 meta description,可能含站点文案,展示层截断) */
  introduction?: string
  chapters: NovelChapter[]
  addedAt: number
}

export interface NovelHit {
  id: string
  title: string
  /** 作者(登录墙未登录时列表可能拿不到,可空) */
  author?: string
  /** 内容标签(可空,未登录时可能拿不到) */
  tags?: string[]
}

/** 小说搜索分页结果(站点每页 30 条,热门词可达数百页) */
export interface NovelSearchPage {
  hits: NovelHit[]
  page: number
  totalPages: number
}

/** 小说关注作者(后端 novel_followed_authors 返回;authorId 以作者名充当标识) */
export interface AuthorFollow {
  authorId: string
  authorName: string
}

/** 视频关注条目(video_follow_list;follow_target 统一表) */
export interface FollowTargetEntry {
  source: string
  kind: string
  targetId: string
  displayName: string
  followedAt: number
}

/** 可关注目标(video_follow_targets;厂商/女优列表条目) */
export interface FollowTargetItem {
  targetId: string
  displayName: string
  /** 作品数(站点提供则有,可选) */
  workCount?: number
}

/** 发现流最新系列的一页(首页第 1 页不带参数,深页走 ?p=;空页后端抛错终止) */
export interface NovelLatestPage {
  hits: NovelHit[]
  page: number
  totalPages: number
}

/** 全局历史条目(item_history 表;视频/小说写入,漫画走存量 view_history) */
export interface ItemHistoryEntry {
  source: string
  kind: string
  itemId: string
  title: string
  viewedAt: number
  /** 进度 JSON 字符串(小说:{"chapter":N,"page":N};视频:{"position":秒数}) */
  progress?: string
}

/** 小说关键词黑名单条目(novel_blacklist_list;标题子串匹配,按创建倒序) */
export interface NovelBlacklistEntry {
  keyword: string
  createdAt: number
}

export interface NovelPage {
  title: string
  author: string
  text: string
  totalPages: number
}
