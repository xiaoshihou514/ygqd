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

function matchesEntry(entry: BlacklistEntry, itemTags: string[], itemTitle?: string): boolean {
  switch (entry.mode) {
    case 'fuzzy':
      return itemTags.some((t) => t.includes(entry.tag))
        || (itemTitle != null && itemTitle.includes(entry.tag))
    case 'exact':
      return itemTags.some((t) => t === entry.tag)
    case 'single':
      return itemTags.length === 1 && itemTags[0] === entry.tag
    default:
      return false
  }
}

function loadFromStorage(): BlacklistEntry[] {
  try {
    const raw = localStorage.getItem(BLACKLIST_KEY)
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
      const existingTags = new Set(serverEntries.map((e) => e.tag))
      const toMigrate = localEntries.filter((e) => !existingTags.has(e.tag))
      for (const e of toMigrate) {
        await apiAdd(e.tag, e.mode)
      }
      localStorage.removeItem(BLACKLIST_KEY)
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

function add(tag: string, mode: BlacklistMode = 'exact') {
  const trimmed = tag.trim()
  if (!trimmed) return
  if (entries.value.some((e) => e.tag === trimmed)) return

  apiAdd(trimmed, mode).catch(() => {})
  entries.value = [...entries.value, { tag: trimmed, mode, createdAt: Date.now() }]
  bumpVersion()
}

function remove(tag: string) {
  apiRemove(tag).catch(() => {})
  entries.value = entries.value.filter((e) => e.tag !== tag)
  bumpVersion()
}

function updateMode(tag: string, mode: BlacklistMode) {
  apiUpdateMode(tag, mode).catch(() => {})
  entries.value = entries.value.map((e) => e.tag === tag ? { ...e, mode } : e)
  bumpVersion()
}

function has(tag: string): boolean {
  return entries.value.some((e) => e.mode === 'exact' && e.tag === tag.trim())
}

function hasAny(tags: string[]): boolean {
  return tags.some((tag) => has(tag))
}

function filterItems<T extends { tags: string[]; title?: string }>(items: T[]): T[] {
  if (entries.value.length === 0) return items
  return items.filter((item) => !entries.value.some((e) => matchesEntry(e, item.tags, item.title)))
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
