import { ref } from 'vue'
import { fetchViewHistory, recordViewHistory as apiRecord } from '@/services/api'
import { parseSourceId } from '@/config/sources'
import type { ViewHistoryEntry } from '@/types'

const history = ref<ViewHistoryEntry[]>([])
const loading = ref(false)
/** 并发拉取只落地最后一次发起的结果 */
let loadSeq = 0
let pendingLoads = 0
let lastLimit = 50

async function fetchInto(limit: number, showLoading: boolean): Promise<void> {
  const seq = ++loadSeq
  pendingLoads++
  if (showLoading) loading.value = true
  try {
    const list = await fetchViewHistory(limit)
    if (seq === loadSeq) {
      history.value = list
      lastLimit = limit
    }
  } finally {
    pendingLoads--
    if (seq === loadSeq) loading.value = false
  }
}

function load(limit = 50): Promise<void> {
  return fetchInto(limit, true)
}

type RecordEntry = {
  source: string
  comicId: string
  title: string
  thumbnail: string
  categoryId: number
  author: string
}

export function useViewHistory() {
  /** 写入后本地 UPSERT 置顶(与 db::record 语义一致:同 source+comicId 只留一条,
   *  viewed_at 刷新为当前时间),省掉一次全量重拉;不翻 loading,页面不闪骨架。
   *  有拉取在途时其结果可能早于本次写入,改为静默重拉兜底。 */
  async function record(entry: RecordEntry): Promise<void> {
    await apiRecord(entry)
    if (pendingLoads > 0) {
      await fetchInto(lastLimit, false)
      return
    }
    const next: ViewHistoryEntry = {
      source: parseSourceId(entry.source),
      comicId: entry.comicId,
      title: entry.title,
      thumbnail: entry.thumbnail,
      categoryId: entry.categoryId,
      author: entry.author,
      viewedAt: Date.now(),
    }
    history.value = [
      next,
      ...history.value.filter((h) => !(h.source === next.source && h.comicId === next.comicId)),
    ].slice(0, lastLimit)
  }

  return { history, loading, load, record }
}
