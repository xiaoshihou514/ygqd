<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, onActivated, onDeactivated, nextTick } from 'vue'
import { useRouter } from 'vue-router'
import { fetchCategoryList, fetchBikaCategories, fetchBikaLeaderboard } from '@/services/api'
import type { ComicItem } from '@/types'
import { parseLikes } from '@/utils/likes'
import { appendDistinct, interleaveSources } from '@/utils/merge'
import { HOME_TABS, normalizeCategoryLabel, type HomeTab } from '@/config/sources'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import ComicGrid from '@/components/ComicGrid.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import VideoBrowser from '@/components/VideoBrowser.vue'
import EmptyState from '@/components/EmptyState.vue'

defineOptions({ name: 'HomePage' })

const HOME_STATE_KEY = 'home_page_state_v2'
/** 「最新」混合流固定取 niacg 的漫画分类 */
/** 内容分段:漫画/视频/小说(小说源接入前的占位) */
type ContentSegment = 'comic' | 'video' | 'novel'
const segment = ref<ContentSegment>('comic')
const SEGMENTS: { key: ContentSegment; label: string }[] = [
  { key: 'comic', label: '漫画' },
  { key: 'video', label: '视频' },
  { key: 'novel', label: '小说' },
]
function switchSegment(key: ContentSegment) {
  segment.value = key
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
  activeTab.value.source === 'bika' && error.value.includes('登录'),
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
    error.value = (e as Error).message
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
    error.value = (e as Error).message
  } finally {
    loadingMore.value = false
  }
}

function setupObserver() {
  sentinel = document.querySelector('.scroll-sentinel')
  if (!sentinel) return

  observer = new IntersectionObserver((entries) => {
    if (entries[0]?.isIntersecting) {
      loadNextPage()
    }
  }, { rootMargin: '200px' })

  observer.observe(sentinel)
}

function retry() {
  loading.value = true
  error.value = ''
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
  } catch {
    // 源不可用(未登录/网络)时不追加其分类 tab,「最新」与静态 tab 不受影响
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
  setupObserver()
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

let sentinel: HTMLElement | null = null
let observer: IntersectionObserver | null = null

onMounted(() => {
  loadBikaTabs()
  const restored = restoreHomeState()
  const ready = restored ? Promise.resolve() : loadFirstPage()
  ready.then(() => {
    loading.value = false
    restoreHomeScroll()
    setupObserver()
  })
})

onActivated(() => {
  restoreHomeScroll()
  nextTick(setupObserver)
})

onDeactivated(() => {
  saveHomeState()
  observer?.disconnect()
})

onUnmounted(() => {
  saveHomeState()
  if (observer) {
    observer.disconnect()
    observer = null
  }
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

      <VideoBrowser v-if="segment === 'video'" />

      <EmptyState
        v-else-if="segment === 'novel'"
        title="小说板块准备中"
        message="小说源(hlib.cc)接入后开放"
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
          </div>
          <span class="page-count">共 {{ filteredItems.length }} 部</span>
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

        <ComicGrid :items="filteredItems" />

        <div v-if="loadingMore" class="loading-more">
          <LoadingSpinner message="正在加载更多..." />
        </div>

        <div
          v-if="hasNext"
          class="scroll-sentinel"
        />

        <div v-if="!hasNext && items.length > 0" class="no-more">
          <span>已加载全部漫画</span>
        </div>

        <div v-if="error && items.length > 0" class="load-error">
          <span>{{ error }}</span>
          <button class="retry-btn" @click="loadNextPage">
            重试
          </button>
        </div>

        <div v-if="items.length === 0" class="home-empty">
          <EmptyState title="暂无内容" message="未能获取到漫画内容" />
        </div>
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

.page-header {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-lg);
  flex-wrap: wrap;
}

.content-tabs {
  display: inline-flex;
  flex-wrap: wrap;
  align-items: center;
  padding: 3px;
  border-radius: var(--radius-pill);
  background: var(--color-divider-soft);
}

.content-tab {
  min-width: 66px;
  padding: 7px 16px;
  border-radius: var(--radius-pill);
  color: var(--color-text-secondary);
  background: transparent;
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  transition: color 0.18s, background 0.18s, box-shadow 0.18s;
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

.loading-more {
  padding: var(--spacing-lg) 0;
}

.scroll-sentinel {
  height: 1px;
}

.no-more {
  text-align: center;
  padding: var(--spacing-lg) 0;
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}

.load-error {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-md) 0;
  font-size: var(--font-size-caption);
  color: var(--color-error);
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
