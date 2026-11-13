import { computed, type Ref } from 'vue'
import { useRouter } from 'vue-router'
import type { ItemHistoryEntry, ViewHistoryEntry } from '@/types'

/** 时间线条目统一形态(漫画存量转换 + item_history,小说板块删除后仅两板块) */
export interface TimelineEntry {
  kind: 'comic' | 'video'
  source: string
  itemId: string
  title: string
  viewedAt: number
  /** 封面(仅漫画存量有;视频历史无封面数据,展示 kind 图标占位) */
  thumbnail?: string
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

  /** 漫画分组网格数据(ComicGrid 形态适配) */
  const comicGridItems = computed(() =>
    history.value.map((h) => ({
      source: h.source,
      id: h.comicId,
      title: h.title,
      thumbnail: h.thumbnail,
      category: '',
      categoryId: h.categoryId,
      tags: [] as string[],
      likes: '',
      link: '',
    })),
  )

  /** 两板块时间线混排:viewedAt 降序交错 */
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
    return [...comicEntries, ...videoEntries].sort((a, b) => b.viewedAt - a.viewedAt)
  })

  const videoEntries = computed(() => timeline.value.filter((e) => e.kind === 'video'))

  /** 视频条目的播放位置(progress JSON 解析) */
  function entryPosition(e: TimelineEntry): number | undefined {
    if (!e.progress) return undefined
    try {
      const pos: unknown = (JSON.parse(e.progress) as { position?: unknown } | null)?.position
      return typeof pos === 'number' && Number.isFinite(pos) && pos >= 0 ? pos : undefined
    } catch {
      return undefined
    }
  }

  /** 时间线条目跳转:视频→详情/漫画→详情(存量字段) */
  function openTimelineEntry(entry: TimelineEntry) {
    if (entry.kind === 'video') {
      router.push(`/videos/${entry.itemId}`)
    } else {
      router.push(`/comic/${entry.source}/${entry.categoryId ?? 9}/${entry.itemId}`)
    }
  }

  function kindLabel(kind: string): string {
    return kind === 'comic' ? '漫画' : '视频'
  }

  function formatPosition(seconds: number): string {
    if (!Number.isFinite(seconds) || seconds < 0) seconds = 0
    const m = Math.floor(seconds / 60)
    const s = Math.floor(seconds % 60)
    return `看到 ${m} 分 ${s} 秒`
  }

  return {
    comicGridItems,
    timeline,
    videoEntries,
    entryPosition,
    openTimelineEntry,
    kindLabel,
    formatPosition,
  }
}
