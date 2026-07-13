import { ref, computed } from 'vue'

const HISTORY_KEY = 'niacg-search-history'
const MAX_ITEMS = 20

const history = ref<string[]>([])

function loadFromStorage(): string[] {
  try {
    const raw = localStorage.getItem(HISTORY_KEY)
    if (!raw) return []
    const arr: unknown = JSON.parse(raw)
    if (!Array.isArray(arr)) return []
    return arr.filter((t): t is string => typeof t === 'string' && t.length > 0).slice(0, MAX_ITEMS)
  } catch {
    return []
  }
}

function saveToStorage() {
  localStorage.setItem(HISTORY_KEY, JSON.stringify(history.value))
}

history.value = loadFromStorage()

function add(keyword: string) {
  const trimmed = keyword.trim()
  if (!trimmed) return
  const next = [trimmed, ...history.value.filter((k: string) => k !== trimmed)].slice(0, MAX_ITEMS)
  history.value = next
  saveToStorage()
}

function remove(keyword: string) {
  history.value = history.value.filter((k: string) => k !== keyword)
  saveToStorage()
}

function clear() {
  history.value = []
  saveToStorage()
}

export function useSearchHistory() {
  function suggestions(input: string): string[] {
    const q = input.trim().toLowerCase()
    if (!q) return history.value.slice(0, 5)
    return history.value.filter((k: string) => k.toLowerCase().includes(q)).slice(0, 5)
  }

  return {
    history: computed(() => history.value),
    suggestions,
    add,
    remove,
    clear,
  }
}
