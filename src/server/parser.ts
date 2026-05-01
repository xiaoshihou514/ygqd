import { parse, type HTMLElement } from 'node-html-parser'
import type { ComicItem, PaginationInfo, HomeSection } from '../types'

const NIACG_BASE = 'https://www.niacg.com'

const CATEGORY_MAP: Record<string, string> = {
  '1': 'COS',
  '2': 'CG',
  '3': '本子',
  '4': '套图',
  '9': 'A漫',
}

function resolveUrl(url: string): string {
  if (!url) return ''
  if (url.startsWith('http://') || url.startsWith('https://') || url.startsWith('//')) return url
  if (url.startsWith('/')) return NIACG_BASE + url
  return NIACG_BASE + '/' + url
}

function extractItem(el: HTMLElement): ComicItem | null {
  const linkEl = el.querySelector('a[href*="moehome"]')
  if (!linkEl) return null

  const href = linkEl.getAttribute('href') || ''
  const match = href.match(/moehome-(\d+)-(\d+)\.html/)
  if (!match) return null

  const categoryId = Number(match[1])
  const id = match[2]

  const imgEl = el.querySelector('img')
  const rawThumbnail =
    (imgEl?.getAttribute('data-src') as string | undefined) ||
    (imgEl?.getAttribute('data-original') as string | undefined) ||
    (imgEl?.getAttribute('src') as string | undefined) ||
    ''
  const thumbnail = resolveUrl(rawThumbnail)

  const titleEl = el.querySelector('.video-title') || el.parentNode?.querySelector('.video-title')
  const title = titleEl?.textContent?.trim() || ''

  const tagEls = el.querySelectorAll('.tag')
  const parentTags = el.parentNode?.querySelectorAll('.tag') || []
  const allTags = tagEls.length > 0 ? tagEls : parentTags
  const tags = allTags.map((t) => t.textContent.trim()).filter(Boolean)

  const likeEl = el.querySelector('[id^="albim_likes_"]')
  const likes = likeEl?.textContent?.trim() || ''

  const catEl = el.querySelector('.label-category, .label-sub')
  const category = catEl?.textContent?.trim() || CATEGORY_MAP[String(categoryId)] || ''

  return {
    id,
    title: title.replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"'),
    thumbnail,
    category,
    categoryId,
    tags,
    likes,
    link: href,
  }
}

export function parseHomepage(html: string): HomeSection[] {
  const root = parse(html)
  const sections: HomeSection[] = []

  const allH4s = root.querySelectorAll('h4')
  const allCarousels = root.querySelectorAll('.owl-carousel')

  for (const h4 of allH4s) {
    const heading = h4.textContent.trim()
    let categoryId = 0
    let label = ''
    let category = ''

    if (heading.includes('COS')) {
      categoryId = 1; category = 'COS'; label = 'COS推荐'
    } else if (heading.includes('套图')) {
      categoryId = 4; category = '套图'; label = '套图推荐'
    } else if (heading.includes('CG')) {
      categoryId = 2; category = 'CG'; label = 'CG推荐'
    } else if (heading.includes('本本') || heading.includes('本子')) {
      categoryId = 3; category = '本子'; label = '本子推荐'
    } else if (heading.includes('里番')) {
      categoryId = 19; category = '里番'; label = '里番推荐'
    } else if (heading.includes('3D')) {
      categoryId = 20; category = '3D'; label = '3D推荐'
    } else if (heading.includes('同人')) {
      categoryId = 21; category = '同人'; label = '同人推荐'
    } else {
      continue
    }

    // Find the first carousel that appears after this h4's end position in HTML
    const h4EndPos = h4.range?.[1] || 0
    let carousel: HTMLElement | null = null
    for (const c of allCarousels) {
      const cStartPos = c.range?.[0] || 0
      if (cStartPos > h4EndPos) {
        carousel = c
        break
      }
    }

    if (!carousel) continue

    const items: ComicItem[] = []
    const itemWrappers = carousel.querySelectorAll('.owl-item .p-b-15')

    for (const wrapper of itemWrappers) {
      const item = extractItem(wrapper)
      if (item) items.push(item)
    }

    if (items.length > 0) {
      sections.push({ category, categoryId, label, items })
    }
  }

  return sections
}

export function parseListPage(html: string): { items: ComicItem[]; pagination: PaginationInfo } {
  const root = parse(html)
  const items: ComicItem[] = []

  const itemEls = [
    ...root.querySelectorAll('.owl-item .p-b-15'),
    ...root.querySelectorAll('.owl-item'),
    ...root.querySelectorAll('.list-col .p-b-15'),
    ...root.querySelectorAll('.p-b-15'),
  ]

  const seen = new Set<string>()
  for (const el of itemEls) {
    const item = extractItem(el)
    if (item && !seen.has(item.id)) {
      seen.add(item.id)
      items.push(item)
    }
  }

  const pagination = extractPagination(root)

  return { items, pagination }
}

function extractPagination(root: ReturnType<typeof parse>): PaginationInfo {
  const pageLinks = root.querySelectorAll('.pagination a')
  const urls = pageLinks
    .map((a) => a.getAttribute('href') || '')
    .filter((h) => h.includes('listinfo'))

  const pageMatch = urls.find((u) => u)?.match(/listinfo-\d+-(\d+)\.html/)
  const currentPage = pageMatch ? Number(pageMatch[1]) : 0

  const totalMatch = urls.find((u) => u.includes('尾页')) || urls.at(-1)
  let total = currentPage + 1
  if (totalMatch) {
    const m = totalMatch.match(/listinfo-\d+-(\d+)\.html/)
    if (m) total = Number(m[1])
  }

  return {
    current: currentPage,
    total,
    hasNext: currentPage < total,
    hasPrev: currentPage > 0,
  }
}

export function parseSearchResults(html: string): { items: ComicItem[] } {
  const root = parse(html)
  const items: ComicItem[] = []

  const itemEls = [
    ...root.querySelectorAll('.list-col .p-b-15'),
    ...root.querySelectorAll('.owl-item .p-b-15'),
    ...root.querySelectorAll('.p-b-15'),
  ]

  const seen = new Set<string>()
  for (const el of itemEls) {
    const item = extractItem(el)
    if (item && !seen.has(item.id)) {
      seen.add(item.id)
      items.push(item)
    }
  }

  return { items }
}
