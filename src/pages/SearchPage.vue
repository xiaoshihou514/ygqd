<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { ref, computed, watch, onMounted, onUnmounted, onActivated, onDeactivated, nextTick } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { searchComics, searchVideos } from '@/services/api'
import { useVideoSearchScope } from '@/composables/useVideoSearchScope'
import type { ComicItem, SearchParams, SearchResult, VideoCard } from '@/types'
import { interleaveSources } from '@/utils/merge'
import { parseLikes } from '@/utils/likes'
import { formatCount } from '@/utils/formatCount'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import { useSearchHistory } from '@/composables/useSearchHistory'
import { useComicViewedIds } from '@/composables/useComicViewedIds'
import type { SourceId } from '@/config/sources'
import { parseSearchScope } from '@/config/sources'
import SearchForm from '@/components/SearchForm.vue'
import SearchInput from '@/components/SearchInput.vue'
import MediaCollection from '@/components/MediaCollection.vue'
import { fromComic, fromVideo } from '@/utils/mediaItem'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

defineOptions({ name: 'SearchPage' })

const PAGE_SIZE = 20
const MAX_PAGES = 50
const SEARCH_SOURCES = ['niacg', 'bika'] as const

const route = useRoute()
const router = useRouter()

const items = ref<ComicItem[]>([])
const viewedIds = useComicViewedIds(items)
const loading = ref(false)
const loadingMore = ref(false)
const error = ref('')
const hasSearched = ref(false)
const lastKeyword = ref('')
const minLikes = ref(0)
const sortOrder = ref('default')
const visibleCount = ref(PAGE_SIZE)
const currentPage = ref(0)
const hasNextPage = ref(false)
const lastParams = ref<SearchParams | null>(null)
const { filterItems, version: blacklistVersion } = useTagBlacklist()
const { add: addHistory } = useSearchHistory()
const fetchingProgress = ref('')

interface MergedSearchOutcome {
  items: ComicItem[]
  hasNext: boolean
  totalPages: number
  errors: string[]
}

function sourcesFor(params: SearchParams): SourceId[] {
  return params.source === 'all' ? [...SEARCH_SOURCES] : [params.source]
}

/** 双源并发搜索并交错合并;部分源失败不阻塞其余源 */
async function runSearch(
  params: SearchParams,
  page: number,
  cacheBuster?: string,
): Promise<MergedSearchOutcome> {
  const settled = await Promise.allSettled(
    sourcesFor(params).map((source) => searchComics(source, params, page, cacheBuster)),
  )
  const fulfilled = settled.filter(
    (result): result is PromiseFulfilledResult<SearchResult> => result.status === 'fulfilled',
  )
  const errors = settled
    .filter((result) => result.status === 'rejected')
    .map((result) => String((result as PromiseRejectedResult).reason))
  if (fulfilled.length === 0) {
    throw new Error(errors[0] || '搜索失败')
  }
  return {
    items: interleaveSources(fulfilled.map((result) => result.value.items)),
    hasNext: fulfilled.some((result) => result.value.pagination.hasNext),
    totalPages: Math.max(
      ...fulfilled.map((result) => result.value.pagination.total),
      1,
    ),
    errors,
  }
}

async function fetchAllPages(
  params: SearchParams,
  firstPageItems: ComicItem[],
  totalPages: number,
): Promise<ComicItem[]> {
  const pageCount = Math.min(totalPages, MAX_PAGES)
  const all = [...firstPageItems]

  for (let i = 1; i < pageCount; i += 5) {
    const batch: Promise<ComicItem[]>[] = []
    for (let j = i; j < Math.min(i + 5, pageCount); j++) {
      fetchingProgress.value = `正在获取全部结果（${j + 1}/${pageCount}）...`
      batch.push(runSearch(params, j).then((r) => r.items))
    }
    const results = await Promise.all(batch)
    all.push(...results.flat())
  }

  fetchingProgress.value = ''
  return all
}

const processedItems = computed(() => {
  let result = items.value
  if (minLikes.value > 0) {
    result = result.filter((item) => parseLikes(item.likes) >= minLikes.value)
  }
  if (sortOrder.value === 'likes_desc') {
    result = [...result].sort((a, b) => parseLikes(b.likes) - parseLikes(a.likes))
  }
  result = filterItems(result)
  return result
})

const visibleItems = computed(() => processedItems.value.slice(0, visibleCount.value))

const hasMoreLocal = computed(() => visibleCount.value < processedItems.value.length)
const comicMediaItems = computed(() =>
  visibleItems.value.map((c) => fromComic(c, { viewed: viewedIds.value.has(`${c.source}:${c.id}`) })),
)


async function loadMore() {
  if (loadingMore.value) return
  if (hasMoreLocal.value) {
    visibleCount.value = Math.min(visibleCount.value + PAGE_SIZE, processedItems.value.length)
    return
  }
  if (!hasNextPage.value || !lastParams.value) return
  loadingMore.value = true
  const nextPage = currentPage.value + 1
  try {
    const outcome = await runSearch(lastParams.value, nextPage)
    items.value = [...items.value, ...outcome.items]
    currentPage.value = nextPage
    hasNextPage.value = outcome.hasNext
    if (outcome.errors.length > 0) {
      error.value = outcome.errors[0]!
    }
  } catch {
    // silently ignore page load failure
  } finally {
    loadingMore.value = false
  }
}


watch([minLikes, sortOrder], () => {
  visibleCount.value = PAGE_SIZE
})

let prevBlacklistVersion = blacklistVersion.value
watch(blacklistVersion, (newVer) => {
  if (newVer !== prevBlacklistVersion && hasSearched.value && lastParams.value) {
    prevBlacklistVersion = newVer
    reSearchWithCacheBuster()
  }
})

async function reSearchWithCacheBuster() {
  if (!lastParams.value) return
  error.value = ''
  currentPage.value = 0
  hasNextPage.value = false
  visibleCount.value = PAGE_SIZE
  loadingMore.value = true
  const buster = String(Date.now())
  try {
    const outcome = await runSearch(lastParams.value, 0, buster)
    if (sortOrder.value === 'likes_desc') {
      items.value = await fetchAllPages(lastParams.value, outcome.items, outcome.totalPages)
      hasNextPage.value = false
    } else {
      items.value = outcome.items
      hasNextPage.value = outcome.hasNext
    }
    if (outcome.errors.length > 0) {
      error.value = outcome.errors[0]!
    }
  } catch (e) {
    error.value = errorMessage(e)
    items.value = []
  } finally {
    loadingMore.value = false
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
      currentPage: currentPage.value,
      hasNextPage: hasNextPage.value,
      hasSearched: hasSearched.value,
      classid: (route.query.classid as string) || '9',
      show: (route.query.show as string) || 'title,text,keyboard,ftitle',
      source: (route.query.source as string) || 'all',
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
    currentPage.value = state.currentPage ?? 0
    hasNextPage.value = state.hasNextPage ?? false
    hasSearched.value = state.hasSearched ?? false
    const savedClassid = state.classid as string | undefined
    const savedShow = state.show as string | undefined
    const savedSource = parseSearchScope((state.source as string | undefined) || 'all')
    if (lastKeyword.value) {
      lastParams.value = {
        source: savedSource,
        keyword: lastKeyword.value,
        classid: savedClassid ? Number(savedClassid) : 9,
        show: savedShow || 'title,text,keyboard,ftitle',
        tempid: '1',
      }
    }
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

function clearSavedState() {
  try {
    sessionStorage.removeItem(SESSION_KEY)
  } catch {
    // ignore
  }
}

// ============ 统一搜索:范围 chips(漫画默认=现有流程;视频/小说=并入) ============
type SearchScope2 = 'comic' | 'video'
const scope = ref<SearchScope2>('comic')
const SCOPES: Array<{ key: SearchScope2; label: string }> = [
  { key: 'comic', label: '漫画' },
  { key: 'video', label: '视频' },
]

// 三个范围共用一个关键词:在任一范围搜过的词,切到其它范围自动用同一个词搜
const sharedKeyword = ref((route.query.keyword as string | undefined) ?? '')
// 重构笔 4c:视频范围状态机迁移 useVideoSearchScope(哨兵重绑回调参数化)
const {
  videoSearchedKw, videoItems, videoPage, videoHasNext, videoSearching, videoError, videoMediaItems,
  handleVideoSearch, loadMoreVideos,
} = useVideoSearchScope(sharedKeyword, () => {})





async function handleSearch(params: SearchParams, likesFilter: number, order: string) {
  addHistory(params.keyword)
  sharedKeyword.value = params.keyword
  loading.value = true
  error.value = ''
  hasSearched.value = true
  lastKeyword.value = params.keyword
  minLikes.value = likesFilter
  sortOrder.value = order
  currentPage.value = 0
  hasNextPage.value = false
  visibleCount.value = PAGE_SIZE
  lastParams.value = params
  prevBlacklistVersion = blacklistVersion.value
  try {
    const outcome = await runSearch(params, 0)
    if (order === 'likes_desc') {
      items.value = await fetchAllPages(params, outcome.items, outcome.totalPages)
      hasNextPage.value = false
    } else {
      items.value = outcome.items
      hasNextPage.value = outcome.hasNext
    }
    if (outcome.errors.length > 0) {
      error.value = outcome.errors[0]!
    }
  } catch (e) {
    error.value = errorMessage(e)
    items.value = []
  } finally {
    loading.value = false
  }
}

function searchFromQuery() {
  const keyword = route.query.keyword as string | undefined
  if (keyword) {
    const show = (route.query.show as string) || 'title,text,keyboard,ftitle'
    const source = parseSearchScope((route.query.source as string) || 'all')
    handleSearch(
      { source, keyword, classid: 9, show, tempid: '1' },
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
    } else {
      clearSavedState()
      searchFromQuery()
    }
  } else if (restoreSearchState()) {
    restoreScrollPosition()
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
  restoreScrollPosition()
})

onDeactivated(() => {
  saveSearchState()
})

onUnmounted(() => {
  saveSearchState()
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
        <div class="scope-bar" role="tablist" aria-label="搜索范围">
        <button
          v-for="s in SCOPES"
          :key="s.key"
          type="button"
          class="scope-btn"
          :class="{ active: scope === s.key }"
          role="tab"
          :aria-selected="scope === s.key"
          @click="scope = s.key"
        >
          {{ s.label }}
        </button>
      </div>

      <template v-if="scope === 'comic'">
        <SearchForm @search="handleSearch" />
      </template>

      <template v-else-if="scope === 'video'">
        <SearchInput
          v-model="sharedKeyword"
          history-key="niacg-video-search-history"
          placeholder="搜索视频(番号/标题/厂商)…"
          :loading="videoSearching"
          @search="handleVideoSearch"
        />
        <div class="sort-row" role="group" aria-label="排序方式">
          <span class="sort-label">排序</span>
          <div class="sort-chips">
            <button
              v-for="s in VIDEO_SORTS"
              :key="s.key"
              type="button"
              class="sort-chip"
              :class="{ active: videoSort === s.key }"
              @click="setVideoSort(s.key)"
            >
              {{ s.label }}
            </button>
          </div>
        </div>
        <MediaCollection
          v-if="videoSearchedKw"
          :items="videoMediaItems"
          density="video"
          root-margin="400px"
          :loading="videoSearching && !videoItems.length"
          :loading-more="videoSearching && videoItems.length > 0"
          :has-more="videoHasNext"
          :error="videoError"
          empty-title="没有匹配的视频"
          empty-message="请尝试番号、日文标题或厂商名"
          class="scope-video-grid"
          @load-more="loadMoreVideos"
          @retry="videoItems.length ? loadMoreVideos() : handleVideoSearch()"
        />
      </template>


      <template v-if="scope === 'comic'">
      <div v-if="!hasSearched" class="search-intro">
        <EmptyState
          title="搜索漫画资源"
          message="输入关键词，选择分类和搜索范围开始搜索"
        />
      </div>

      <template v-else>
        <div v-if="loading" class="search-loading">
          <LoadingSpinner :message="fetchingProgress || '正在搜索...'" />
        </div>

        <div v-else-if="error && items.length === 0" class="search-error">
          <EmptyState title="搜索失败" :message="error" />
        </div>

        <div v-else>
          <div v-if="error && items.length > 0" class="search-error-bar">
            <span>{{ error }}</span>
          </div>
          <div class="search-result-header">
            <h2 class="result-title">{{ resultTitle }}</h2>
            <button v-if="processedItems.length > 0" class="split-btn" @click="goToSplit">
              <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M4 6h16M4 12h16M4 18h16" />
              </svg>
            </button>
          </div>

          <MediaCollection
            :items="comicMediaItems"
            density="comic"
            :loading-more="loadingMore"
            :has-more="hasMoreLocal || hasNextPage"
            :end-text="`已加载全部 ${processedItems.length} 条结果`"
            empty-title="未找到相关内容"
            empty-message="请尝试更换关键词或调整搜索范围"
            @load-more="loadMore"
          />
        </div>
      </template>
      </template>
    </div>
  </div>
</template>

<style scoped>
.search-page .container {
  max-width: none;
  padding: 0 var(--spacing-md);
}

.search-page {
  padding-bottom: var(--spacing-xl);
}


.scope-video-grid {
  margin-top: var(--spacing-md);
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



.search-error-bar {
  text-align: center;
  padding: var(--spacing-sm);
  margin-bottom: var(--spacing-md);
  font-size: var(--font-size-caption);
  color: var(--color-primary);
  background: var(--color-canvas-parchment);
  border-radius: var(--radius-sm);
}

/* ============ 统一搜索范围(批次 1):chips/表单/结果行/加载更多 ============ */
.scope-bar {
  display: flex;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-md);
}

.scope-btn {
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

.scope-btn.active {
  border-color: var(--color-primary);
  background: var(--color-primary);
  color: #fff;
}







/* 1920 横屏行长可读性:全宽背景下结果列限宽居中 */











.sort-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: var(--spacing-sm);
}

.sort-label {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
  flex-shrink: 0;
}

.sort-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.sort-chip {
  padding: 4px 14px;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  color: var(--color-text-secondary);
  font-size: var(--font-size-fine-print);
  cursor: pointer;
  transition: all 0.15s;
}

.sort-chip:hover {
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.sort-chip.active {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: #fff;
}

</style>
