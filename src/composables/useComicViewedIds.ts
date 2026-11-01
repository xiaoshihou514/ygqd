import { ref, watch, type Ref } from 'vue'
import { fetchComicViewedIds } from '@/services/api'
import type { ComicItem, SourceId } from '@/types'

/**
 * 批量查询列表中已读的漫画(基于浏览历史),返回 "source:id" 集合。
 * 列表频繁变化时 400ms 防抖后查询。
 */
export function useComicViewedIds(items: Ref<ComicItem[]>) {
  const viewedIds = ref(new Set<string>())
  let timer: ReturnType<typeof setTimeout> | null = null
  let generation = 0

  watch(items, (list) => {
    if (!list.length) {
      viewedIds.value = new Set()
      return
    }
    if (timer) clearTimeout(timer)
    const current = ++generation
    timer = setTimeout(async () => {
      const bySource = new Map<SourceId, string[]>()
      for (const it of list) {
        const arr = bySource.get(it.source) ?? []
        arr.push(it.id)
        bySource.set(it.source, arr)
      }
      const next = new Set<string>()
      for (const [source, ids] of bySource) {
        try {
          const viewed = await fetchComicViewedIds(source, ids)
          for (const id of viewed) next.add(`${source}:${id}`)
        } catch {
          // 查询失败不影响列表展示
        }
      }
      if (current === generation) viewedIds.value = next
    }, 400)
  })

  return viewedIds
}
