import { onScopeDispose, ref, watch, type Ref } from 'vue'
import { fetchComicViewedIds } from '@/services/api'
import type { ComicItem, SourceId } from '@/types'

/**
 * 批量查询列表中已读的漫画(基于浏览历史),返回 "source:id" 集合。
 * 列表频繁变化时 400ms 防抖后查询;各源并发查询,过期结果丢弃。
 */
export function useComicViewedIds(items: Ref<ComicItem[]>) {
  const viewedIds = ref(new Set<string>())
  let timer: ReturnType<typeof setTimeout> | null = null
  let generation = 0

  function cancelPending() {
    if (timer) clearTimeout(timer)
    timer = null
    generation++
  }

  async function query(list: readonly ComicItem[], current: number) {
    const bySource = new Map<SourceId, Set<string>>()
    for (const it of list) {
      let ids = bySource.get(it.source)
      if (!ids) bySource.set(it.source, (ids = new Set()))
      ids.add(it.id)
    }
    const results = await Promise.all(
      [...bySource].map(async ([source, ids]) => {
        try {
          return { source, viewed: await fetchComicViewedIds(source, [...ids]) }
        } catch {
          return { source, viewed: [] as string[] } // 查询失败不影响列表展示
        }
      }),
    )
    if (current !== generation) return
    const next = new Set<string>()
    for (const { source, viewed } of results) {
      for (const id of viewed) next.add(`${source}:${id}`)
    }
    viewedIds.value = next
  }

  watch(items, (list) => {
    cancelPending()
    if (!list.length) {
      if (viewedIds.value.size) viewedIds.value = new Set()
      return
    }
    const current = generation
    timer = setTimeout(() => {
      timer = null
      void query(list, current)
    }, 400)
  })

  onScopeDispose(cancelPending)

  return viewedIds
}
