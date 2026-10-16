<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, onActivated, onDeactivated, nextTick } from 'vue'
import { fetchCategoryList, fetchBikaCategories } from '@/services/api'
import type { ComicItem } from '@/types'
import { parseLikes } from '@/utils/likes'
import { HOME_TABS, type HomeTab } from '@/config/sources'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import ComicGrid from '@/components/ComicGrid.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

defineOptions({ name: 'HomePage' })

const HOME_STATE_KEY = 'home_page_state_v2'

interface TabState {
  items: ComicItem[]
  page: number
  hasNext: boolean
  scrollY: number
}

const tabs = ref<HomeTab[]>([...HOME_TABS])
const activeKey = ref(HOME_TABS[0]!.key)
const tabStates = ref<Record<string, TabState>>({})

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

const filteredItems = computed(() => {
  let result = items.value
  if (activeTab.value.key === 'niacg:9') {
    result = result.filter((item) => item.categoryId !== 1)
  }
  if (minLikes.value > 0) {
    result = result.filter((item) => parseLikes(item.likes) >= minLikes.value)
  }
  result = filterItems(result)
  return result
})

let sentinel: HTMLElement | null = null
let observer: IntersectionObserver | null = null

async function loadFirstPage() {
  loading.value = true
  error.value = ''
  try {
    const tab = activeTab.value
    const result = await fetchCategoryList(tab.source, tab.category, 0)
    const state = stateFor(tab.key)
    state.items = result.items
    state.hasNext = result.pagination.hasNext
    state.page = 0
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
    const nextPage = state.page + 1
    const result = await fetchCategoryList(tab.source, tab.category, nextPage)
    const seen = new Set(state.items.map((item) => `${item.source}:${item.id}`))
    state.items.push(...result.items.filter((item) => {
      const key = `${item.source}:${item.id}`
      if (seen.has(key)) return false
      seen.add(key)
      return true
    }))
    state.hasNext = result.pagination.hasNext
    state.page = nextPage
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

async function loadBikaTabs() {
  try {
    const categories = await fetchBikaCategories()
    const bikaIndex = tabs.value.findIndex((tab) => tab.key === 'bika:')
    if (bikaIndex < 0) return
    const dynamic = categories.map((category) => ({
      key: `bika:${category}`,
      label: category,
      source: 'bika' as const,
      category,
    }))
    tabs.value.splice(bikaIndex + 1, 0, ...dynamic)
  } catch {
    // 哔咔未登录或网络不可用时不追加分类 tab,保留「哔咔」全部 tab
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
      <div v-if="loading" class="home-loading">
        <LoadingSpinner message="正在加载漫画列表..." />
      </div>

      <div v-else-if="error && items.length === 0" class="home-error">
        <EmptyState title="加载失败" :message="error">
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
</style>
