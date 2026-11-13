import { ref, computed } from 'vue'
import {
  fetchBlacklist,
  addBlacklistEntry as apiAdd,
  removeBlacklistEntry as apiRemove,
  updateBlacklistEntry as apiUpdateMode,
} from '@/services/api'
import type { BlacklistEntry, BlacklistMode } from '@/types'

const BLACKLIST_KEY = 'niacg-tag-blacklist'

const entries = ref<BlacklistEntry[]>([])
const _version = ref(0)

function bumpVersion() {
  _version.value++
}

function matchesEntry(entry: BlacklistEntry, itemTags: readonly string[], itemTitle?: string): boolean {
  switch (entry.mode) {
    case 'fuzzy':
      return itemTags.some((t) => t.includes(entry.tag))
        || (itemTitle != null && itemTitle.includes(entry.tag))
    case 'exact':
      return itemTags.includes(entry.tag)
    case 'single':
      return itemTags.length === 1 && itemTags[0] === entry.tag
    default:
      return false
  }
}

function readLegacyStorage(): string | null {
  try {
    return localStorage.getItem(BLACKLIST_KEY)
  } catch {
    return null
  }
}

function loadFromStorage(): BlacklistEntry[] {
  try {
    const raw = readLegacyStorage()
    if (!raw) return []
    const arr: unknown = JSON.parse(raw)
    if (!Array.isArray(arr)) return []
    return arr
      .filter((t): t is string => typeof t === 'string' && t.length > 0)
      .map((tag) => ({ tag, mode: 'exact' as BlacklistMode }))
  } catch {
    return []
  }
}

async function load() {
  try {
    const serverEntries = await fetchBlacklist()
    const localEntries = loadFromStorage()
    if (localEntries.length > 0) {
      // 旧版 localStorage 黑名单一次性迁移入库:逐条容错,全部成功才清本地,
      // 单条失败不拖垮整次加载(否则会退回只读本地、丢掉服务端条目)
      const existingTags = new Set(serverEntries.map((e) => e.tag))
      const toMigrate = localEntries.filter((e) => !existingTags.has(e.tag))
      const results = await Promise.allSettled(toMigrate.map((e) => apiAdd(e.tag, e.mode)))
      if (results.every((r) => r.status === 'fulfilled')) {
        try {
          localStorage.removeItem(BLACKLIST_KEY)
        } catch {
          /* 存储不可用:下次启动重试迁移(已入库条目会被 existingTags 跳过) */
        }
      }
      entries.value = [...serverEntries, ...toMigrate]
    } else {
      entries.value = serverEntries
    }
  } catch {
    entries.value = loadFromStorage()
  }
  bumpVersion()
}

load()

const tagList = computed(() => [...entries.value]
  .sort((a, b) => a.tag.localeCompare(b.tag)))

const count = computed(() => entries.value.length)

/** exact 模式标签集合:has()/hasAny() 每张卡片都会调用,O(1) 查找 */
const exactTags = computed(() => {
  const set = new Set<string>()
  for (const e of entries.value) if (e.mode === 'exact') set.add(e.tag)
  return set
})

/** 乐观更新后后端写入失败:重新拉取以与数据库对齐,避免界面与实际状态长期不一致 */
function resyncOnFailure(e: unknown) {
  console.error('[blacklist] 写入失败,重新同步', e)
  void load()
}

function add(tag: string, mode: BlacklistMode = 'exact') {
  const trimmed = tag.trim()
  if (!trimmed) return
  if (entries.value.some((e) => e.tag === trimmed)) return

  apiAdd(trimmed, mode).catch(resyncOnFailure)
  entries.value = [...entries.value, { tag: trimmed, mode, createdAt: Date.now() }]
  bumpVersion()
}

function remove(tag: string) {
  if (!entries.value.some((e) => e.tag === tag)) return
  apiRemove(tag).catch(resyncOnFailure)
  entries.value = entries.value.filter((e) => e.tag !== tag)
  bumpVersion()
}

function updateMode(tag: string, mode: BlacklistMode) {
  const current = entries.value.find((e) => e.tag === tag)
  if (!current || current.mode === mode) return
  apiUpdateMode(tag, mode).catch(resyncOnFailure)
  entries.value = entries.value.map((e) => e.tag === tag ? { ...e, mode } : e)
  bumpVersion()
}

function has(tag: string): boolean {
  return exactTags.value.has(tag.trim())
}

function hasAny(tags: string[]): boolean {
  return tags.some((tag) => has(tag))
}

function filterItems<T extends { tags: string[]; title?: string }>(items: T[]): T[] {
  const list = entries.value
  if (list.length === 0) return items
  return items.filter((item) => !list.some((e) => matchesEntry(e, item.tags, item.title)))
}

export function useTagBlacklist() {
  return {
    entries,
    tagList,
    count,
    version: _version,
    add,
    remove,
    updateMode,
    has,
    hasAny,
    filterItems,
  }
}
