<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted, onActivated, onDeactivated, nextTick } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { searchComics } from '@/services/api'
import type { ComicItem, SearchParams } from '@/types'
import { parseLikes } from '@/utils/likes'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import SearchForm from '@/components/SearchForm.vue'
import ComicGrid from '@/components/ComicGrid.vue'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

defineOptions({ name: 'SearchPage' })

const PAGE_SIZE = 20

const route = useRoute()
const router = useRouter()

const items = ref<ComicItem[]>([])
const loading = ref(false)
const error = ref('')
const hasSearched = ref(false)
const lastKeyword = ref('')
const minLikes = ref(0)
const sortOrder = ref('default')
const visibleCount = ref(PAGE_SIZE)
const { filterItems } = useTagBlacklist()

const processedItems = computed(() => {
  let result = items.value
  if (minLikes.value > 0) {
    result = result.filter((item) => parseLikes(item.likes) >= minLikes.value)
  }
  if (sortOrder.value === 'likes_desc') {
    result = [...result].sort((a, b) => parseLikes(b.likes) - parseLikes(a.likes))
  } else if (sortOrder.value === 'likes_asc') {
    result = [...result].sort((a, b) => parseLikes(a.likes) - parseLikes(b.likes))
  }
  result = filterItems(result)
  return result
})

const visibleItems = computed(() => processedItems.value.slice(0, visibleCount.value))

const hasMore = computed(() => visibleCount.value < processedItems.value.length)

watch([minLikes, sortOrder], () => {
  visibleCount.value = PAGE_SIZE
})

let sentinel: HTMLElement | null = null
let observer: IntersectionObserver | null = null

function loadMore() {
  if (!hasMore.value) return
  visibleCount.value = Math.min(visibleCount.value + PAGE_SIZE, processedItems.value.length)
  nextTick(() => {
    setupObserver()
  })
}

function setupObserver() {
  if (observer) {
    observer.disconnect()
    observer = null
  }
  sentinel = document.querySelector('.scroll-sentinel')
  if (!sentinel || !hasMore.value) return

  observer = new IntersectionObserver((entries) => {
    if (entries[0]?.isIntersecting) {
      loadMore()
    }
  }, { rootMargin: '200px' })

  observer.observe(sentinel)
}

function teardownObserver() {
  if (observer) {
    observer.disconnect()
    observer = null
  }
}

const SESSION_KEY = 'search_page_state'

function saveSearchState() {
  if (!hasSearched.value || loading.value) return
  try {
    sessionStorage.setItem(SESSION_KEY, JSON.stringify({
      items: items.value,
      lastKeyword: lastKeyword.value,
      minLikes: minLikes.value,
      sortOrder: sortOrder.value,
      visibleCount: visibleCount.value,
      hasSearched: hasSearched.value,
      classid: (route.query.classid as string) || '9',
      show: (route.query.show as string) || 'title,text,keyboard,ftitle',
      scrollY: window.scrollY,
    }))
  } catch {
    // ignore storage errors
  }
}

function restoreSearchState(): boolean {
  try {
    const raw = sessionStorage.getItem(SESSION_KEY)
    if (!raw) return false
    const state = JSON.parse(raw)
    items.value = state.items ?? []
    lastKeyword.value = state.lastKeyword ?? ''
    minLikes.value = state.minLikes ?? 0
    sortOrder.value = state.sortOrder ?? 'default'
    visibleCount.value = state.visibleCount ?? PAGE_SIZE
    hasSearched.value = state.hasSearched ?? false
    return true
  } catch {
    return false
  }
}

function restoreScrollPosition() {
  try {
    const raw = sessionStorage.getItem(SESSION_KEY)
    if (!raw) return
    const state = JSON.parse(raw)
    if (typeof state.scrollY === 'number') {
      nextTick(() => {
        window.scrollTo(0, state.scrollY)
      })
    }
  } catch {
    // ignore
  }
}

function restoreRouteQuery() {
  try {
    const raw = sessionStorage.getItem(SESSION_KEY)
    if (!raw) return
    const state = JSON.parse(raw)
    const keyword = state.lastKeyword ?? ''
    if (!keyword) return
    router.replace({
      name: 'search',
      query: {
        keyword,
        classid: state.classid ?? '9',
        show: state.show ?? 'title,text,keyboard,ftitle',
      },
    })
  } catch {
    // ignore
  }
}

function clearSavedState() {
  try {
    sessionStorage.removeItem(SESSION_KEY)
  } catch {
    // ignore
  }
}

async function handleSearch(params: SearchParams, likesFilter: number, order: string) {
  loading.value = true
  error.value = ''
  hasSearched.value = true
  lastKeyword.value = params.keyword
  minLikes.value = likesFilter
  sortOrder.value = order
  visibleCount.value = PAGE_SIZE
  try {
    const result = await searchComics(params)
    items.value = result.items
  } catch (e) {
    error.value = (e as Error).message
    items.value = []
  } finally {
    loading.value = false
    await nextTick()
    setupObserver()
  }
}

function searchFromQuery() {
  const keyword = route.query.keyword as string | undefined
  if (keyword) {
    const show = (route.query.show as string) || 'title,text,keyboard,ftitle'
    handleSearch(
      { keyword, classid: 9, show, tempid: '1' },
      0,
      'default',
    )
  }
}

onMounted(() => {
  const keyword = route.query.keyword as string | undefined
  if (keyword) {
    const saved = restoreSearchState()
    if (saved && lastKeyword.value === keyword) {
      restoreScrollPosition()
      nextTick(() => {
        setupObserver()
      })
    } else {
      clearSavedState()
      searchFromQuery()
    }
  } else if (restoreSearchState()) {
    restoreScrollPosition()
    nextTick(() => {
      setupObserver()
    })
  } else {
    searchFromQuery()
  }
})

onActivated(() => {
  const currentKeyword = route.query.keyword as string | undefined
  if (currentKeyword && currentKeyword !== lastKeyword.value) {
    clearSavedState()
    searchFromQuery()
    return
  }
  if (!currentKeyword && lastKeyword.value) {
    restoreRouteQuery()
  }
  restoreScrollPosition()
  nextTick(() => {
    setupObserver()
  })
})

onDeactivated(() => {
  saveSearchState()
  teardownObserver()
})

onUnmounted(() => {
  saveSearchState()
  teardownObserver()
})

const resultTitle = computed(() => {
  if (!hasSearched.value) return ''
  if (loading.value) return '搜索中...'
  return `"${lastKeyword.value}" 的搜索结果`
})

function goToSplit() {
  sessionStorage.setItem('split_items', JSON.stringify(processedItems.value))
  router.push({ name: 'split' })
}
</script>

<template>
  <div class="search-page">
    <div class="container">
      <SearchForm @search="handleSearch" />

      <div v-if="!hasSearched" class="search-intro">
        <EmptyState
          title="搜索漫画资源"
          message="输入关键词，选择分类和搜索范围开始搜索"
        />
      </div>

      <template v-else>
        <div v-if="loading" class="search-loading">
          <LoadingSpinner message="正在搜索..." />
        </div>

        <div v-else-if="error" class="search-error">
          <EmptyState title="搜索失败" :message="error" />
        </div>

        <div v-else>
          <div class="search-result-header">
            <h2 class="result-title">{{ resultTitle }}</h2>
            <button v-if="processedItems.length > 0" class="split-btn" @click="goToSplit">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M4 6h16M4 12h16M4 18h16" />
              </svg>
            </button>
          </div>

          <ComicGrid v-if="visibleItems.length > 0" :items="visibleItems" />

          <div v-if="hasMore" class="scroll-sentinel" />

          <div v-if="!hasMore && processedItems.length > PAGE_SIZE" class="no-more">
            <span>已加载全部 {{ processedItems.length }} 条结果</span>
          </div>

          <EmptyState
            v-if="processedItems.length === 0"
            title="未找到相关内容"
            message="请尝试更换关键词或调整搜索范围"
          />
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.search-page {
  padding-bottom: var(--spacing-xl);
}

.search-intro {
  padding-top: var(--spacing-xl);
}

.search-loading {
  padding-top: var(--spacing-xl);
}

.search-error {
  padding-top: var(--spacing-xl);
}

.search-result-header {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-md);
  padding-bottom: var(--spacing-sm);
  border-bottom: 1px solid var(--color-hairline);
  flex-wrap: wrap;
}

.result-title {
  font-size: var(--font-size-tagline);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  letter-spacing: -0.231px;
}

.split-btn {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: var(--spacing-xs) var(--spacing-md);
  margin-left: auto;
  color: var(--color-primary);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  background: transparent;
  border: 1px solid var(--color-primary);
  border-radius: var(--radius-pill);
  cursor: pointer;
  transition: all 0.2s;
  white-space: nowrap;
  flex-shrink: 0;
}

.split-btn:hover {
  background: var(--color-primary);
  color: var(--color-on-primary);
}

@media (max-width: 480px) {
  .search-result-header {
    flex-direction: column;
    align-items: flex-start;
    gap: var(--spacing-xs);
  }

  .split-btn {
    margin-left: 0;
    margin-top: var(--spacing-xs);
  }
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
</style>
