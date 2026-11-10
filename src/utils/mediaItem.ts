/**
 * 三板块统一条目模型(MediaCard / MediaCollection 唯一入参形态)。
 *
 * 漫画/视频/小说/历史/收藏的数据源各不相同,全部经本文件的适配函数转换后再交给
 * 卡片——板块差异(封面代理来源、中字/无码角标、小说无封面占位、历史进度文案、
 * 漫画路由四段)只在这里落定,卡片与容器不再按数据源分支。
 */
import type { RouteLocationRaw } from 'vue-router'
import type { SourceId } from '@/config/sources'
import type {
  ComicItem,
  FavoriteEntry,
  NovelHit,
  NovelSeries,
  VideoCard,
  ViewHistoryEntry,
} from '@/types'
import type { TimelineEntry } from '@/composables/useTimeline'
import { comicPath } from '@/utils/comicPath'
import { parseLikes } from '@/utils/likes'
import { videoBaseCode } from '@/utils/videoCode'

export type MediaKind = 'comic' | 'video' | 'novel'

/** 角标色调:accent=强调(分类/榜单序号),cs=中字,leak=无码,neutral=中性 */
export type MediaBadgeTone = 'accent' | 'cs' | 'leak' | 'neutral'

export interface MediaBadge {
  text: string
  tone: MediaBadgeTone
}

export interface MediaCover {
  src: string
  /** 图片代理通道:漫画按来源站,视频走 fourhoi 无 Referer 通道 */
  source: SourceId | 'video'
}

/** 跳转去处:应用内路由与外链至少一个(杜绝可点击却无去处的卡片) */
export type MediaTarget =
  | { to: RouteLocationRaw; href?: string }
  | { to?: RouteLocationRaw; href: string }

interface MediaItemBase {
  /** 列表 key(同一列表内唯一) */
  key: string
  kind: MediaKind
  title: string
  /** 副标题:视频=基础番号;小说/漫画=作者 */
  subtitle?: string
  /** 封面;缺失时卡片按 kind 显示占位(小说书本图标 + 标题) */
  cover?: MediaCover
  /** 封面角标(分类/中字/无码/榜单序号) */
  badges?: MediaBadge[]
  /** 内容标签(卡片最多显示 3 个) */
  tags?: string[]
  /** 计数(展示统一走 formatCount 中文格式) */
  stats?: { likes?: number; views?: number; favorites?: number; chapters?: number }
  /** 时长(fs1 有;missav 列表无;视频卡片封面遮罩展示) */
  duration?: string
  /** 附加说明(字数/发布日期等站点原文),与计数同一行以「 · 」连接 */
  extra?: string[]
  /** 进度文案(「读到第 3 章 第 2 页」「看到 12 分 5 秒」) */
  progress?: string
  /** 时间戳(历史/收藏;行式布局显示) */
  time?: number
  /** 已看/已读(右上角独立标记,不并入 badges) */
  viewed?: boolean
  /** 漫画长按快速拉黑的候选标签(为空则不启用长按) */
  blacklistTags?: string[]
}

export type MediaItem = MediaItemBase & MediaTarget

// ───────── 漫画 ─────────

export function fromComic(c: ComicItem, opts: { viewed?: boolean } = {}): MediaItem {
  return {
    key: `comic:${c.source}:${c.categoryId}:${c.id}`,
    kind: 'comic',
    title: c.title,
    subtitle: c.author || undefined,
    cover: c.thumbnail ? { src: c.thumbnail, source: c.source } : undefined,
    badges: c.category ? [{ text: c.category, tone: 'neutral' }] : undefined,
    tags: c.tags,
    stats: c.likes ? { likes: parseLikes(c.likes) } : undefined,
    viewed: opts.viewed,
    blacklistTags: c.tags.length ? c.tags : undefined,
    // 路由四段缺一即 404:categoryId 由数据源原样带入
    to: comicPath(c),
    href: comicPath(c),
  }
}

/** 漫画浏览历史(存量 view_history) */
export function fromComicHistory(h: ViewHistoryEntry): MediaItem {
  const path = comicPath({ source: h.source, categoryId: h.categoryId, id: h.comicId })
  return {
    key: `comic:${h.source}:${h.categoryId}:${h.comicId}`,
    kind: 'comic',
    title: h.title,
    subtitle: h.author || undefined,
    cover: h.thumbnail ? { src: h.thumbnail, source: h.source } : undefined,
    time: h.viewedAt,
    to: path,
    href: path,
  }
}

/** 漫画收藏 */
export function fromFavorite(f: FavoriteEntry): MediaItem {
  const path = comicPath({ source: f.source, categoryId: f.categoryId ?? 9, id: f.comicId })
  return {
    key: `fav:${f.source}:${f.comicId}`,
    kind: 'comic',
    title: f.title,
    subtitle: f.author || undefined,
    cover: f.thumbnail ? { src: f.thumbnail, source: f.source } : undefined,
    time: f.createdAt,
    to: path,
    href: path,
  }
}

// ───────── 视频 ─────────

/** 视频封面(fourhoi 规律构造;与后端 parse_missav_search 同源公式) */
export function videoCoverUrl(code: string): string {
  return `https://fourhoi.com/${code}/cover-t.jpg`
}

function videoBadges(code: string, chinese?: boolean): MediaBadge[] {
  const badges: MediaBadge[] = []
  if (code.includes('-uncensored-leak')) badges.push({ text: '无码', tone: 'leak' })
  if (chinese || code.includes('-chinese-subtitle')) badges.push({ text: '中字', tone: 'cs' })
  return badges
}

export function fromVideo(v: VideoCard, opts: { watched?: boolean } = {}): MediaItem {
  const base = videoBaseCode(v.code)
  const views = parseLikes(v.views)
  return {
    key: `video:${v.code}`,
    kind: 'video',
    title: v.title || base,
    subtitle: base,
    cover: { src: v.thumbnail || videoCoverUrl(v.code), source: 'video' },
    badges: videoBadges(v.code, v.chinese),
    stats: views ? { views } : undefined,
    duration: v.duration || undefined,
    viewed: opts.watched,
    to: `/videos/${v.code}`,
  }
}

// ───────── 小说 ─────────

/**
 * open:点击去向。发现流/榜单直接进阅读器(任务 7);搜索/关注/历史进系列详情
 * (两种去向是现有产品行为,统一卡片不改变)
 */
export function fromNovelHit(
  h: NovelHit,
  opts: { rank?: number; open?: 'reader' | 'series' } = {},
): MediaItem {
  const views = h.views ? parseLikes(h.views) : 0
  const favorites = h.favorites ? parseLikes(h.favorites) : 0
  const extra = [h.wordCount, h.publishedDate].filter((x): x is string => !!x)
  return {
    key: `novel:${h.id}`,
    kind: 'novel',
    title: h.title,
    subtitle: h.author || undefined,
    badges: opts.rank ? [{ text: `#${opts.rank}`, tone: 'accent' }] : undefined,
    tags: h.tags,
    stats: views || favorites ? { views: views || undefined, favorites: favorites || undefined } : undefined,
    extra: extra.length ? extra : undefined,
    to: opts.open === 'reader' ? `/novels/${h.id}` : `/novels/series/${h.id}`,
  }
}

/** 书架/历史里的系列(progress=[章节序, 页码]) */
export function fromNovelSeries(
  s: NovelSeries,
  progress?: [number, number] | null,
): MediaItem {
  return {
    key: `novel:${s.id}`,
    kind: 'novel',
    title: s.title,
    subtitle: s.author || undefined,
    // hlib 不提供系列封面(cover 恒为空),卡片走书本图标占位
    stats: s.chapters.length ? { chapters: s.chapters.length } : undefined,
    progress: progress ? `读到第 ${progress[0]} 章` : undefined,
    to: `/novels/series/${s.id}`,
  }
}

// ───────── 全局历史时间线 ─────────

/** 历史进度 JSON → 文案(损坏忽略) */
export function timelineProgressText(e: Pick<TimelineEntry, 'kind' | 'progress'>): string {
  if (!e.progress) return ''
  try {
    if (e.kind === 'novel') {
      const p = JSON.parse(e.progress) as { chapter?: number; page?: number }
      return [p.chapter ? `读到第 ${p.chapter} 章` : '', p.page ? `第 ${p.page} 页` : '']
        .filter(Boolean)
        .join(' ')
    }
    if (e.kind === 'video') {
      const pos = (JSON.parse(e.progress) as { position?: number }).position
      if (typeof pos === 'number' && pos > 0) {
        return `看到 ${Math.floor(pos / 60)} 分 ${Math.floor(pos % 60)} 秒`
      }
    }
  } catch {
    /* progress JSON 损坏忽略 */
  }
  return ''
}

const KIND_LABEL: Record<MediaKind, string> = { comic: '漫画', video: '视频', novel: '小说' }

/** kindBadge:混排时间线显示「漫画/视频/小说」角标;单板块分组里可关掉 */
export function fromTimeline(e: TimelineEntry, opts: { kindBadge?: boolean } = {}): MediaItem {
  let to: RouteLocationRaw
  let cover: MediaCover | undefined
  if (e.kind === 'comic') {
    to = comicPath({ source: e.source as SourceId, categoryId: e.categoryId ?? 9, id: e.itemId })
    cover = e.thumbnail ? { src: e.thumbnail, source: e.source as SourceId } : undefined
  } else if (e.kind === 'video') {
    to = `/videos/${e.itemId}`
    cover = { src: e.thumbnail || videoCoverUrl(e.itemId), source: 'video' }
  } else {
    to = `/novels/series/${e.itemId}`
  }
  return {
    key: `${e.kind}:${e.source}:${e.itemId}:${e.viewedAt}`,
    kind: e.kind,
    title: e.title,
    subtitle: e.kind === 'video' ? videoBaseCode(e.itemId) : undefined,
    cover,
    badges: opts.kindBadge === false ? undefined : [{ text: KIND_LABEL[e.kind], tone: 'neutral' }],
    progress: timelineProgressText(e) || undefined,
    time: e.viewedAt,
    to,
  }
}
