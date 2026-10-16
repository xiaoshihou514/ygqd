import type { ComicItem } from '@/types'

export function itemKey(item: Pick<ComicItem, 'source' | 'id'>): string {
  return `${item.source}:${item.id}`
}

/** 多个源的同页结果按位置交错合并并去重,保持各源内部顺序 */
export function interleaveSources(lists: readonly ComicItem[][]): ComicItem[] {
  const seen = new Set<string>()
  const maxLength = Math.max(...lists.map((list) => list.length), 0)
  const merged: ComicItem[] = []
  for (let index = 0; index < maxLength; index++) {
    for (const list of lists) {
      const item = list[index]
      if (!item) continue
      const key = itemKey(item)
      if (seen.has(key)) continue
      seen.add(key)
      merged.push(item)
    }
  }
  return merged
}

/** 追加去重:把 incoming 中 target 尚未包含的条目并入(原地修改 target) */
export function appendDistinct(target: ComicItem[], incoming: readonly ComicItem[]): void {
  const seen = new Set(target.map(itemKey))
  for (const item of incoming) {
    const key = itemKey(item)
    if (seen.has(key)) continue
    seen.add(key)
    target.push(item)
  }
}
