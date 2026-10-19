import type { ComicItem, ParsedTitle, SplitGroup } from '@/types'

const CHINESE_NUM_MAP: Record<string, number> = {
  一: 1, 二: 2, 三: 3, 四: 4, 五: 5,
  六: 6, 七: 7, 八: 8, 九: 9, 十: 10,
  零: 0,
}

function chineseNumToInt(s: string): number | null {
  const mappedNumber = CHINESE_NUM_MAP[s]
  if (mappedNumber !== undefined) return mappedNumber
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
  const patterns: RegExp[] = [
    /第(?:\d+\s*-\s*\d+|\d+|[零一二三四五六七八九十百]+)[话回章卷集]/g,
    /[话回章卷集](\d+)/g,
    /S2\s*(\d+)/g,
    /第二部\s*(\d+)/g,
    /续集\s*(\d+)/g,
    /([零一二三四五六七八九十百]+)(?:$|\s|-|,|，|、|\.)/g,
    /(\d+-\d+(?:-\d+)?)/g,
    /(?<!\w)(\d+)(?!\w|年|月|日|岁)/g,
  ]

  let earliest = s.length
  for (const regex of patterns) {
    regex.lastIndex = 0
    let match: RegExpExecArray | null
    while ((match = regex.exec(s)) !== null) {
      earliest = Math.min(earliest, match.index)
    }
  }

  return earliest < s.length ? earliest : -1
}

function parseChapterNumbers(infoStr: string): { chapters: number[]; hasExtra: boolean } {
  const hasExtra = EXTRA_MARKERS.test(infoStr)
  const chapterInfo = infoStr.replace(EXTRA_MARKERS, '')
  const ranges = [...chapterInfo.matchAll(/(\d+)\s*-\s*(\d+)/g)]
  const rangeChapters = ranges.flatMap(([, startText, endText]) => {
    const start = Number.parseInt(startText!, 10)
    const end = Number.parseInt(endText!, 10)
    const firstChapter = Math.min(start, end)
    const lastChapter = Math.max(start, end)
    return Array.from(
      { length: lastChapter - firstChapter + 1 },
      (_, index) => firstChapter + index,
    )
  })
  const remainingInfo = chapterInfo.replace(/(\d+)\s*-\s*(\d+)/g, '')
  const numericChapters = [...remainingInfo.matchAll(/(\d+)/g)].map(([chapter]) =>
    Number.parseInt(chapter!, 10),
  )
  const chineseChapters = [...remainingInfo.matchAll(/([零一二三四五六七八九十百]+)/g)]
    .map(([, chapter]) => chineseNumToInt(chapter!))
    .filter((chapter): chapter is number => chapter !== null)

  return {
    chapters: [...new Set([...rangeChapters, ...numericChapters, ...chineseChapters])].sort(
      (a, b) => a - b,
    ),
    hasExtra,
  }
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

    const isComplete = g.hasCompleteMark || isStandalone
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

export function splitAndGroup(items: readonly ComicItem[]): SplitGroup[] {
  const parsed = items.map(parseTitle)
  return groupParsed(parsed)
}
