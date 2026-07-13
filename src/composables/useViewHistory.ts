import { ref } from 'vue'
import { fetchViewHistory, recordViewHistory as apiRecord } from '@/services/api'
import type { ViewHistoryEntry } from '@/types'

const history = ref<ViewHistoryEntry[]>([])
const loading = ref(false)

export function useViewHistory() {
  async function load(limit = 50): Promise<void> {
    loading.value = true
    try {
      history.value = await fetchViewHistory(limit)
    } finally {
      loading.value = false
    }
  }

  async function record(entry: {
    comicId: string
    title: string
    thumbnail: string
    categoryId: number
    author: string
  }): Promise<void> {
    await apiRecord(entry)
    await load()
  }

  return { history, loading, load, record }
}
