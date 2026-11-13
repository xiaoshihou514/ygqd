<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { ref, computed, watch, onMounted, onUnmounted, onActivated, onDeactivated, nextTick } from 'vue'
import { useRouter, onBeforeRouteLeave } from 'vue-router'
import { fetchCategoryList, fetchBikaCategories, fetchBikaLeaderboard } from '@/services/api'
import type { ComicItem } from '@/types'
import { parseLikes } from '@/utils/likes'
import { appendDistinct, interleaveSources } from '@/utils/merge'
import { HOME_TABS, normalizeCategoryLabel, type HomeTab } from '@/config/sources'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import { useComicViewedIds } from '@/composables/useComicViewedIds'
import MediaCollection from '@/components/MediaCollection.vue'
import { fromComic } from '@/utils/mediaItem'
import { useVideoFeed } from '@/composables/useVideoFeed'
import EmptyState from '@/components/EmptyState.vue'

defineOptions({ name: 'HomePage' })

const HOME_STATE_KEY = 'home_page_state_v2'
/** 「最新」混合流固定取 niacg 的漫画分类 */
const MIX_NIACG_CATEGORY = '9'
const BIKA_TABS_ERROR_PREFIX = '哔咔分类/榜单加载失败'

const router = useRouter()

// ============ 内容分段:漫画/视频 ============
type ContentSegment = 'comic' | 'video'
const SEGMENTS: { key: ContentSegment; label: string }[] = [
  { key: 'comic', label: '漫画' },
  { key: 'video', label: '视频' },
]
const segment = ref<ContentSegment>('comic')
const segmentIndex = computed(() => Math.max(0, SEGMENTS.findIndex((s) => s.key === segment.value)))
const videoFeed = useVideoFeed()
/** 视频段的滚动位置(漫画段按分类 tab 各自记在 TabState.scrollY) */
let videoScrollY = 0

function switchSegment(key: ContentSegment) {
  if (key === segment.value) return
  rememberScroll()
  segment.value = key
  if (key === 'video') videoFeed.ensureLoaded()
  restoreScroll()
}

// ============ 漫画分类 tab 与各 tab 数据 ============
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

// 加载/错误按 tab 记账:快速切 tab 时,旧 tab 的迟到响应只写回它自己的状态,
// 不会把新 tab 的骨架/错误条状态带乱
const loadingKeys = ref(new Set<string>())
const loadingMoreKeys = ref(new Set<string>())
const tabErrors = ref<Record<string, string>>({})
const loading = computed(() => loadingKeys.value.has(activeKey.value))
const loadingMore = computed(() => loadingMoreKeys.value.has(activeKey.value))
const error = computed(() => tabErrors.value[activeKey.value] ?? '')
/** 哔咔分类/榜单 tab 自身的加载失败(与各 tab 列表错误分开展示) */
const bikaTabsError = ref('')

const minLikes = ref(0)
const { filterItems } = useTagBlacklist()

const needsLogin = computed(() => activeTab.value.source === 'bika' && error.value.includes('登录'))

const filteredItems = computed(() => {
  let result = items.value
  if (activeTab.value.key === 'mix:' || activeTab.value.key === 'niacg:9') {
    result = result.filter((item) => item.source !== 'niacg' || item.categoryId !== 1)
  }
  if (minLikes.value > 0) {
    result = result.filter((item) => parseLikes(item.likes) >= minLikes.value)
  }
  return filterItems(result)
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

async function loadFirstPage(tab: HomeTab = activeTab.value) {
  const key = tab.key
  if (loadingKeys.value.has(key)) return
  loadingKeys.value.add(key)
  delete tabErrors.value[key]
  if (tab.source === 'mix') mixNotice.value = ''
  const state = stateFor(key)
  try {
    if (tab.source === 'mix') {
      state.mix = { niacg: { page: -1, hasNext: true }, bika: { page: -1, hasNext: true } }
      await loadMixPage(state, true)
    } else if (tab.rank) {
      // 排行榜无分页,一次返回整个榜
      const result = await fetchBikaLeaderboard(tab.rank)
      state.items = result.items
      state.hasNext = false
    } else {
      const result = await fetchCategoryList(tab.source, tab.category, 0)
      state.items = result.items
      state.hasNext = result.pagination.hasNext
    }
    state.page = 0
  } catch (e) {
    tabErrors.value[key] = errorMessage(e)
  } finally {
    loadingKeys.value.delete(key)
  }
}

async function loadNextPage() {
  const tab = activeTab.value
  const key = tab.key
  const state = stateFor(key)
  if (loadingMoreKeys.value.has(key) || loadingKeys.value.has(key) || !state.hasNext) return
  loadingMoreKeys.value.add(key)
  delete tabErrors.value[key]
  try {
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
    tabErrors.value[key] = errorMessage(e)
  } finally {
    loadingMoreKeys.value.delete(key)
  }
}

function retry() {
  void loadBikaTabs()
  void loadFirstPage()
}

function goToSettings() {
  router.push('/settings')
}

let bikaTabsPending = false
async function loadBikaTabs() {
  if (bikaTabsPending || tabs.value.some((tab) => tab.source === 'bika')) return
  bikaTabsPending = true
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
    bikaTabsError.value = ''
  } catch (e) {
    // 失败必弹(用户第二次反馈看不到榜单,静默吞错导致无法定位):
    // 页级提示条 + 诊断日志。「最新」与静态 tab 不受影响。
    const msg = errorMessage(e)
    bikaTabsError.value = `${BIKA_TABS_ERROR_PREFIX}:${msg}。请在设置中检查哔咔登录状态后重试`
    console.error('[home] 哔咔分类/榜单 tab 加载失败', msg)
  } finally {
    bikaTabsPending = false
  }
}

function switchTab(key: string) {
  if (key === activeKey.value) return
  rememberScroll()
  activeKey.value = key
  // 已有数据的 tab 直接展示缓存,不重抓;空 tab(含上次失败的)才发起首屏加载
  if (stateFor(key).items.length === 0) void loadFirstPage()
  restoreScroll()
}

// ============ 分类 tab 滑动指示器 + 选中项自动滚入可视区 ============
const tabsEl = ref<HTMLElement | null>(null)
const indicator = ref({ x: 0, width: 0 })
/** 首次定位不做过渡,避免指示器从最左侧滑入 */
const indicatorAnimated = ref(false)

function prefersReducedMotion(): boolean {
  return typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
}

function syncTabIndicator(scrollIntoView: boolean) {
  const box = tabsEl.value
  const btn = box?.querySelector<HTMLElement>('.content-tab.active')
  if (!box || !btn) {
    indicator.value = { x: 0, width: 0 }
    return
  }
  // 隐藏状态(keep-alive 失活、段切换中)量不到尺寸,保持原值
  if (btn.offsetWidth === 0) return
  indicator.value = { x: btn.offsetLeft, width: btn.offsetWidth }
  if (scrollIntoView) {
    const left = btn.offsetLeft - (box.clientWidth - btn.offsetWidth) / 2
    box.scrollTo({ left: Math.max(0, left), behavior: prefersReducedMotion() ? 'auto' : 'smooth' })
  }
  if (!indicatorAnimated.value) requestAnimationFrame(() => { indicatorAnimated.value = true })
}

watch(activeKey, () => nextTick(() => syncTabIndicator(true)))
watch(() => tabs.value.length, () => nextTick(() => syncTabIndicator(false)))

let tabsResizeObserver: ResizeObserver | null = null
watch(tabsEl, (el) => {
  tabsResizeObserver?.disconnect()
  tabsResizeObserver = null
  if (!el) return
  indicatorAnimated.value = false
  if (typeof ResizeObserver === 'function') {
    tabsResizeObserver = new ResizeObserver(() => syncTabIndicator(false))
    tabsResizeObserver.observe(el)
  }
  nextTick(() => syncTabIndicator(true))
})

// ============ 滚动位置记忆(每个分类 tab / 视频段各自一份) ============
function rememberScroll() {
  if (segment.value === 'video') videoScrollY = window.scrollY
  else activeState.value.scrollY = window.scrollY
}

function restoreScroll() {
  void nextTick(() => {
    window.scrollTo(0, segment.value === 'video' ? videoScrollY : activeState.value.scrollY)
  })
}

function saveHomeState() {
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

// 离开路由前记下滚动位置:此时旧页面仍在 DOM 中,window.scrollY 准确;
// 到 onDeactivated 时新页面已替换进来,滚动值可能已被页面高度截断
onBeforeRouteLeave(() => {
  rememberScroll()
})

onMounted(() => {
  void loadBikaTabs()
  if (restoreHomeState()) restoreScroll()
  else void loadFirstPage()
})

onActivated(() => {
  restoreScroll()
  nextTick(() => syncTabIndicator(false))
  // 从设置页登录回来:失败过的哔咔 tab 与空列表错误态自动重试一次
  if (bikaTabsError.value) void loadBikaTabs()
  if (segment.value === 'comic' && error.value && items.value.length === 0) void loadFirstPage()
})

onDeactivated(() => {
  saveHomeState()
})

onUnmounted(() => {
  tabsResizeObserver?.disconnect()
  saveHomeState()
})
</script>

<template>
  <div class="home-page">
    <div class="container">
      <div
        class="seg-control"
        role="tablist"
        aria-label="内容板块"
        :style="{ '--seg-count': SEGMENTS.length, '--seg-index': segmentIndex }"
      >
        <span class="seg-thumb" aria-hidden="true" />
        <button
          v-for="seg in SEGMENTS"
          :key="seg.key"
          type="button"
          class="seg-btn"
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
        <div class="page-header">
          <div
            ref="tabsEl"
            class="content-tabs"
            :class="{ animated: indicatorAnimated }"
            role="tablist"
            aria-label="内容分类"
          >
            <span
              v-show="indicator.width > 0"
              class="tab-indicator"
              aria-hidden="true"
              :style="{ width: `${indicator.width}px`, transform: `translateX(${indicator.x}px)` }"
            />
            <button
              v-for="tab in tabs"
              :key="tab.key"
              type="button"
              class="content-tab"
              :class="{ active: activeKey === tab.key }"
              role="tab"
              :aria-selected="activeKey === tab.key"
              @click="switchTab(tab.key)"
            >
              {{ tab.label }}
            </button>
            <span v-if="!loading" class="tabs-page-count">共 {{ filteredItems.length }} 部</span>
          </div>
          <div class="filter-group">
            <input
              v-model.number="minLikes"
              type="number"
              inputmode="numeric"
              class="filter-input"
              placeholder="最低星标数"
              aria-label="最低星标数"
              min="0"
            />
          </div>
        </div>

        <div v-if="bikaTabsError" class="home-notice" role="alert">
          <span class="home-notice-text">{{ bikaTabsError }}</span>
          <button type="button" class="home-notice-btn" @click="goToSettings">去设置</button>
          <button type="button" class="home-notice-btn" @click="loadBikaTabs">重试</button>
        </div>
        <div v-else-if="activeTab.source === 'mix' && mixNotice" class="home-notice">
          <span class="home-notice-text">{{ mixNotice }}</span>
          <button type="button" class="home-notice-btn" @click="goToSettings">去设置</button>
        </div>

        <div v-if="error && items.length === 0 && !loading" class="home-error">
          <EmptyState title="加载失败" :message="error">
            <div class="home-error-actions">
              <button v-if="needsLogin" type="button" class="retry-btn secondary" @click="goToSettings">
                去设置连接
              </button>
              <button type="button" class="retry-btn" @click="retry">
                重新加载
              </button>
            </div>
          </EmptyState>
        </div>

        <MediaCollection
          v-else
          :items="comicMediaItems"
          density="comic"
          :loading="loading"
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
    </div>
  </div>
</template>

<style scoped>
.home-page {
  padding-bottom: var(--spacing-xl);
}

/* ---- 内容分段控件(iOS 分段控件:滑块用 transform 平移) ---- */
.seg-control {
  position: relative;
  display: grid;
  grid-auto-flow: column;
  grid-auto-columns: 1fr;
  max-width: 240px;
  margin-bottom: var(--spacing-md);
  padding: 3px;
  border-radius: var(--radius-pill);
  background: var(--color-divider-soft);
}

.seg-thumb {
  position: absolute;
  top: 3px;
  bottom: 3px;
  left: 3px;
  width: calc((100% - 6px) / var(--seg-count));
  border-radius: var(--radius-pill);
  background: var(--color-card-bg);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.1);
  transform: translateX(calc(var(--seg-index) * 100%));
  transition: transform var(--duration-base) var(--ease-standard);
}

.seg-btn {
  position: relative;
  z-index: 1;
  min-height: 38px;
  border-radius: var(--radius-pill);
  color: var(--color-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  transition: color var(--duration-base) var(--ease-standard),
    opacity var(--duration-fast) var(--ease-standard);
}

.seg-btn.active {
  color: var(--color-text-primary);
}

/* ---- 分类 tab 与星标筛选:始终同一行,tab 区可收缩并横滑,筛选框固定宽度不收缩 ---- */
.page-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-lg);
  flex-wrap: nowrap;
}

.content-tabs {
  position: relative;
  display: flex;
  flex: 0 1 auto;
  min-width: 0;
  /* 横滑不换行:分类 tab 追加(哔咔登录后)不再把星标筛选挤到第二行 */
  flex-wrap: nowrap;
  overflow-x: auto;
  scrollbar-width: none;
  overscroll-behavior-x: contain;
  align-items: center;
  padding: 3px;
  border-radius: var(--radius-pill);
  background: var(--color-divider-soft);
}

.content-tabs::-webkit-scrollbar {
  display: none;
}

.tab-indicator {
  position: absolute;
  top: 3px;
  bottom: 3px;
  left: 0;
  border-radius: var(--radius-pill);
  background: var(--color-card-bg);
  box-shadow: 0 1px 4px rgba(0, 0, 0, 0.1);
  will-change: transform;
}

.content-tabs.animated .tab-indicator {
  transition: transform var(--duration-base) var(--ease-standard),
    width var(--duration-base) var(--ease-standard);
}

.content-tab {
  position: relative;
  z-index: 1;
  flex-shrink: 0;
  min-width: 66px;
  min-height: 38px;
  padding: 0 16px;
  border-radius: var(--radius-pill);
  color: var(--color-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  transition: color var(--duration-base) var(--ease-standard),
    opacity var(--duration-fast) var(--ease-standard);
}

.content-tab.active {
  color: var(--color-text-primary);
}

/* 分段/分类 tab 按下:滑块不随文字缩放,改为文字轻微变淡(iOS 分段控件手感) */
.seg-btn:active,
.content-tab:active {
  transform: none;
  opacity: 0.6;
}

.seg-btn:focus-visible,
.content-tab:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: -2px;
}

.tabs-page-count {
  flex-shrink: 0;
  padding: 0 var(--spacing-sm);
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
  white-space: nowrap;
}

.filter-group {
  margin-left: auto;
  flex-shrink: 0;
}

.filter-input {
  height: 44px;
  width: 140px;
  padding: 0 var(--spacing-md);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-fine-print);
  outline: none;
  transition: border-color var(--duration-base) var(--ease-standard);
  color: var(--color-text-primary);
  background: var(--color-canvas);
}

.filter-input:focus {
  border-color: var(--color-primary);
}

.filter-input::placeholder {
  color: var(--color-text-muted);
}

/* 窄屏只缩小筛选框,不换行 */
@media (max-width: 560px) {
  .filter-input {
    width: 104px;
    padding: 0 var(--spacing-sm);
  }
}

/* ---- 提示条(哔咔 tab 加载失败 / 混合流部分源未连接) ---- */
.home-notice {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-md);
  padding: var(--spacing-xxs) var(--spacing-xxs) var(--spacing-xxs) var(--spacing-md);
  border-radius: var(--radius-md);
  background: var(--color-canvas);
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
  animation: notice-in var(--duration-base) var(--ease-out);
}

.home-notice-text {
  flex: 1;
  min-width: 0;
}

.home-notice-btn {
  flex: none;
  min-height: 44px;
  padding: 0 var(--spacing-sm);
  border-radius: var(--radius-pill);
  color: var(--color-accent);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
}

@keyframes notice-in {
  from {
    opacity: 0;
    transform: translateY(-4px);
  }
}

/* ---- 整块错误态 ---- */
.home-error {
  padding-top: var(--spacing-xxl);
}

.home-error-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: center;
  gap: var(--spacing-sm);
}

.retry-btn {
  min-height: 44px;
  padding: 0 var(--spacing-lg);
  background: var(--color-primary);
  color: var(--color-on-primary);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-regular);
  transition: background-color var(--duration-base) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.retry-btn.secondary {
  background: var(--color-canvas);
  color: var(--color-primary);
}

.retry-btn:active {
  transform: scale(var(--press-scale));
}

@media (hover: hover) {
  .retry-btn:not(.secondary):hover {
    background: var(--color-primary-focus);
  }
}
</style>
