import { computed, ref, shallowRef } from 'vue'
import { errorMessage } from '@/utils/errors'
import { fetchVideoList, fetchVideoWatchedIds } from '@/services/api'
import { fromVideo } from '@/utils/mediaItem'
import type { VideoCard } from '@/types'

/**
 * 视频最新流(原 VideoBrowser 的抓取逻辑):首页视频段与视频页共用,
 * 展示统一交给 MediaCollection。追加加载的拦截条件与原实现一致。
 *
 * 竞态:每次 load 取一个请求代号,reload 会让在途的旧请求(含翻页)作废,
 * 其响应与 loading 状态收尾都不再落地。
 */
export function useVideoFeed() {
  // 卡片只整体替换不就地改字段,浅响应即可(省掉逐卡深代理)
  const cards = shallowRef<VideoCard[]>([])
  const loading = ref(false)
  const loadingMore = ref(false)
  const loaded = ref(false)
  const error = ref('')
  const page = ref(1)
  const hasNext = ref(true)
  const watched = shallowRef<ReadonlySet<string>>(new Set())
  let requestId = 0

  const items = computed(() => {
    const seen = watched.value
    return cards.value.map((v) => fromVideo(v, { watched: seen.has(v.code) }))
  })

  /** 已看标记:每次落地后按当前全部卡片重查(含之前页——从详情页看完回来后标记得以刷新) */
  async function refreshWatched(id: number) {
    try {
      const hit = await fetchVideoWatchedIds(cards.value.map((v) => v.code))
      if (id === requestId) watched.value = new Set(hit)
    } catch {
      /* 查询失败不影响列表 */
    }
  }

  async function load(reset: boolean) {
    if (reset) {
      loading.value = true
      error.value = ''
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
    const id = ++requestId
    const targetPage = reset ? 1 : page.value
    try {
      const result = await fetchVideoList(targetPage)
      if (id !== requestId) return
      // 跨页(及页内)按番号去重:上游翻页期间有新片上架会让条目后移重现
      const seen = new Set(reset ? [] : cards.value.map((v) => v.code))
      const added = result.items.filter((v) => !seen.has(v.code) && (seen.add(v.code), true))
      if (reset) cards.value = added
      else if (added.length) cards.value = [...cards.value, ...added]
      hasNext.value = result.pagination.hasNext
      await refreshWatched(id)
      if (id !== requestId) return
      page.value = targetPage + 1
      loaded.value = true
    } catch (e) {
      if (id !== requestId) return
      error.value = errorMessage(e) || '加载失败'
    } finally {
      if (id === requestId) {
        loading.value = false
        loadingMore.value = false
      }
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
