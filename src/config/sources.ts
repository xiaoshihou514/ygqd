export type SourceId = 'niacg' | 'bika'
export type SearchScope = 'all' | SourceId

export const SOURCE_LABELS: Record<string, string> = {
  niacg: 'ACG',
  bika: '哔咔',
}

export const SEARCH_SOURCE_OPTIONS: { id: SearchScope; label: string }[] = [
  { id: 'all', label: '全部' },
  { id: 'niacg', label: 'ACG' },
  { id: 'bika', label: '哔咔' },
]

export interface HomeTab {
  key: string
  label: string
  source: SourceId
  category: string
}

/** 首页静态 tab;哔咔的分类 tab 在运行时从接口拉取后追加 */
export const HOME_TABS: HomeTab[] = [
  { key: 'niacg:9', label: '漫画', source: 'niacg', category: '9' },
  { key: 'niacg:1', label: 'COS', source: 'niacg', category: '1' },
  { key: 'bika:', label: '哔咔', source: 'bika', category: '' },
]

export function sourceLabel(source: string): string {
  return SOURCE_LABELS[source] ?? source
}
