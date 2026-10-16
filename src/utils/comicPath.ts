import type { ComicItem, ComicDetail, ViewHistoryEntry } from '@/types'

type PathLike = Pick<ComicItem, 'source' | 'categoryId' | 'id'>

export function comicPath(item: PathLike): string {
  return `/comic/${item.source}/${item.categoryId}/${item.id}`
}

export function historyEntryPath(entry: ViewHistoryEntry): string {
  return `/comic/${entry.source}/${entry.categoryId}/${entry.comicId}`
}
