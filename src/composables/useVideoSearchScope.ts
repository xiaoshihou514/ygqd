import { computed, ref, type Ref } from 'vue'
import { searchVideos } from '@/services/api'
import { errorMessage } from '@/utils/errors'
import { fromVideo } from '@/utils/mediaItem'
import type { VideoCard } from '@/types'

/**
 * 视频搜索范围状态机(重构笔 4c 迁入)。
 * 哨兵重绑依赖页面 DOM(搜索结果渲染后哨兵才存在)——搜索完成后调
 * onSearched 回调,页面在回调里 nextTick 重绑 observer。
 */
export function useVideoSearchScope(sharedKeyword: Ref<string>, onSearched: () => void) {
  const videoSearchedKw = ref('')
  const videoItems = ref<VideoCard[]>([])
  const videoPage = ref(1)
  const videoHasNext = ref(false)
  const videoSearching = ref(false)
  const videoError = ref('')
  const videoMediaItems = computed(() => videoItems.value.map((v) => fromVideo(v)))

  // ⑬ 视频搜索无限滚动:哨兵滚动加载(视频源无反爬闸,无节流)
  async function loadMoreVideos() {
    if (videoSearching.value || !videoHasNext.value) return
    videoSearching.value = true
    videoError.value = ''
    try {
      const res = await searchVideos(videoSearchedKw.value, videoPage.value + 1)
      const seen = new Set(videoItems.value.map((v) => v.code))
      videoItems.value.push(...res.items.filter((v) => !seen.has(v.code)))
      videoPage.value += 1
      videoHasNext.value = res.pagination.hasNext
    } catch (e) {
      videoError.value = errorMessage(e) || '加载更多失败'
    } finally {
      videoSearching.value = false
    }
  }

  async function handleVideoSearch() {
    const kw = sharedKeyword.value.trim()
    if (!kw || videoSearching.value) return
    videoSearching.value = true
    videoError.value = ''
    videoSearchedKw.value = kw
    videoItems.value = []
    try {
      const res = await searchVideos(kw, 1)
      videoItems.value = res.items
      videoPage.value = 1
      videoHasNext.value = res.pagination.hasNext
    } catch (e) {
      videoError.value = errorMessage(e) || '搜索失败'
    } finally {
      videoSearching.value = false
    }
  }

  return {
    videoSearchedKw,
    videoItems,
    videoPage,
    videoHasNext,
    videoSearching,
    videoError,
    videoMediaItems,
    loadMoreVideos,
    handleVideoSearch,
  }
}
