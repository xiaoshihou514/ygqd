import { ref } from 'vue'
import { fetchViewHistory, recordViewHistory as apiRecord } from '@/services/api'
import type { ViewHistoryEntry } from '@/types'

const history = ref<ViewHistoryEntry[]>([])
const loading = ref(false)
/** 并发 load(详情页 record 后重拉 + 历史页主动加载)只落地最后一次发起的结果 */
let loadSeq = 0

async function load(limit = 50): Promise<void> {
  const seq = ++loadSeq
  loading.value = true
  try {
    const list = await fetchViewHistory(limit)
    if (seq === loadSeq) history.value = list
  } finally {
    if (seq === loadSeq) loading.value = false
  }
}

export function useViewHistory() {
  async function record(entry: {
    source: string
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
