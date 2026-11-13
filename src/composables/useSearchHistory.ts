import { ref, computed } from 'vue'
import type { Ref } from 'vue'

const MAX_ITEMS = 20
const MAX_SUGGESTIONS = 5

export interface SearchHistoryStore {
  history: Readonly<Ref<string[]>>
  suggestions: (input: string) => string[]
  add: (keyword: string) => void
  remove: (keyword: string) => void
  clear: () => void
}

function loadFromStorage(key: string): string[] {
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

function saveToStorage(key: string, list: readonly string[]) {
  try {
    localStorage.setItem(key, JSON.stringify(list))
  } catch {
    // 存储不可用(隐私模式/配额满):内存态照常工作,仅不持久化
  }
}

/** 同一 key 只建一个实例:搜索页(useSearchHistory)与 SearchInput
 *  (createSearchHistory)并存时若各持一份,一方写入会被另一方的旧快照覆盖 */
const stores = new Map<string, SearchHistoryStore>()

/** 搜索历史存储工厂:同一 key 全局单例(按 key 缓存在模块级),不同内容板块
 *  (漫画/视频)用不同 key 隔离——番号与作品名/作者词语义不同,共用会
 *  互相干扰。纯前端 localStorage,与后端无关(对齐漫画侧既有机制)。 */
export function createSearchHistory(key: string): SearchHistoryStore {
  const existing = stores.get(key)
  if (existing) return existing

  const history = ref<string[]>(loadFromStorage(key))

  function commit(next: string[]) {
    history.value = next
    saveToStorage(key, next)
  }

  function add(keyword: string) {
    const trimmed = keyword.trim()
    if (!trimmed) return
    if (history.value[0] === trimmed) return
    commit([trimmed, ...history.value.filter((k) => k !== trimmed)].slice(0, MAX_ITEMS))
  }

  function remove(keyword: string) {
    if (!history.value.includes(keyword)) return
    commit(history.value.filter((k) => k !== keyword))
  }

  function clear() {
    if (!history.value.length) return
    commit([])
  }

  function suggestions(input: string): string[] {
    const q = input.trim().toLowerCase()
    if (!q) return history.value.slice(0, MAX_SUGGESTIONS)
    const out: string[] = []
    for (const k of history.value) {
      if (k.toLowerCase().includes(q)) {
        out.push(k)
        if (out.length >= MAX_SUGGESTIONS) break
      }
    }
    return out
  }

  const store: SearchHistoryStore = {
    history: computed(() => history.value) as Readonly<Ref<string[]>>,
    suggestions,
    add,
    remove,
    clear,
  }
  stores.set(key, store)
  return store
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
