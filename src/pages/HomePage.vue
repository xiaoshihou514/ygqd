<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, onActivated, onDeactivated, nextTick } from 'vue'
import { fetchCategoryList } from '@/services/api'
import type { ComicItem } from '@/types'
import { parseLikes } from '@/utils/likes'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import ComicGrid from '@/components/ComicGrid.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

defineOptions({ name: 'HomePage' })

const HOME_STATE_KEY = 'home_page_state'
const tabs = [
  { category: 9, label: '漫画' },
  { category: 1, label: 'COS' },
] as const
type HomeCategory = (typeof tabs)[number]['category']

interface TabState {
  items: ComicItem[]
  page: number
  hasNext: boolean
  scrollY: number
}

const activeCategory = ref<HomeCategory>(9)
const tabStates = ref<Record<HomeCategory, TabState>>({
  9: { items: [], page: 0, hasNext: true, scrollY: 0 },
  1: { items: [], page: 0, hasNext: true, scrollY: 0 },
})

const loading = ref(true)
const loadingMore = ref(false)
const error = ref('')
const minLikes = ref(0)
const { filterItems } = useTagBlacklist()

const activeState = computed(() => tabStates.value[activeCategory.value])
const items = computed(() => activeState.value.items)
const hasNext = computed(() => activeState.value.hasNext)

const filteredItems = computed(() => {
  let result = items.value
  if (activeCategory.value === 9) {
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
    const result = await fetchCategoryList(activeCategory.value, 0)
    activeState.value.items = result.items
    activeState.value.hasNext = result.pagination.hasNext
    activeState.value.page = 0
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
    const nextPage = activeState.value.page + 1
    const result = await fetchCategoryList(activeCategory.value, nextPage)
    activeState.value.items.push(...result.items)
    activeState.value.hasNext = result.pagination.hasNext
    activeState.value.page = nextPage
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

async function switchTab(category: HomeCategory) {
  if (category === activeCategory.value) return
  activeState.value.scrollY = window.scrollY
  activeCategory.value = category
  error.value = ''
  if (activeState.value.items.length === 0) {
    await loadFirstPage()
  }
  await nextTick()
  window.scrollTo(0, activeState.value.scrollY)
  setupObserver()
}

function saveHomeState() {
  activeState.value.scrollY = window.scrollY
  try {
    sessionStorage.setItem(HOME_STATE_KEY, JSON.stringify({
      activeCategory: activeCategory.value,
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
    if (saved.activeCategory === 9 || saved.activeCategory === 1) {
      activeCategory.value = saved.activeCategory
    }
    if (saved.tabStates?.[9] && saved.tabStates?.[1]) {
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
              :key="tab.category"
              class="content-tab"
              :class="{ active: activeCategory === tab.category }"
              role="tab"
              :aria-selected="activeCategory === tab.category"
              @click="switchTab(tab.category)"
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

.page-title {
  font-size: var(--font-size-hint);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
}

.content-tabs {
  display: inline-flex;
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
