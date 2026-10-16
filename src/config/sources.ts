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
  source: SourceId | 'mix'
  category: string
}

/** 首页静态 tab;哔咔的分类 tab 在运行时从接口拉取后追加 */
export const HOME_TABS: HomeTab[] = [
  { key: 'mix:', label: '最新', source: 'mix', category: '' },
  { key: 'niacg:9', label: '漫画', source: 'niacg', category: '9' },
  { key: 'niacg:1', label: 'COS', source: 'niacg', category: '1' },
  { key: 'bika:', label: '哔咔', source: 'bika', category: '' },
]

export function sourceLabel(source: string): string {
  return SOURCE_LABELS[source] ?? source
}

/** 哔咔分类为繁体,展示时归一化为简体;查询仍用原始值 */
const T2S_CHARS: Record<string, string> = {
  長: '长', 韓: '韩', 單: '单', 純: '纯', 愛: '爱', 園: '园', 蘿: '萝',
  後: '后', 宮: '宫', 異: '异', 觀: '观', 涼: '凉', 戀: '恋', 話: '话',
  們: '们', 貓: '猫', 龍: '龙', 鳳: '凤', 劍: '剑', 槍: '枪', 車: '车',
  東: '东', 陰: '阴', 陽: '阳', 環: '环', 蘭: '兰',
  藝: '艺', 術: '术', 顏: '颜', 質: '质', 髮: '发', 續: '续', 經: '经',
  聞: '闻', 見: '见', 語: '语', 讀: '读', 談: '谈', 論: '论', 譜: '谱',
  貝: '贝', 貼: '贴', 貴: '贵', 買: '买', 賣: '卖', 雜: '杂', 靈: '灵',
  響: '响', 頁: '页', 頭: '头', 體: '体', 鳥: '鸟', 麗: '丽', 齡: '龄',
}

export function normalizeCategoryLabel(label: string): string {
  return Array.from(label)
    .map((ch) => T2S_CHARS[ch] ?? ch)
    .join('')
}
