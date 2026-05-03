import type { ComicItem, ParsedTitle, SplitGroup } from '@/types'

const CHINESE_NUM_MAP: Record<string, number> = {
  一: 1, 二: 2, 三: 3, 四: 4, 五: 5,
  六: 6, 七: 7, 八: 8, 九: 9, 十: 10,
  零: 0,
}

function chineseNumToInt(s: string): number | null {
  if (CHINESE_NUM_MAP[s]) return CHINESE_NUM_MAP[s]
  if (s === '百') return 100
  if (s.length === 2 && s[0] === '十') {
    const unit = CHINESE_NUM_MAP[s[1] ?? '']
    if (unit !== undefined) return 10 + unit
    return 10
  }
  if (s.length === 3 && s[1] === '十') {
    const tens = CHINESE_NUM_MAP[s[0] ?? '']
    const ones = CHINESE_NUM_MAP[s[2] ?? '']
    if (tens !== undefined && ones !== undefined) return tens * 10 + ones
  }
  if (s.length === 3 && s[0] === '十') {
    const ones = CHINESE_NUM_MAP[s[2] ?? '']
    if (ones !== undefined) return 10 + ones
  }
  if (s.length === 2 && s.endsWith('十')) {
    const tens = CHINESE_NUM_MAP[s[0] ?? '']
    if (tens !== undefined) return tens * 10
  }
  return null
}

const COMPLETE_MARKERS = /(完本|全篇|END|\(完\)|完全版|全本|(?<!\w)完(?!\w|$)|\b完\b)/

const EXTRA_MARKERS = /(番外|特别篇)/

function findChapterSeparator(s: string): number {
  const patterns: { regex: RegExp; adjust?: (m: RegExpExecArray) => number }[] = [
    { regex: /第(\d+|[一二三四五六七八九十百]+)[话回章卷集]/g, adjust: (m) => m.index },
    { regex: /[话回章卷集](\d+)/g, adjust: (m) => m.index },
    { regex: /S2\s*(\d+)/g, adjust: (m) => m.index },
    { regex: /第二部\s*(\d+)/g, adjust: (m) => m.index },
    { regex: /续集\s*(\d+)/g, adjust: (m) => m.index },
    { regex: /([一二三四五六七八九十百]+)(?:$|\s|-|,|，|、|\.)/g, adjust: (m) => m.index },
    { regex: /(\d+-\d+(?:-\d+)?)/g, adjust: (m) => m.index },
    { regex: /(?<!\w)(\d+)(?!\w|年|月|日|岁)/g, adjust: (m) => m.index },
  ]

  let earliest = s.length
  for (const { regex, adjust } of patterns) {
    regex.lastIndex = 0
    let m: RegExpExecArray | null
    while ((m = regex.exec(s)) !== null) {
      const pos = adjust ? adjust(m) : m.index
      if (pos < earliest) {
        earliest = pos
      }
    }
  }

  return earliest < s.length ? earliest : -1
}

function parseChapterNumbers(infoStr: string): { chapters: number[]; hasExtra: boolean } {
  const chapters: number[] = []
  let hasExtra = false

  if (EXTRA_MARKERS.test(infoStr)) {
    hasExtra = true
    infoStr = infoStr.replace(EXTRA_MARKERS, '')
  }

  const rangeRegex = /(\d+)\s*-\s*(\d+)/g
  let m: RegExpExecArray | null
  while ((m = rangeRegex.exec(infoStr)) !== null) {
    const a = parseInt(m[1]!, 10)
    const b = parseInt(m[2]!, 10)
    const start = Math.min(a, b)
    const end = Math.max(a, b)
    for (let i = start; i <= end; i++) {
      chapters.push(i)
    }
    infoStr = infoStr.replace(m[0], '')
  }

  const singleDigitRegex = /(\d+)/g
  while ((m = singleDigitRegex.exec(infoStr)) !== null) {
    chapters.push(parseInt(m[1]!, 10))
  }

  const chineseRegex = /([一二三四五六七八九十百]+)/g
  while ((m = chineseRegex.exec(infoStr)) !== null) {
    const val = chineseNumToInt(m[1]!)
    if (val !== null) {
      chapters.push(val)
    }
  }

  const dihuaRegex = /第(\d+)[话回章卷集]/g
  while ((m = dihuaRegex.exec(infoStr)) !== null) {
    chapters.push(parseInt(m[1]!, 10))
  }
  const dihuaChineseRegex = /第([一二三四五六七八九十百]+)[话回章卷集]/g
  while ((m = dihuaChineseRegex.exec(infoStr)) !== null) {
    const val = chineseNumToInt(m[1]!)
    if (val !== null) {
      chapters.push(val)
    }
  }

  const s2Regex = /S2\s*(\d+)/g
  while ((m = s2Regex.exec(infoStr)) !== null) {
    chapters.push(parseInt(m[1]!, 10))
  }

  const dierbuRegex = /第二部\s*(\d+)/g
  while ((m = dierbuRegex.exec(infoStr)) !== null) {
    chapters.push(parseInt(m[1]!, 10))
  }

  return { chapters: [...new Set(chapters)].sort((a, b) => a - b), hasExtra }
}

function parseTitle(item: ComicItem): ParsedTitle {
  let title = item.title

  title = title.replace(/\[AI绘图\]/g, '').replace(/\[Chinese\]/g, '')

  const authorMatch = title.match(/\[([^\]]+)\]/)
  const author = authorMatch?.[1]?.trim() ?? ''
  let s = authorMatch ? title.replace(authorMatch[0], '').trim() : title.trim()

  const isComplete = COMPLETE_MARKERS.test(s)
  s = s.replace(COMPLETE_MARKERS, '').replace(/[()（）]/g, ' ').replace(/\s+/g, ' ').trim()

  const sepIdx = findChapterSeparator(s)

  let workName: string
  let chapterInfo: string
  let chapters: number[] = []
  let hasExtra = false

  if (sepIdx === -1) {
    workName = s
    chapterInfo = ''
  } else {
    workName = s.slice(0, sepIdx).replace(/[-—–\s,，、]+$/, '').trim()
    chapterInfo = s.slice(sepIdx).trim()

    if (!workName) {
      workName = s
      chapterInfo = ''
    }
  }

  if (chapterInfo) {
    const parsed = parseChapterNumbers(chapterInfo)
    chapters = parsed.chapters
    hasExtra = parsed.hasExtra
  }

  workName = workName.replace(/&amp;/g, '&').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"')

  return {
    author,
    workName,
    chapters,
    hasExtra,
    isComplete,
    rawTitle: item.title,
    item,
  }
}

function groupParsed(parsed: ParsedTitle[]): SplitGroup[] {
  const groupMap = new Map<string, { author: string; workName: string; items: ComicItem[]; chapters: Set<number>; hasExtra: boolean; hasCompleteMark: boolean }>()

  for (const p of parsed) {
    const key = `${p.author}|${p.workName}`
    if (!groupMap.has(key)) {
      groupMap.set(key, {
        author: p.author,
        workName: p.workName,
        items: [],
        chapters: new Set(),
        hasExtra: false,
        hasCompleteMark: false,
      })
    }
    const g = groupMap.get(key)!
    g.items.push(p.item)
    for (const ch of p.chapters) {
      g.chapters.add(ch)
    }
    if (p.hasExtra) g.hasExtra = true
    if (p.isComplete) g.hasCompleteMark = true
  }

  const groups: SplitGroup[] = []
  for (const [, g] of groupMap) {
    const knownChapters = [...g.chapters].sort((a, b) => a - b)
    const isStandalone = knownChapters.length === 0

    let missingChapters: number[] = []
    if (!isStandalone && knownChapters.length > 0) {
      const max = knownChapters[knownChapters.length - 1]!
      const knownSet = new Set(knownChapters)
      const expectedStart = 1
      for (let i = expectedStart; i <= max; i++) {
        if (!knownSet.has(i)) {
          missingChapters.push(i)
        }
      }
    }

    let isComplete = false
    if (g.hasCompleteMark) {
      isComplete = true
    } else if (isStandalone) {
      isComplete = true
    } else if (knownChapters.length === 1) {
      isComplete = false
    } else if (missingChapters.length > 0) {
      isComplete = false
    } else {
      isComplete = false
    }

    groups.push({
      author: g.author,
      workName: g.workName,
      items: g.items,
      knownChapters,
      missingChapters,
      hasExtra: g.hasExtra,
      isComplete,
      isStandalone,
    })
  }

  groups.sort((a, b) => {
    const lenDiff = b.knownChapters.length - a.knownChapters.length
    if (lenDiff !== 0) return lenDiff
    if (a.author !== b.author) return a.author.localeCompare(b.author)
    return a.workName.localeCompare(b.workName)
  })

  return groups
}

export function splitAndGroup(items: ComicItem[]): SplitGroup[] {
  const parsed = items.map(parseTitle)
  return groupParsed(parsed)
}
