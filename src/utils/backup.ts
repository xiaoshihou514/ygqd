import {
  fetchBlacklist,
  fetchFollowedAuthors,
  followAuthor,
  novelFollowedAuthorsReadonly,
  videoFollow,
  videoFollowList,
} from '@/services/api'
import type { Theme } from '@/composables/useTheme'
import type { BlacklistEntry, BlacklistMode, FollowedAuthor, FollowTargetEntry } from '@/types'

/**
 * 统一备份(设置 + 关注)的唯一格式定义与读写逻辑。
 *
 * v2:settings.{theme, blacklist} + follows.{comicAuthors, videoTargets(全部源×类型), novelAuthors}
 * v1(旧版导出,仍可导入):blacklist.entries + follows.{comicAuthors, videoTargetsActress, videoTargetsMaker}
 *
 * 导入为**合并**语义:只新增/更新,从不删除本机已有数据;小说作者仅随导出保留(写接口已移除)。
 */

export const BACKUP_FORMAT = 'ygqd.backup'
export const BACKUP_VERSION = 2

const VIDEO_FOLLOW_SOURCES = ['fs1', 'missav'] as const
const VIDEO_FOLLOW_KINDS = ['actress', 'maker'] as const
const THEMES: readonly Theme[] = ['system', 'light', 'dark']
const MODES: readonly BlacklistMode[] = ['fuzzy', 'exact', 'single']

export interface BackupV2 {
  format: typeof BACKUP_FORMAT
  version: 2
  exportedAt: string
  settings: { theme: Theme; blacklist: BlacklistEntry[] }
  follows: {
    comicAuthors: FollowedAuthor[]
    videoTargets: FollowTargetEntry[]
    novelAuthors: { authorId: string; authorName: string }[]
  }
}

/** 解析后的可导入内容(已校验、已归一化) */
export interface BackupPayload {
  theme: Theme | null
  blacklist: { tag: string; mode: BlacklistMode }[]
  comicAuthors: string[]
  videoTargets: { source: string; kind: string; targetId: string; displayName: string }[]
}

export interface ImportSummary {
  blacklistAdded: number
  blacklistUpdated: number
  comicAuthorsAdded: number
  videoTargetsAdded: number
  themeApplied: boolean
  failed: number
}

/** 汇总本机全部设置与关注数据。任一关注源读取失败按空降级,不拖垮整次导出。 */
export async function buildBackup(theme: Theme): Promise<BackupV2> {
  const combos = VIDEO_FOLLOW_SOURCES.flatMap((s) => VIDEO_FOLLOW_KINDS.map((k) => [s, k] as const))
  const [blacklist, comicAuthors, novelAuthors, ...targetLists] = await Promise.all([
    fetchBlacklist(),
    fetchFollowedAuthors(),
    novelFollowedAuthorsReadonly().catch(() => []),
    ...combos.map(([s, k]) => videoFollowList(s, k).catch((): FollowTargetEntry[] => [])),
  ])
  return {
    format: BACKUP_FORMAT,
    version: BACKUP_VERSION,
    exportedAt: new Date().toISOString(),
    settings: { theme, blacklist },
    follows: { comicAuthors, videoTargets: targetLists.flat(), novelAuthors },
  }
}

// ---------- 解析与校验 ----------

type Obj = Record<string, unknown>
const isObj = (v: unknown): v is Obj => typeof v === 'object' && v !== null && !Array.isArray(v)
const arr = (v: unknown): unknown[] => (Array.isArray(v) ? v : [])
const str = (v: unknown): string => (typeof v === 'string' ? v.trim() : '')

function parseBlacklist(list: unknown[]): BackupPayload['blacklist'] {
  const seen = new Set<string>()
  const out: BackupPayload['blacklist'] = []
  for (const e of list) {
    if (!isObj(e)) continue
    const tag = str(e.tag)
    if (!tag || seen.has(tag)) continue
    seen.add(tag)
    const mode = MODES.includes(e.mode as BlacklistMode) ? (e.mode as BlacklistMode) : 'exact'
    out.push({ tag, mode })
  }
  return out
}

function parseTargets(list: unknown[], fallback?: { source: string; kind: string }): BackupPayload['videoTargets'] {
  const out: BackupPayload['videoTargets'] = []
  for (const e of list) {
    if (!isObj(e)) continue
    const source = str(e.source) || fallback?.source || ''
    const kind = str(e.kind) || fallback?.kind || ''
    const targetId = str(e.targetId)
    if (!targetId || !VIDEO_FOLLOW_SOURCES.includes(source as never) || !VIDEO_FOLLOW_KINDS.includes(kind as never)) continue
    out.push({ source, kind, targetId, displayName: str(e.displayName) || targetId })
  }
  return out
}

/** 校验并归一化备份 JSON 文本;不是本应用备份时抛出带中文说明的错误。 */
export function parseBackup(text: string): BackupPayload {
  let data: unknown
  try {
    data = JSON.parse(text)
  } catch {
    throw new Error('文件不是有效的 JSON')
  }
  if (!isObj(data) || data.format !== BACKUP_FORMAT) throw new Error('不是 ygqd 备份文件')
  const version = typeof data.version === 'number' ? data.version : 0
  if (version < 1) throw new Error('备份文件版本无法识别')
  if (version > BACKUP_VERSION) throw new Error('备份来自更新版本的应用,请先升级')

  const settings = isObj(data.settings) ? data.settings : {}
  const follows = isObj(data.follows) ? data.follows : {}
  const legacyBlacklist = isObj(data.blacklist) ? data.blacklist.entries : undefined

  const theme = THEMES.includes(settings.theme as Theme) ? (settings.theme as Theme) : null
  const blacklist = parseBlacklist(arr(version >= 2 ? settings.blacklist : legacyBlacklist))
  const comicAuthors = [...new Set(arr(follows.comicAuthors).map((a) => str(isObj(a) ? a.author : a)).filter(Boolean))]
  const videoTargets = version >= 2
    ? parseTargets(arr(follows.videoTargets))
    : [
        ...parseTargets(arr(follows.videoTargetsActress), { source: 'missav', kind: 'actress' }),
        ...parseTargets(arr(follows.videoTargetsMaker), { source: 'missav', kind: 'maker' }),
      ]

  return { theme, blacklist, comicAuthors, videoTargets }
}

// ---------- 合并导入 ----------

export interface ImportDeps {
  /** 当前黑名单(用于判定新增/改模式) */
  blacklist: readonly BlacklistEntry[]
  addBlacklist: (tag: string, mode: BlacklistMode) => void
  updateBlacklistMode: (tag: string, mode: BlacklistMode) => void
  setTheme: (theme: Theme) => void
}

/**
 * 合并导入:黑名单缺失则新增、模式不同以备份为准;关注缺失则新增;主题直接应用。
 * 黑名单走组合式(乐观更新 + 失败自动重同步),关注直接调命令并统计失败数。
 */
export async function applyBackup(payload: BackupPayload, deps: ImportDeps): Promise<ImportSummary> {
  const summary: ImportSummary = {
    blacklistAdded: 0,
    blacklistUpdated: 0,
    comicAuthorsAdded: 0,
    videoTargetsAdded: 0,
    themeApplied: false,
    failed: 0,
  }

  const current = new Map(deps.blacklist.map((e) => [e.tag, e.mode]))
  for (const { tag, mode } of payload.blacklist) {
    const existing = current.get(tag)
    if (existing === undefined) {
      deps.addBlacklist(tag, mode)
      summary.blacklistAdded++
    } else if (existing !== mode) {
      deps.updateBlacklistMode(tag, mode)
      summary.blacklistUpdated++
    }
  }

  if (payload.theme) {
    deps.setTheme(payload.theme)
    summary.themeApplied = true
  }

  const combos = [...new Set(payload.videoTargets.map((t) => `${t.source}\u0000${t.kind}`))]
  const [authors, ...targetLists] = await Promise.all([
    fetchFollowedAuthors(),
    ...combos.map((c) => {
      const [s = '', k = ''] = c.split('\u0000')
      return videoFollowList(s, k)
    }),
  ])
  const followedAuthors = new Set(authors.map((a) => a.author))
  const followedTargets = new Set(targetLists.flat().map((t) => `${t.source}\u0000${t.kind}\u0000${t.targetId}`))

  const jobs: Promise<'author' | 'target'>[] = []
  for (const author of payload.comicAuthors) {
    if (!followedAuthors.has(author)) jobs.push(followAuthor(author).then(() => 'author' as const))
  }
  for (const t of payload.videoTargets) {
    if (followedTargets.has(`${t.source}\u0000${t.kind}\u0000${t.targetId}`)) continue
    jobs.push(videoFollow(t.source, t.kind, t.targetId, t.displayName).then(() => 'target' as const))
  }
  for (const r of await Promise.allSettled(jobs)) {
    if (r.status === 'rejected') summary.failed++
    else if (r.value === 'author') summary.comicAuthorsAdded++
    else summary.videoTargetsAdded++
  }
  return summary
}

/** 导入结果的一行中文摘要 */
export function describeImport(s: ImportSummary): string {
  const parts: string[] = []
  if (s.blacklistAdded) parts.push(`黑名单 +${s.blacklistAdded}`)
  if (s.blacklistUpdated) parts.push(`黑名单改模式 ${s.blacklistUpdated}`)
  if (s.comicAuthorsAdded) parts.push(`漫画作者 +${s.comicAuthorsAdded}`)
  if (s.videoTargetsAdded) parts.push(`视频关注 +${s.videoTargetsAdded}`)
  if (s.themeApplied) parts.push('主题已应用')
  const body = parts.length ? `已导入:${parts.join('、')}` : '备份内容与本机一致,无需导入'
  return s.failed ? `${body};${s.failed} 项写入失败` : body
}
