import { computed, type Ref } from 'vue'
import { useRouter } from 'vue-router'
import type { ItemHistoryEntry, ViewHistoryEntry } from '@/types'

/** 时间线条目统一形态(漫画存量转换 + item_history,重构笔 4 迁入) */
export interface TimelineEntry {
  kind: 'comic' | 'video' | 'novel'
  source: string
  itemId: string
  title: string
  viewedAt: number
  /** 封面(仅漫画存量有;视频/小说历史无封面数据,展示 kind 图标占位) */
  thumbnail?: string
  /** 小说阅读进度(progress JSON 解析) */
  chapter?: number
  /** 视频/小说的进度 JSON 原文(视频含播放位置秒数) */
  progress?: string
  /** 漫画详情跳转用(存量字段) */
  categoryId?: number
}

export function useTimeline(
  history: Ref<ViewHistoryEntry[]>,
  itemHistory: Ref<ItemHistoryEntry[]>,
) {
  const router = useRouter()

  /** 三板块时间线混排:viewedAt 降序交错 */
  const timeline = computed<TimelineEntry[]>(() => {
    const comicEntries: TimelineEntry[] = history.value.map((h) => ({
      kind: 'comic',
      source: h.source,
      itemId: h.comicId,
      title: h.title,
      viewedAt: h.viewedAt,
      thumbnail: h.thumbnail,
      categoryId: h.categoryId,
    }))
    const novelEntries: TimelineEntry[] = itemHistory.value
      .filter((e) => e.kind === 'novel')
      .map((e) => {
        let chapter: number | undefined
        try {
          const p = e.progress ? (JSON.parse(e.progress) as { chapter?: number }) : undefined
          chapter = p?.chapter
        } catch {
          /* progress JSON 损坏忽略 */
        }
        return {
          kind: 'novel' as const,
          source: e.source,
          itemId: e.itemId,
          title: e.title,
          viewedAt: e.viewedAt,
          chapter,
          progress: e.progress,
        }
      })
    const videoEntries: TimelineEntry[] = itemHistory.value
      .filter((e) => e.kind === 'video')
      .map((e) => ({
        kind: 'video' as const,
        source: e.source,
        itemId: e.itemId,
        title: e.title,
        viewedAt: e.viewedAt,
        progress: e.progress,
      }))
    return [...comicEntries, ...novelEntries, ...videoEntries].sort(
      (a, b) => b.viewedAt - a.viewedAt,
    )
  })

  const videoEntries = computed(() => timeline.value.filter((e) => e.kind === 'video'))
  const novelEntries = computed(() =>
    timeline.value
      .filter((e) => e.kind === 'novel')
      .map((e) => {
        let page: number | undefined
        try {
          page = e.progress ? (JSON.parse(e.progress) as { page?: number }).page : undefined
        } catch {
          /* progress JSON 损坏忽略 */
        }
        return { ...e, page }
      }),
  )

  /** 视频条目的播放位置(progress JSON 解析) */
  function entryPosition(e: TimelineEntry): number | undefined {
    try {
      return e.progress ? (JSON.parse(e.progress) as { position?: number }).position : undefined
    } catch {
      return undefined
    }
  }

  /** 时间线条目跳转:小说→系列详情/视频→详情/漫画→详情(存量字段) */
  function openTimelineEntry(entry: TimelineEntry) {
    if (entry.kind === 'novel') {
      router.push(`/novels/series/${entry.itemId}`)
    } else if (entry.kind === 'video') {
      router.push(`/videos/${entry.itemId}`)
    } else {
      router.push(`/comic/${entry.source}/${entry.categoryId ?? 9}/${entry.itemId}`)
    }
  }

  function kindLabel(kind: string): string {
    return kind === 'comic' ? '漫画' : kind === 'video' ? '视频' : '小说'
  }

  function formatPosition(seconds: number): string {
    const m = Math.floor(seconds / 60)
    const s = Math.floor(seconds % 60)
    return `看到 ${m} 分 ${s} 秒`
  }

  return {
    timeline,
    videoEntries,
    novelEntries,
    entryPosition,
    openTimelineEntry,
    kindLabel,
    formatPosition,
  }
}
