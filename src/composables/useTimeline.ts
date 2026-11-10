import { computed, type Ref } from 'vue'
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
        thumbnail: e.thumbnail,
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

  return {
    timeline,
    videoEntries,
    novelEntries,
  }
}
