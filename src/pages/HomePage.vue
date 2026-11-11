<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { ref, computed, onMounted, onUnmounted, onActivated, onDeactivated, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { fetchCategoryList, fetchBikaCategories, fetchBikaLeaderboard } from '@/services/api'
import type { ComicItem } from '@/types'
import { parseLikes } from '@/utils/likes'
import { appendDistinct, interleaveSources } from '@/utils/merge'
import { HOME_TABS, normalizeCategoryLabel, type HomeTab } from '@/config/sources'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import { useComicViewedIds } from '@/composables/useComicViewedIds'
import MediaCollection from '@/components/MediaCollection.vue'
import { fromComic } from '@/utils/mediaItem'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import { useVideoFeed } from '@/composables/useVideoFeed'
import EmptyState from '@/components/EmptyState.vue'

defineOptions({ name: 'HomePage' })

const HOME_STATE_KEY = 'home_page_state_v2'
/** 「最新」混合流固定取 niacg 的漫画分类 */
/** 内容分段:漫画/视频/小说(小说源接入前的占位) */
type ContentSegment = 'comic' | 'video'
const segment = ref<ContentSegment>('comic')
const videoFeed = useVideoFeed()
const SEGMENTS: { key: ContentSegment; label: string }[] = [
  { key: 'comic', label: '漫画' },
  { key: 'video', label: '视频' },
]
function switchSegment(key: ContentSegment) {
  segment.value = key
  if (key === 'video') videoFeed.ensureLoaded()
}

const MIX_NIACG_CATEGORY = '9'

interface SourcePageState {
  page: number
  hasNext: boolean
}

interface TabState {
  items: ComicItem[]
  page: number
  hasNext: boolean
  scrollY: number
  mix?: { niacg: SourcePageState; bika: SourcePageState }
}

const router = useRouter()

const tabs = ref<HomeTab[]>([...HOME_TABS])
const activeKey = ref(HOME_TABS[0]!.key)
const tabStates = ref<Record<string, TabState>>({})
const mixNotice = ref('')

function stateFor(key: string): TabState {
  if (!tabStates.value[key]) {
    tabStates.value[key] = { items: [], page: 0, hasNext: true, scrollY: 0 }
  }
  return tabStates.value[key]!
}

const activeTab = computed(() =>
  tabs.value.find((tab) => tab.key === activeKey.value) ?? tabs.value[0]!,
)
const activeState = computed(() => stateFor(activeKey.value))
const items = computed(() => activeState.value.items)
const hasNext = computed(() => activeState.value.hasNext)

const loading = ref(true)
const loadingMore = ref(false)
const error = ref('')
const minLikes = ref(0)
const { filterItems } = useTagBlacklist()

const needsLogin = computed(() =>
  (activeTab.value.source === 'bika' && error.value.includes('登录')) ||
  error.value.startsWith('哔咔分类/榜单加载失败'),
)

const filteredItems = computed(() => {
  let result = items.value
  if (activeTab.value.key === 'mix:' || activeTab.value.key === 'niacg:9') {
    result = result.filter((item) => item.source !== 'niacg' || item.categoryId !== 1)
  }
  if (minLikes.value > 0) {
    result = result.filter((item) => parseLikes(item.likes) >= minLikes.value)
  }
  result = filterItems(result)
  return result
})

const viewedIds = useComicViewedIds(filteredItems)
const comicMediaItems = computed(() =>
  filteredItems.value.map((c) => fromComic(c, { viewed: viewedIds.value.has(`${c.source}:${c.id}`) })),
)

/** 混合流:两个源同页并发交错;哔咔不可用时安静降级为单源 */
async function loadMixPage(state: TabState, reset: boolean) {
  const mix = state.mix ?? { niacg: { page: -1, hasNext: true }, bika: { page: -1, hasNext: true } }
  const fetches: { source: 'niacg' | 'bika'; page: number; request: Promise<ComicItem[]> }[] = []
  for (const source of ['niacg', 'bika'] as const) {
    if (!mix[source].hasNext) continue
    fetches.push({
      source,
      page: mix[source].page + 1,
      request: fetchCategoryList(source, source === 'niacg' ? MIX_NIACG_CATEGORY : '', mix[source].page + 1)
        .then((result) => result.items),
    })
  }
  if (fetches.length === 0) {
    state.hasNext = false
    return
  }
  const settled = await Promise.allSettled(fetches.map((fetch) => fetch.request))
  const lists: ComicItem[][] = []
  fetches.forEach((fetch, index) => {
    const result = settled[index]!
    if (result.status === 'fulfilled') {
      mix[fetch.source] = { page: fetch.page, hasNext: true }
      lists.push(result.value)
    } else {
      mix[fetch.source] = { page: mix[fetch.source].page, hasNext: false }
      if (fetch.source === 'bika') {
        mixNotice.value = '部分内容源未连接，可在设置中登录后混入更多最新内容'
      }
    }
  })
  state.mix = mix
  if (reset) {
    state.items = interleaveSources(lists)
  } else {
    appendDistinct(state.items, interleaveSources(lists))
  }
  state.hasNext = mix.niacg.hasNext || mix.bika.hasNext
  if (settled.every((result) => result.status === 'rejected')) {
    throw new Error(String((settled[0] as PromiseRejectedResult).reason))
  }
}

async function loadFirstPage() {
  loading.value = true
  error.value = ''
  if (activeTab.value.key === 'mix:') {
    mixNotice.value = ''
  }
  try {
    const tab = activeTab.value
    const state = stateFor(tab.key)
    if (tab.source === 'mix') {
      state.mix = { niacg: { page: -1, hasNext: true }, bika: { page: -1, hasNext: true } }
      await loadMixPage(state, true)
      state.page = 0
    } else if (tab.rank) {
      // 排行榜无分页,一次返回整个榜
      const result = await fetchBikaLeaderboard(tab.rank)
      state.items = result.items
      state.hasNext = false
      state.page = 0
    } else {
      const result = await fetchCategoryList(tab.source, tab.category, 0)
      state.items = result.items
      state.hasNext = result.pagination.hasNext
      state.page = 0
    }
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    loading.value = false
  }
}

async function loadNextPage() {
  if (loadingMore.value || !hasNext.value || loading.value) return
  loadingMore.value = true
  try {
    const tab = activeTab.value
    const state = stateFor(tab.key)
    if (tab.source === 'mix') {
      await loadMixPage(state, false)
    } else {
      const nextPage = state.page + 1
      const result = await fetchCategoryList(tab.source, tab.category, nextPage)
      appendDistinct(state.items, result.items)
      state.hasNext = result.pagination.hasNext
      state.page = nextPage
    }
  } catch (e) {
    error.value = errorMessage(e)
  } finally {
    loadingMore.value = false
  }
}

function retry() {
  loading.value = true
  error.value = ''
  void loadBikaTabs()
  loadFirstPage()
}

function goToSettings() {
  router.push('/settings')
}

async function loadBikaTabs() {
  try {
    const categories = await fetchBikaCategories()
    if (tabs.value.some((tab) => tab.source === 'bika')) return
    const rankTabs: HomeTab[] = [
      { key: 'bika-rank:H24', label: '日榜', source: 'bika', category: '', rank: 'H24' },
      { key: 'bika-rank:D7', label: '周榜', source: 'bika', category: '', rank: 'D7' },
      { key: 'bika-rank:D30', label: '月榜', source: 'bika', category: '', rank: 'D30' },
    ]
    tabs.value.push(
      ...rankTabs,
      ...categories.map((category) => ({
        key: `bika:${category}`,
        label: normalizeCategoryLabel(category),
        source: 'bika' as const,
        category,
      })),
    )
    // 成功后清除 loadBikaTabs 自己设置的失败提示(不影响其它 tab 的错误)
    if (error.value.startsWith('哔咔分类/榜单加载失败')) error.value = ''
  } catch (e) {
    // 失败必弹(用户第二次反馈看不到榜单,静默吞错导致无法定位):
    // 页级错误条提示 + 诊断日志。「最新」与静态 tab 不受影响。
    const msg = errorMessage(e)
    error.value = `哔咔分类/榜单加载失败:${msg}。请在设置中检查哔咔登录状态后重试`
    console.error('[home] 哔咔分类/榜单 tab 加载失败', msg)
  }
}

async function switchTab(key: string) {
  if (key === activeKey.value) return
  activeState.value.scrollY = window.scrollY
  activeKey.value = key
  error.value = ''
  if (stateFor(key).items.length === 0) {
    await loadFirstPage()
  }
  await nextTick()
  window.scrollTo(0, stateFor(key).scrollY)
}

function saveHomeState() {
  activeState.value.scrollY = window.scrollY
  try {
    sessionStorage.setItem(HOME_STATE_KEY, JSON.stringify({
      activeKey: activeKey.value,
      tabStates: tabStates.value,
      minLikes: minLikes.value,
    }))
  } catch {
    // ignore storage errors
  }
}

function restoreHomeState(): boolean {
  try {
    const raw = sessionStorage.getItem(HOME_STATE_KEY)
    if (!raw) return false
    const saved = JSON.parse(raw)
    if (typeof saved.activeKey === 'string') {
      activeKey.value = saved.activeKey
    }
    if (saved.tabStates && typeof saved.tabStates === 'object') {
      tabStates.value = saved.tabStates
    }
    minLikes.value = saved.minLikes ?? 0
    return activeState.value.items.length > 0
  } catch {
    return false
  }
}

function restoreHomeScroll() {
  nextTick(() => window.scrollTo(0, activeState.value.scrollY))
}


onMounted(() => {
  loadBikaTabs()
  const restored = restoreHomeState()
  const ready = restored ? Promise.resolve() : loadFirstPage()
  ready.then(() => {
    loading.value = false
    restoreHomeScroll()
  })
})

onActivated(() => {
  restoreHomeScroll()
})

onDeactivated(() => {
  saveHomeState()
})

onUnmounted(() => {
  saveHomeState()
})
</script>

<template>
  <div class="home-page">
    <div class="container">
      <div class="segment-bar" role="tablist" aria-label="内容板块">
        <button
          v-for="seg in SEGMENTS"
          :key="seg.key"
          class="segment-btn"
          :class="{ active: segment === seg.key }"
          role="tab"
          :aria-selected="segment === seg.key"
          @click="switchSegment(seg.key)"
        >
          {{ seg.label }}
        </button>
      </div>

      <MediaCollection
        v-if="segment === 'video'"
        :items="videoFeed.items.value"
        density="video"
        root-margin="400px"
        :loading="videoFeed.loading.value"
        :loading-more="videoFeed.loadingMore.value"
        :has-more="videoFeed.hasNext.value"
        :error="videoFeed.error.value"
        @load-more="videoFeed.loadMore"
        @retry="videoFeed.items.value.length ? videoFeed.loadMore() : videoFeed.reload()"
      />


      <template v-else>
      <div v-if="loading" class="home-loading">
        <LoadingSpinner message="正在加载漫画列表..." />
      </div>

      <div v-else-if="error && items.length === 0" class="home-error">
        <EmptyState title="加载失败" :message="error">
          <button v-if="needsLogin" class="retry-btn" @click="goToSettings">
            去设置连接
          </button>
          <button class="retry-btn" @click="retry">
            重新加载
          </button>
        </EmptyState>
      </div>

      <template v-else>
        <div class="page-header">
          <div class="content-tabs" role="tablist" aria-label="内容分类">
            <button
              v-for="tab in tabs"
              :key="tab.key"
              class="content-tab"
              :class="{ active: activeKey === tab.key }"
              role="tab"
              :aria-selected="activeKey === tab.key"
              @click="switchTab(tab.key)"
            >
              {{ tab.label }}
            </button>
            <span class="tabs-page-count">共 {{ filteredItems.length }} 部</span>
          </div>
          <div class="filter-group">
            <input
              v-model.number="minLikes"
              type="number"
              class="filter-input"
              placeholder="最低星标数"
              min="0"
            />
          </div>
        </div>

        <div v-if="activeTab.source === 'mix' && mixNotice" class="mix-notice">
          <span>{{ mixNotice }}</span>
          <button class="mix-notice-btn" @click="goToSettings">去设置</button>
        </div>

        <MediaCollection
          :items="comicMediaItems"
          density="comic"
          :loading-more="loadingMore"
          :has-more="hasNext"
          :error="error"
          end-text="已加载全部漫画"
          empty-title="暂无内容"
          :empty-message="items.length ? '当前星标筛选下没有内容' : '未能获取到漫画内容'"
          @load-more="loadNextPage"
          @retry="loadNextPage"
        />
      </template>
      </template>
    </div>
  </div>
</template>

<style scoped>
.home-page {
  padding-bottom: var(--spacing-xl);
}

.home-loading {
  padding-top: var(--spacing-xxl);
}

.home-error,
.home-empty {
  padding-top: var(--spacing-xxl);
}

.home-error .retry-btn + .retry-btn {
  margin-left: var(--spacing-sm);
}

/* 分类 tab(含「共X部」)与星标筛选始终同一行:tab 区可收缩并横滑,
   筛选框固定宽度不收缩——tab 再多也不会把筛选框挤到下一行 */
.page-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-lg);
  flex-wrap: nowrap;
}

.content-tabs {
  display: flex;
  flex: 0 1 auto;
  min-width: 0;
  /* 横滑不换行:分类 tab 追加(哔咔登录后)不再把星标筛选挤到第二行 */
  flex-wrap: nowrap;
  overflow-x: auto;
  scrollbar-width: none;
  align-items: center;
  padding: 3px;
  border-radius: var(--radius-pill);
  background: var(--color-divider-soft);
}

.content-tabs::-webkit-scrollbar {
  display: none;
}

.content-tab {
  flex-shrink: 0;
  min-width: 66px;
  padding: 7px 16px;
  border-radius: var(--radius-pill);
  color: var(--color-text-secondary);
  background: transparent;
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  transition: color 0.18s, background 0.18s, box-shadow 0.18s;
}

.tabs-page-count {
  flex-shrink: 0;
  padding: 7px 12px;
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
  white-space: nowrap;
}

.content-tab.active {
  color: var(--color-text-primary);
  background: var(--color-card-bg);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.1);
}

.content-tab:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: 2px;
}

.page-count {
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}

.mix-notice {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-md);
  padding: var(--spacing-xs) var(--spacing-md);
  border-radius: var(--radius-md);
  background: var(--color-canvas);
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
}

.mix-notice-btn {
  flex: none;
  margin-left: auto;
  padding: 2px 10px;
  border-radius: var(--radius-pill);
  color: var(--color-accent);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  background: transparent;
}





.retry-btn {
  padding: var(--spacing-xs) var(--spacing-lg);
  background: var(--color-primary);
  color: var(--color-on-primary);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-regular);
  transition: background 0.2s;
}

.retry-btn:hover {
  background: var(--color-primary-focus);
}

.filter-group {
  margin-left: auto;
  flex-shrink: 0;
}

/* 窄屏只缩小筛选框,不换行 */
@media (max-width: 560px) {
  .filter-input {
    width: 104px;
    padding: 0 var(--spacing-sm);
  }
}

.filter-input {
  height: 36px;
  width: 140px;
  padding: 0 var(--spacing-md);
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-fine-print);
  outline: none;
  transition: border-color 0.2s;
  color: var(--color-text-primary);
  background: var(--color-canvas);
}

.filter-input:focus {
  border-color: var(--color-primary);
}

.filter-input::placeholder {
  color: var(--color-text-muted);
}
.segment-bar {
  display: flex;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-md);
}

.segment-btn {
  flex: 1;
  max-width: 120px;
  padding: 6px 0;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  color: var(--color-text-secondary);
  font-size: var(--font-size-caption);
  cursor: pointer;
  transition: all 0.15s ease;
}

.segment-btn.active {
  border-color: var(--color-primary);
  background: var(--color-primary);
  color: #fff;
}
</style>
