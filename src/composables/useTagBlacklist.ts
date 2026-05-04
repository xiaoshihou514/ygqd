import { ref, computed } from 'vue'
import type { ComicItem } from '@/types'

const BLACKLIST_KEY = 'niacg-tag-blacklist'

const blacklist = ref<Set<string>>(new Set())

function loadFromStorage(): Set<string> {
  try {
    const raw = localStorage.getItem(BLACKLIST_KEY)
    if (!raw) return new Set()
    const arr: string[] = JSON.parse(raw)
    if (!Array.isArray(arr)) return new Set()
    return new Set(arr.filter((t) => typeof t === 'string' && t.length > 0))
  } catch {
    return new Set()
  }
}

function saveToStorage() {
  localStorage.setItem(BLACKLIST_KEY, JSON.stringify([...blacklist.value]))
}

blacklist.value = loadFromStorage()

const tagList = computed(() => [...blacklist.value].sort())

const count = computed(() => blacklist.value.size)

function add(tag: string) {
  const trimmed = tag.trim()
  if (!trimmed) return
  blacklist.value = new Set([...blacklist.value, trimmed])
  saveToStorage()
}

function remove(tag: string) {
  const trimmed = tag.trim()
  if (!blacklist.value.has(trimmed)) return
  const next = new Set(blacklist.value)
  next.delete(trimmed)
  blacklist.value = next
  saveToStorage()
}

function has(tag: string): boolean {
  return blacklist.value.has(tag.trim())
}

function hasAny(tags: string[]): boolean {
  return tags.some((tag) => blacklist.value.has(tag))
}

function filterItems<T extends { tags: string[] }>(items: T[]): T[] {
  if (blacklist.value.size === 0) return items
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
    add,
    remove,
    has,
    hasAny,
    filterItems,
  }
}
