export interface ComicItem {
  id: string
  title: string
  thumbnail: string
  category: string
  categoryId: number
  tags: string[]
  likes: string
  link: string
}

export interface PaginationInfo {
  current: number
  total: number
  hasNext: boolean
  hasPrev: boolean
}

export interface SearchParams {
  keyword: string
  classid: number
  show?: string
  tempid?: string
}

export interface HomeSection {
  category: string
  categoryId: number
  label: string
  items: ComicItem[]
}

export interface CategoryOption {
  id: number
  label: string
}
