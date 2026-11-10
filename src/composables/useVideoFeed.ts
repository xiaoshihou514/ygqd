import { computed, ref } from 'vue'
import { errorMessage } from '@/utils/errors'
import { fetchVideoList, fetchVideoWatchedIds } from '@/services/api'
import { fromVideo } from '@/utils/mediaItem'
import type { VideoCard } from '@/types'

/**
 * 视频最新流(原 VideoBrowser 的抓取逻辑):首页视频段与视频页共用,
 * 展示统一交给 MediaCollection。追加加载的拦截条件与原实现一致。
 */
export function useVideoFeed() {
  const cards = ref<VideoCard[]>([])
  const loading = ref(false)
  const loadingMore = ref(false)
  const loaded = ref(false)
  const error = ref('')
  const page = ref(1)
  const hasNext = ref(true)
  const watched = ref<Set<string>>(new Set())

  const items = computed(() => cards.value.map((v) => fromVideo(v, { watched: watched.value.has(v.code) })))

  async function load(reset: boolean) {
    if (reset) {
      loading.value = true
      error.value = ''
      page.value = 1
    } else {
      // 追加加载拦截:并发锁 / 无下一页 / 首载中 / 无数据(首载失败或空结果,防止悄悄拉下一页)
      const blocked = loadingMore.value || !hasNext.value || loading.value || !cards.value.length
      if (blocked) {
        console.info('[video-feed] 追加加载拦截', {
          loadingMore: loadingMore.value,
          hasNext: hasNext.value,
          loading: loading.value,
          items: cards.value.length,
        })
        return
      }
      loadingMore.value = true
      error.value = ''
    }
    try {
      const result = await fetchVideoList(page.value)
      if (reset) {
        cards.value = result.items
      } else {
        const seen = new Set(cards.value.map((v) => v.code))
        cards.value.push(...result.items.filter((v) => !seen.has(v.code)))
      }
      hasNext.value = result.pagination.hasNext
      try {
        watched.value = new Set(await fetchVideoWatchedIds(cards.value.map((v) => v.code)))
      } catch {
        /* 查询失败不影响列表 */
      }
      page.value += 1
      loaded.value = true
    } catch (e) {
      error.value = errorMessage(e) || '加载失败'
    } finally {
      loading.value = false
      loadingMore.value = false
    }
  }

  /** 首次显示时加载(已加载过则保留,切回不重抓) */
  function ensureLoaded() {
    if (!loaded.value && !loading.value) void load(true)
  }

  return {
    items,
    loading,
    loadingMore,
    error,
    hasNext,
    ensureLoaded,
    reload: () => load(true),
    loadMore: () => load(false),
  }
}
