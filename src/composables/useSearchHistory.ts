import { ref, computed } from 'vue'
import type { Ref } from 'vue'

const MAX_ITEMS = 20

export interface SearchHistoryStore {
  history: Readonly<Ref<string[]>>
  suggestions: (input: string) => string[]
  add: (keyword: string) => void
  remove: (keyword: string) => void
  clear: () => void
}

/** 搜索历史存储工厂:同一 key 全局单例(实例建在模块级),不同内容板块
 *  (漫画/视频)用不同 key 隔离——番号与作品名/作者词语义不同,共用会
 *  互相干扰。纯前端 localStorage,与后端无关(对齐漫画侧既有机制)。 */
export function createSearchHistory(key: string): SearchHistoryStore {
  const history = ref<string[]>([])

  function loadFromStorage(): string[] {
    try {
      const raw = localStorage.getItem(key)
      if (!raw) return []
      const arr: unknown = JSON.parse(raw)
      if (!Array.isArray(arr)) return []
      return arr.filter((t): t is string => typeof t === 'string' && t.length > 0).slice(0, MAX_ITEMS)
    } catch {
      return []
    }
  }

  function saveToStorage() {
    localStorage.setItem(key, JSON.stringify(history.value))
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

  function suggestions(input: string): string[] {
    const q = input.trim().toLowerCase()
    if (!q) return history.value.slice(0, 5)
    return history.value.filter((k: string) => k.toLowerCase().includes(q)).slice(0, 5)
  }

  return {
    history: computed(() => history.value) as Readonly<Ref<string[]>>,
    suggestions,
    add,
    remove,
    clear,
  }
}

// 漫画侧单例:沿用既有存储 key,行为零变化
const comicHistory = createSearchHistory('niacg-search-history')

export function useSearchHistory() {
  return {
    history: comicHistory.history,
    suggestions: (input: string) => comicHistory.suggestions(input),
    add: comicHistory.add,
    remove: comicHistory.remove,
    clear: comicHistory.clear,
  }
}
