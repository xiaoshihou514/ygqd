import type { ComicItem, ComicDetail, HomeSection, PaginationInfo, SearchParams } from '../types'
import type { SearchResult } from '../server/parser'

interface ApiResponse<T> {
  code: number
  data: T
  message?: string
}

async function request<T>(url: string, options?: RequestInit): Promise<T> {
  const resp = await fetch(url, options)
  const json: ApiResponse<T> = await resp.json()
  if (json.code !== 0) {
    throw new Error(json.message || 'Request failed')
  }
  return json.data
}

export function fetchHomeRecommendations(): Promise<HomeSection[]> {
  return request<HomeSection[]>('/api/home')
}

export function fetchCategoryList(
  category: number,
  page: number,
): Promise<{ items: ComicItem[]; pagination: PaginationInfo }> {
  return request<{ items: ComicItem[]; pagination: PaginationInfo }>(
    `/api/list?cat=${category}&page=${page}`,
  )
}

export function searchComics(
  params: SearchParams,
): Promise<SearchResult> {
  const body = new URLSearchParams()
  body.set('classid', String(params.classid))
  body.set('keyword', params.keyword)
  body.set('show', params.show || 'title,text,keyboard,ftitle')
  body.set('tempid', params.tempid || '1')

  return request<SearchResult>('/api/search', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: body.toString(),
  })
}

export function fetchComicDetail(
  categoryId: number,
  id: string,
): Promise<ComicDetail> {
  return request<ComicDetail>(`/api/comic?cat=${categoryId}&id=${id}`)
}
