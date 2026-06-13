import { ref, computed } from 'vue'

const BLACKLIST_KEY = 'niacg-tag-blacklist'

const blacklist = ref<string[]>([])
const _version = ref(0)

function loadFromStorage(): string[] {
  try {
    const raw = localStorage.getItem(BLACKLIST_KEY)
    if (!raw) return []
    const arr: unknown = JSON.parse(raw)
    if (!Array.isArray(arr)) return []
    return arr.filter((t): t is string => typeof t === 'string' && t.length > 0)
  } catch {
    return []
  }
}

function saveToStorage() {
  localStorage.setItem(BLACKLIST_KEY, JSON.stringify(blacklist.value))
}

blacklist.value = loadFromStorage()

const tagList = computed(() => [...blacklist.value].sort())

const count = computed(() => blacklist.value.length)

function bumpVersion() {
  _version.value++
}

function add(tag: string) {
  const trimmed = tag.trim()
  if (!trimmed) return
  if (blacklist.value.includes(trimmed)) return
  blacklist.value = [...blacklist.value, trimmed]
  saveToStorage()
  bumpVersion()
}

function remove(tag: string) {
  const trimmed = tag.trim()
  const idx = blacklist.value.indexOf(trimmed)
  if (idx === -1) return
  const next = [...blacklist.value]
  next.splice(idx, 1)
  blacklist.value = next
  saveToStorage()
  bumpVersion()
}

function has(tag: string): boolean {
  return blacklist.value.includes(tag.trim())
}

function hasAny(tags: string[]): boolean {
  return tags.some((tag) => blacklist.value.includes(tag))
}

function filterItems<T extends { tags: string[] }>(items: T[]): T[] {
  if (blacklist.value.length === 0) return items
  return items.filter((item) => !hasAny(item.tags))
}

let useCount = 0

export function useTagBlacklist() {
  if (useCount === 0 && typeof window !== 'undefined') {
    window.addEventListener('storage', (e) => {
      if (e.key === BLACKLIST_KEY) {
        blacklist.value = loadFromStorage()
      }
    })
  }
  useCount++

  return {
    tagList,
    count,
    version: _version,
    add,
    remove,
    has,
    hasAny,
    filterItems,
  }
}
