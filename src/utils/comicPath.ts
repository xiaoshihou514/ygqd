import type { ComicItem, ViewHistoryEntry } from '@/types'

type PathLike = Pick<ComicItem, 'source' | 'categoryId' | 'id'>

/** 漫画详情路由路径(与 router 的 /comic/:source/:categoryId/:id 对应) */
export function comicPath(item: PathLike): string {
  return `/comic/${item.source}/${item.categoryId}/${item.id}`
}

export function historyEntryPath(entry: ViewHistoryEntry): string {
  return comicPath({ source: entry.source, categoryId: entry.categoryId, id: entry.comicId })
}
