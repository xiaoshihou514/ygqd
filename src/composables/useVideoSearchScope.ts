import { computed, ref, type Ref } from 'vue'
import { searchVideos } from '@/services/api'
import { errorMessage } from '@/utils/errors'
import { fromVideo } from '@/utils/mediaItem'
import type { VideoCard } from '@/types'

/**
 * 视频搜索范围状态机(重构笔 4c 迁入)。
 * 哨兵重绑依赖页面 DOM(搜索结果渲染后哨兵才存在)——搜索完成后调
 * onSearched 回调,页面在回调里 nextTick 重绑 observer。
 *
 * 竞态:每次请求取代号;新搜索(换关键词/切排序)作废在途的旧搜索与翻页,
 * 旧响应不落地、不收尾 loading,避免旧关键词结果混进新列表。
 */
export type VideoSort = '' | 'post_date'

export const VIDEO_SORTS: Array<{ key: VideoSort; label: string }> = [
  { key: '', label: '默认' },
  { key: 'post_date', label: '最近更新' },
]

export function useVideoSearchScope(sharedKeyword: Ref<string>, onSearched: () => void) {
  const videoSearchedKw = ref('')
  const videoSort = ref<VideoSort>('')
  const videoItems = ref<VideoCard[]>([])
  const videoPage = ref(1)
  const videoHasNext = ref(false)
  const videoSearching = ref(false)
  const videoError = ref('')
  const videoMediaItems = computed(() => videoItems.value.map((v) => fromVideo(v)))
  let requestId = 0

  // ⑬ 视频搜索无限滚动:哨兵滚动加载(视频源无反爬闸,无节流)
  async function loadMoreVideos() {
    if (videoSearching.value || !videoHasNext.value) return
    const id = ++requestId
    const kw = videoSearchedKw.value
    const nextPage = videoPage.value + 1
    videoSearching.value = true
    videoError.value = ''
    try {
      const res = await searchVideos(kw, nextPage, videoSort.value || undefined)
      if (id !== requestId) return
      const seen = new Set(videoItems.value.map((v) => v.code))
      const fresh = res.items.filter((v) => !seen.has(v.code) && (seen.add(v.code), true))
      if (fresh.length) videoItems.value = [...videoItems.value, ...fresh]
      videoPage.value = nextPage
      videoHasNext.value = res.pagination.hasNext
    } catch (e) {
      if (id !== requestId) return
      videoError.value = errorMessage(e) || '加载更多失败'
    } finally {
      if (id === requestId) videoSearching.value = false
    }
  }

  async function handleVideoSearch() {
    const kw = sharedKeyword.value.trim()
    if (!kw) return
    // 同关键词的首搜在途:重复提交直接忽略;换关键词则新搜索接管(作废在途请求)
    if (videoSearching.value && kw === videoSearchedKw.value && !videoItems.value.length) return
    await runSearch(kw)
  }

  async function runSearch(kw: string) {
    const id = ++requestId
    videoSearching.value = true
    videoError.value = ''
    videoSearchedKw.value = kw
    videoItems.value = []
    videoHasNext.value = false
    try {
      const res = await searchVideos(kw, 1, videoSort.value || undefined)
      if (id !== requestId) return
      const seen = new Set<string>()
      videoItems.value = res.items.filter((v) => !seen.has(v.code) && (seen.add(v.code), true))
      videoPage.value = 1
      videoHasNext.value = res.pagination.hasNext
    } catch (e) {
      if (id !== requestId) return
      videoError.value = errorMessage(e) || '搜索失败'
    } finally {
      if (id === requestId) {
        videoSearching.value = false
        onSearched()
      }
    }
  }

  /** 切换排序即重搜(排序变化影响结果序) */
  function setVideoSort(key: VideoSort) {
    if (key === videoSort.value) return
    videoSort.value = key
    // 按当前展示结果的关键词重搜(输入框可能已被改动/清空)
    if (videoSearchedKw.value) void runSearch(videoSearchedKw.value)
  }

  return {
    videoSearchedKw,
    videoSort,
    videoItems,
    videoPage,
    videoHasNext,
    videoSearching,
    videoError,
    videoMediaItems,
    loadMoreVideos,
    handleVideoSearch,
    setVideoSort,
  }
}
