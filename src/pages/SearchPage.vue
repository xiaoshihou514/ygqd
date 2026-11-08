<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { ref, computed, watch, onMounted, onUnmounted, onActivated, onDeactivated, nextTick } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { novelAdd, novelSearchPage, searchComics, searchVideos } from '@/services/api'
import type { ComicItem, NovelHit, SearchParams, SearchResult, VideoCard } from '@/types'
import { interleaveSources } from '@/utils/merge'
import { parseLikes } from '@/utils/likes'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import { useSearchHistory } from '@/composables/useSearchHistory'
import { useComicViewedIds } from '@/composables/useComicViewedIds'
import type { SourceId } from '@/config/sources'
import { parseSearchScope } from '@/config/sources'
import SearchForm from '@/components/SearchForm.vue'
import SearchInput from '@/components/SearchInput.vue'
import ComicGrid from '@/components/ComicGrid.vue'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import VideoCardGrid from '@/components/VideoCardGrid.vue'

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

let sentinel: HTMLElement | null = null
let observer: IntersectionObserver | null = null

async function loadMore() {
  if (loadingMore.value) return
  if (hasMoreLocal.value) {
    visibleCount.value = Math.min(visibleCount.value + PAGE_SIZE, processedItems.value.length)
    await nextTick()
    setupObserver()
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
    await nextTick()
    setupObserver()
  }
}

function setupObserver() {
  teardownObserver()
  sentinel = document.querySelector('.scroll-sentinel')
  if (!sentinel) return

  // ⑬ 三范围统一哨兵:视频范围哨兵驱动 loadMoreVideos,漫画范围驱动 loadMore
  if (scope.value === 'video') {
    if (!videoHasNext.value) return
    observer = new IntersectionObserver((entries) => {
      if (entries[0]?.isIntersecting) {
        void loadMoreVideos()
      }
    }, { rootMargin: '400px' })
    observer.observe(sentinel)
    return
  }

  if (!hasMoreLocal.value && !hasNextPage.value) return

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
    await nextTick()
    setupObserver()
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
type SearchScope2 = 'comic' | 'video' | 'novel'
const scope = ref<SearchScope2>('comic')
const SCOPES: Array<{ key: SearchScope2; label: string }> = [
  { key: 'comic', label: '漫画' },
  { key: 'video', label: '视频' },
  { key: 'novel', label: '小说' },
]

// 三个范围共用一个关键词:在任一范围搜过的词,切到其它范围自动用同一个词搜
const sharedKeyword = ref((route.query.keyword as string | undefined) ?? '')
const videoSearchedKw = ref('')
const novelSearchedKw = ref('')

watch(scope, (s) => {
  const kw = sharedKeyword.value.trim()
  if (!kw) return
  if (s === 'video' && videoSearchedKw.value !== kw) {
    void handleVideoSearch().then(() => nextTick(() => setupObserver()))
    return
  }
  if (s === 'novel' && novelSearchedKw.value !== kw) void handleNovelSearch()
})

// —— 视频范围 ——
const videoItems = ref<VideoCard[]>([])
const videoPage = ref(1)
const videoHasNext = ref(false)
const videoSearching = ref(false)
const videoError = ref('')

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
    if (!res.items.length) videoError.value = '没有匹配的视频'
  } catch (e) {
    videoError.value = errorMessage(e) || '搜索失败'
  } finally {
    videoSearching.value = false
  }
}

// —— 小说范围(3 秒限速:同站点反爬闸) ——
const novelSearching = ref(false)
const novelHits = ref<NovelHit[]>([])
const novelPageNo = ref(1)
const novelTotalPages = ref(1)
const novelLoadingMore = ref(false)
const novelError = ref('')
// 高级搜索:排序四维(非法值后端忽略,透传即可)
type NovelSort = 'date' | 'total' | 'char' | 'like'
const novelSort = ref<NovelSort>('date')
const NOVEL_SORTS: Array<{ key: NovelSort; label: string }> = [
  { key: 'date', label: '最新' },
  { key: 'total', label: '总字数' },
  { key: 'char', label: '字数' },
  { key: 'like', label: '最多点赞' },
]
let lastNovelFetchAt = 0
const NOVEL_MIN_INTERVAL = 3000

async function handleNovelSearch() {
  const kw = sharedKeyword.value.trim()
  if (!kw || novelSearching.value || novelLoadingMore.value) return
  novelError.value = ''
  novelSearching.value = true
  novelSearchedKw.value = kw
  novelHits.value = []
  try {
    const elapsed = Date.now() - lastNovelFetchAt
    if (elapsed < NOVEL_MIN_INTERVAL) {
      await new Promise((resolve) => setTimeout(resolve, NOVEL_MIN_INTERVAL - elapsed))
    }
    lastNovelFetchAt = Date.now()
    const res = await novelSearchPage(kw, 1, novelSort.value)
    novelHits.value = res.hits
    novelPageNo.value = res.page
    novelTotalPages.value = res.totalPages
    if (!res.hits.length) novelError.value = '没有匹配的系列'
  } catch (e) {
    novelError.value = errorMessage(e) || '搜索失败'
  } finally {
    novelSearching.value = false
  }
}

function setNovelSort(key: NovelSort) {
  novelSort.value = key
  // 已有搜索词时改排序即重搜(排序变化影响结果序)
  if (novelSearchedKw.value && novelHits.value.length) {
    sharedKeyword.value = novelSearchedKw.value
    void handleNovelSearch()
  }
}

async function loadMoreNovels() {
  if (novelLoadingMore.value || novelPageNo.value >= novelTotalPages.value) return
  novelLoadingMore.value = true
  novelError.value = ''
  try {
    const elapsed = Date.now() - lastNovelFetchAt
    if (elapsed < NOVEL_MIN_INTERVAL) {
      await new Promise((resolve) => setTimeout(resolve, NOVEL_MIN_INTERVAL - elapsed))
    }
    lastNovelFetchAt = Date.now()
    const res = await novelSearchPage(novelSearchedKw.value, novelPageNo.value + 1, novelSort.value)
    const seen = new Set(novelHits.value.map((h) => h.id))
    novelHits.value.push(...res.hits.filter((h) => !seen.has(h.id)))
    novelPageNo.value = res.page
    novelTotalPages.value = res.totalPages
  } catch (e) {
    novelError.value = errorMessage(e) || '加载更多失败'
  } finally {
    novelLoadingMore.value = false
  }
}

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
    await nextTick()
    setupObserver()
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
        <p v-if="videoError" class="scope-error">{{ videoError }}</p>
        <div v-if="videoSearching && !videoItems.length" class="scope-loading">
          <LoadingSpinner message="正在搜索视频..." />
        </div>
        <VideoCardGrid v-if="videoItems.length" :items="videoItems" class="scope-video-grid" />
        <div v-if="videoHasNext && videoItems.length" class="scroll-sentinel" />
        <div v-if="videoSearching && videoItems.length" class="scope-loading">
          <LoadingSpinner message="正在加载更多..." />
        </div>
      </template>

      <template v-else-if="scope === 'novel'">
        <SearchInput
          v-model="sharedKeyword"
          history-key="niacg-novel-search-history"
          placeholder="搜索小说(书名/作者)…"
          :loading="novelSearching || novelLoadingMore"
          @search="handleNovelSearch"
        />
        <div class="sort-row" role="group" aria-label="排序方式">
          <span class="sort-label">排序</span>
          <div class="sort-chips">
            <button
              v-for="s in NOVEL_SORTS"
              :key="s.key"
              type="button"
              class="sort-chip"
              :class="{ active: novelSort === s.key }"
              @click="setNovelSort(s.key)"
            >
              {{ s.label }}
            </button>
          </div>
        </div>
        <p v-if="novelError" class="scope-error">{{ novelError }}</p>
        <div v-if="novelSearching" class="scope-loading">
          <LoadingSpinner message="正在搜索小说(经小说源连接,首次可能需要十几秒)..." />
        </div>
        <div v-if="novelHits.length" class="scope-rows">
          <button
            v-for="h in novelHits"
            :key="h.id"
            type="button"
            class="scope-row"
            @click="router.push(`/novels/series/${h.id}`)"
          >
            <svg class="scope-row-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
              <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
              <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
            </svg>
            <span class="scope-row-main">
              <span class="scope-row-title">{{ h.title }}</span>
              <span v-for="tag in (h.tags ?? []).slice(0, 2)" :key="tag" class="scope-row-badge">{{ tag }}</span>
            </span>
            <span class="scope-row-sub">{{ h.author || '未知作者' }}</span>
          </button>
        </div>
        <div v-if="novelPageNo < novelTotalPages && novelHits.length" class="scope-more-row">
          <button type="button" class="scope-more-btn" :disabled="novelLoadingMore" @click="loadMoreNovels">
            {{ novelLoadingMore ? `加载中…(站点限速约 3 秒/次)` : `加载更多(第 ${novelPageNo + 1}/${novelTotalPages} 页)` }}
          </button>
        </div>
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

          <ComicGrid v-if="visibleItems.length > 0" :items="visibleItems" :viewed-ids="viewedIds" />

          <div v-if="hasMoreLocal || hasNextPage" class="scroll-sentinel" />

          <div v-if="loadingMore" class="loading-more">
            <LoadingSpinner message="加载更多..." />
          </div>

          <div v-if="!hasMoreLocal && !hasNextPage && processedItems.length > 0" class="no-more">
            <span>已加载全部 {{ processedItems.length }} 条结果</span>
          </div>

          <EmptyState
            v-if="processedItems.length === 0 && !loading"
            title="未找到相关内容"
            message="请尝试更换关键词或调整搜索范围"
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

.scope-loading {
  padding: var(--spacing-lg) 0;
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

.scroll-sentinel {
  height: 1px;
}

.loading-more {
  padding: var(--spacing-lg) 0;
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

.no-more {
  text-align: center;
  padding: var(--spacing-lg) 0;
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
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

.scope-search {
  display: flex;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-sm);
}

.scope-search-wrap {
  flex: 1;
  min-width: 0;
  position: relative;
}

.scope-input {
  width: 100%;
  box-sizing: border-box;
  padding: 8px 14px;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: var(--color-card-bg);
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
}

.scope-btn-submit {
  padding: 8px 18px;
  border: none;
  border-radius: var(--radius-pill, 999px);
  background: var(--color-primary);
  color: #fff;
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.scope-btn-submit:disabled {
  opacity: 0.5;
}

.scope-error {
  font-size: var(--font-size-caption);
  color: var(--color-error);
  margin: 0 0 var(--spacing-sm);
}

/* 1920 横屏行长可读性:全宽背景下结果列限宽居中 */
.scope-rows {
  max-width: 1100px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: var(--spacing-xs);
}

.scope-row {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 10px var(--spacing-md);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  background: var(--color-card-bg);
  cursor: pointer;
  text-align: left;
  transition: border-color 0.15s;
}

.scope-row:hover {
  border-color: var(--color-primary);
}

.scope-row-icon {
  flex-shrink: 0;
  width: 18px;
  height: 18px;
  color: var(--color-text-muted);
  opacity: 0.5;
}

.scope-row-main {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  min-width: 0;
}

.scope-row-title {
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.scope-row-badge {
  flex-shrink: 0;
  padding: 1px 6px;
  border-radius: 4px;
  font-size: 10px;
  background: var(--color-divider-soft);
  color: var(--color-text-secondary);
}

.scope-row-sub {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.scope-more-row {
  display: flex;
  justify-content: center;
  max-width: 1100px;
  margin: 0 auto;
  width: 100%;
}

.scope-more-btn {
  padding: 8px var(--spacing-lg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-pill, 999px);
  background: var(--color-card-bg);
  color: var(--color-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.scope-more-btn:disabled {
  opacity: 0.55;
  cursor: default;
}

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
