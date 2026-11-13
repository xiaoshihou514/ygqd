<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { ref, computed, watch, onMounted, onUnmounted, onActivated, onDeactivated, nextTick } from 'vue'
import { useRoute, useRouter, onBeforeRouteLeave } from 'vue-router'
import { searchComics } from '@/services/api'
import { useVideoSearchScope, VIDEO_SORTS } from '@/composables/useVideoSearchScope'
import type { ComicItem, SearchParams, SearchResult } from '@/types'
import { interleaveSources } from '@/utils/merge'
import { parseLikes } from '@/utils/likes'
import { useTagBlacklist } from '@/composables/useTagBlacklist'
import { useSearchHistory } from '@/composables/useSearchHistory'
import { useComicViewedIds } from '@/composables/useComicViewedIds'
import type { SourceId } from '@/config/sources'
import { parseSearchScope } from '@/config/sources'
import SearchForm from '@/components/SearchForm.vue'
import SearchInput from '@/components/SearchInput.vue'
import MediaCollection from '@/components/MediaCollection.vue'
import { fromComic } from '@/utils/mediaItem'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

defineOptions({ name: 'SearchPage' })

const PAGE_SIZE = 20
const MAX_PAGES = 50
/** 「星标降序」需拉全部页:每批并发请求的页数 */
const FETCH_ALL_BATCH = 5
const SEARCH_SOURCES = ['niacg', 'bika'] as const
const DEFAULT_SHOW = 'title,text,keyboard,ftitle'
/** 与后端 niacg 搜索限速闸一致(src-tauri/src/lib.rs NIACG_SEARCH_INTERVAL):两次搜索至少间隔 5.2 秒 */
const NIACG_SEARCH_INTERVAL_MS = 5200
const NIACG_WAIT_HINT = 'ACG 源两次搜索需间隔约 5 秒'

const route = useRoute()
const router = useRouter()

const items = ref<ComicItem[]>([])
const viewedIds = useComicViewedIds(items)
const loading = ref(false)
const loadingMore = ref(false)
const error = ref('')
/** 翻页失败(有结果时):走 MediaCollection 错误条 + 重试,不再静默吞掉 */
const loadMoreError = ref('')
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

/**
 * 搜索代次:每次发起新搜索(或黑名单变更重搜)自增。迟到的旧请求结果
 * 对比代次后丢弃,不会覆盖新结果;「星标降序」的全量翻页也据此提前终止,
 * 避免旧搜索继续占用 niacg 限速闸、拖慢新搜索。
 */
let searchGen = 0

interface MergedSearchOutcome {
  items: ComicItem[]
  hasNext: boolean
  totalPages: number
  errors: string[]
}

function sourcesFor(params: SearchParams): SourceId[] {
  return params.source === 'all' ? [...SEARCH_SOURCES] : [params.source]
}

function involvesNiacg(params: SearchParams | null): boolean {
  return !!params && sourcesFor(params).includes('niacg')
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

function fetchAllProgressText(done: number, total: number, params: SearchParams): string {
  const base = `正在获取全部结果（${done}/${total} 页）`
  if (!involvesNiacg(params)) return `${base}...`
  // 后端闸把 niacg 请求串行化,剩余耗时 ≈ 剩余页数 × 5.2 秒
  const seconds = Math.ceil(((total - done) * NIACG_SEARCH_INTERVAL_MS) / 1000)
  return `${base}，${NIACG_WAIT_HINT}，预计还需约 ${seconds} 秒`
}

/** 「星标降序」需全量排序:拉取剩余页;代次过期时提前返回 null */
async function fetchAllPages(
  params: SearchParams,
  firstPageItems: ComicItem[],
  totalPages: number,
  gen: number,
): Promise<ComicItem[] | null> {
  const pageCount = Math.min(totalPages, MAX_PAGES)
  const all = [...firstPageItems]
  let done = 1
  fetchingProgress.value = fetchAllProgressText(done, pageCount, params)

  try {
    for (let i = 1; i < pageCount; i += FETCH_ALL_BATCH) {
      if (gen !== searchGen) return null
      const batch: Promise<ComicItem[]>[] = []
      for (let j = i; j < Math.min(i + FETCH_ALL_BATCH, pageCount); j++) {
        batch.push(
          runSearch(params, j).then((r) => {
            done += 1
            if (gen === searchGen) fetchingProgress.value = fetchAllProgressText(done, pageCount, params)
            return r.items
          }),
        )
      }
      const results = await Promise.all(batch)
      all.push(...results.flat())
    }
  } finally {
    if (gen === searchGen) fetchingProgress.value = ''
  }
  return gen === searchGen ? all : null
}

const processedItems = computed(() => {
  let result = items.value
  if (minLikes.value > 0) {
    result = result.filter((item) => parseLikes(item.likes) >= minLikes.value)
  }
  if (sortOrder.value === 'likes_desc') {
    result = [...result].sort((a, b) => parseLikes(b.likes) - parseLikes(a.likes))
  }
  return filterItems(result)
})

const visibleItems = computed(() => processedItems.value.slice(0, visibleCount.value))

const hasMoreLocal = computed(() => visibleCount.value < processedItems.value.length)
const comicMediaItems = computed(() =>
  visibleItems.value.map((c) => fromComic(c, { viewed: viewedIds.value.has(`${c.source}:${c.id}`) })),
)

const searchingLabel = computed(() =>
  involvesNiacg(lastParams.value) ? `正在搜索…（${NIACG_WAIT_HINT}）` : '正在搜索...',
)
const loadingMoreLabel = computed(() =>
  involvesNiacg(lastParams.value) ? `正在加载更多…（${NIACG_WAIT_HINT}）` : '正在加载更多...',
)

async function loadMore() {
  if (loadingMore.value || loading.value) return
  if (hasMoreLocal.value) {
    visibleCount.value = Math.min(visibleCount.value + PAGE_SIZE, processedItems.value.length)
    return
  }
  const params = lastParams.value
  if (!hasNextPage.value || !params) return
  const gen = searchGen
  loadingMore.value = true
  loadMoreError.value = ''
  const nextPage = currentPage.value + 1
  try {
    const outcome = await runSearch(params, nextPage)
    if (gen !== searchGen) return
    items.value = [...items.value, ...outcome.items]
    currentPage.value = nextPage
    hasNextPage.value = outcome.hasNext
    if (outcome.errors.length > 0) {
      error.value = outcome.errors[0]!
    }
  } catch (e) {
    if (gen === searchGen) loadMoreError.value = errorMessage(e) || '加载更多失败'
  } finally {
    if (gen === searchGen) loadingMore.value = false
  }
}

watch([minLikes, sortOrder], () => {
  visibleCount.value = PAGE_SIZE
})

/** 首页结果写回(新搜索与黑名单重搜共用);代次过期返回 false */
async function applyFirstPage(
  params: SearchParams,
  order: string,
  gen: number,
  cacheBuster?: string,
): Promise<boolean> {
  const outcome = await runSearch(params, 0, cacheBuster)
  if (gen !== searchGen) return false
  if (order === 'likes_desc') {
    const all = await fetchAllPages(params, outcome.items, outcome.totalPages, gen)
    if (all === null) return false
    items.value = all
    hasNextPage.value = false
  } else {
    items.value = outcome.items
    hasNextPage.value = outcome.hasNext
  }
  if (outcome.errors.length > 0) {
    error.value = outcome.errors[0]!
  }
  return true
}

function resetPaging() {
  // 新一代搜索接管:旧代次的 finally 不再复位这些标志,这里统一复位
  loading.value = false
  error.value = ''
  loadMoreError.value = ''
  currentPage.value = 0
  hasNextPage.value = false
  visibleCount.value = PAGE_SIZE
  loadingMore.value = false
}

let prevBlacklistVersion = blacklistVersion.value
watch(blacklistVersion, (newVer) => {
  if (newVer !== prevBlacklistVersion && hasSearched.value && lastParams.value) {
    prevBlacklistVersion = newVer
    void reSearchWithCacheBuster()
  }
})

async function reSearchWithCacheBuster() {
  const params = lastParams.value
  if (!params) return
  const gen = ++searchGen
  resetPaging()
  loadingMore.value = true
  try {
    await applyFirstPage(params, sortOrder.value, gen, String(Date.now()))
  } catch (e) {
    if (gen !== searchGen) return
    error.value = errorMessage(e)
    items.value = []
  } finally {
    if (gen === searchGen) loadingMore.value = false
  }
}

// ============ 会话状态(刷新/重建后恢复) ============
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
      classid: String(lastParams.value?.classid ?? 9),
      show: lastParams.value?.show || (route.query.show as string) || DEFAULT_SHOW,
      source: lastParams.value?.source || (route.query.source as string) || 'all',
      scrollY: scopeScroll.comic,
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
    scopeScroll.comic = typeof state.scrollY === 'number' ? state.scrollY : 0
    const savedClassid = state.classid as string | undefined
    const savedShow = state.show as string | undefined
    const savedSource = parseSearchScope((state.source as string | undefined) || 'all')
    if (lastKeyword.value) {
      lastParams.value = {
        source: savedSource,
        keyword: lastKeyword.value,
        classid: savedClassid ? Number(savedClassid) : 9,
        show: savedShow || DEFAULT_SHOW,
        tempid: '1',
      }
    }
    return true
  } catch {
    return false
  }
}

function clearSavedState() {
  try {
    sessionStorage.removeItem(SESSION_KEY)
  } catch {
    // ignore
  }
}

// ============ 统一搜索:范围分段(漫画=现有流程;视频=并入) ============
type SearchScope2 = 'comic' | 'video'
const SCOPES: Array<{ key: SearchScope2; label: string }> = [
  { key: 'comic', label: '漫画' },
  { key: 'video', label: '视频' },
]
const scope = ref<SearchScope2>('comic')
const scopeIndex = computed(() => Math.max(0, SCOPES.findIndex((s) => s.key === scope.value)))
/** 各范围的滚动位置分别记忆 */
const scopeScroll: Record<SearchScope2, number> = { comic: 0, video: 0 }

function rememberScroll() {
  scopeScroll[scope.value] = window.scrollY
}

function restoreScroll() {
  void nextTick(() => window.scrollTo(0, scopeScroll[scope.value]))
}

function switchScope(key: SearchScope2) {
  if (key === scope.value) return
  rememberScroll()
  scope.value = key
  restoreScroll()
}

// 范围共用一个关键词:在任一范围搜过的词,切到其它范围时输入框预填同一个词
const sharedKeyword = ref((route.query.keyword as string | undefined) ?? '')
// 重构笔 4c:视频范围状态机迁移 useVideoSearchScope(哨兵重绑由 MediaCollection 负责)
const {
  videoSearchedKw, videoItems, videoHasNext, videoSearching, videoError, videoMediaItems,
  videoSort, setVideoSort,
  handleVideoSearch, loadMoreVideos,
} = useVideoSearchScope(sharedKeyword, () => {})

async function handleSearch(params: SearchParams, likesFilter: number, order: string) {
  const gen = ++searchGen
  addHistory(params.keyword)
  sharedKeyword.value = params.keyword
  resetPaging()
  loading.value = true
  hasSearched.value = true
  lastKeyword.value = params.keyword
  minLikes.value = likesFilter
  sortOrder.value = order
  lastParams.value = params
  prevBlacklistVersion = blacklistVersion.value
  scopeScroll.comic = 0
  try {
    await applyFirstPage(params, order, gen)
  } catch (e) {
    if (gen !== searchGen) return
    error.value = errorMessage(e)
    items.value = []
  } finally {
    if (gen === searchGen) loading.value = false
  }
}

/** 最近一次按路由参数发起的搜索(keyword/show/source),用于判断回到本页时是否需要重搜 */
let consumedQuerySig = ''
function routeQuerySig(): string {
  const { keyword, show, source } = route.query
  return [keyword, show, source].map((v) => (typeof v === 'string' ? v : '')).join('\u0000')
}

function searchFromQuery() {
  const keyword = route.query.keyword as string | undefined
  if (!keyword) return
  consumedQuerySig = routeQuerySig()
  // 带关键词进入(详情页标签/作者、关注页)均为漫画搜索:范围切回漫画再出结果
  scope.value = 'comic'
  const show = (route.query.show as string) || DEFAULT_SHOW
  const source = parseSearchScope((route.query.source as string) || 'all')
  void handleSearch({ source, keyword, classid: 9, show, tempid: '1' }, 0, 'default')
}

// 离开路由前记下滚动位置:此时本页仍在 DOM 中,window.scrollY 准确
onBeforeRouteLeave(() => {
  rememberScroll()
})

onMounted(() => {
  const keyword = route.query.keyword as string | undefined
  const restored = restoreSearchState()
  if (restored && (!keyword || lastKeyword.value === keyword)) {
    consumedQuerySig = routeQuerySig()
    restoreScroll()
    return
  }
  if (keyword) clearSavedState()
  searchFromQuery()
})

onActivated(() => {
  const currentKeyword = route.query.keyword as string | undefined
  if (currentKeyword && (routeQuerySig() !== consumedQuerySig || currentKeyword !== lastKeyword.value)) {
    clearSavedState()
    searchFromQuery()
    return
  }
  restoreScroll()
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
  try {
    sessionStorage.setItem('split_items', JSON.stringify(processedItems.value))
  } catch {
    // 结果过多超出存储配额时仍进入分屏页(其自行处理空数据)
  }
  router.push({ name: 'split' })
}
</script>

<template>
  <div class="search-page">
    <div class="container">
      <div
        class="seg-control"
        role="tablist"
        aria-label="搜索范围"
        :style="{ '--seg-count': SCOPES.length, '--seg-index': scopeIndex }"
      >
        <span class="seg-thumb" aria-hidden="true" />
        <button
          v-for="s in SCOPES"
          :key="s.key"
          type="button"
          class="seg-btn"
          :class="{ active: scope === s.key }"
          role="tab"
          :aria-selected="scope === s.key"
          @click="switchScope(s.key)"
        >
          {{ s.label }}
        </button>
      </div>

      <template v-if="scope === 'comic'">
        <SearchForm @search="handleSearch" />

        <div v-if="!hasSearched" class="search-intro">
          <EmptyState
            title="搜索漫画资源"
            message="输入关键词，选择分类和搜索范围开始搜索"
          />
        </div>

        <template v-else>
          <div v-if="loading" class="search-loading">
            <LoadingSpinner :message="fetchingProgress || searchingLabel" />
          </div>

          <div v-else-if="error && items.length === 0" class="search-error">
            <EmptyState title="搜索失败" :message="error" />
          </div>

          <div v-else class="search-results">
            <div v-if="error && items.length > 0" class="search-error-bar" role="status">
              <span>{{ error }}</span>
            </div>
            <div class="search-result-header">
              <h2 class="result-title">{{ resultTitle }}</h2>
              <button
                v-if="processedItems.length > 0"
                type="button"
                class="split-btn"
                aria-label="分屏浏览"
                title="分屏浏览"
                @click="goToSplit"
              >
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                  <path d="M4 6h16M4 12h16M4 18h16" />
                </svg>
              </button>
            </div>
            <p v-if="fetchingProgress" class="search-progress">{{ fetchingProgress }}</p>

            <MediaCollection
              :items="comicMediaItems"
              density="comic"
              :loading-more="loadingMore"
              :loading-more-label="loadingMoreLabel"
              :has-more="hasMoreLocal || hasNextPage"
              :error="loadMoreError"
              :end-text="`已加载全部 ${processedItems.length} 条结果`"
              empty-title="未找到相关内容"
              empty-message="请尝试更换关键词或调整搜索范围"
              @load-more="loadMore"
              @retry="loadMore"
            />
          </div>
        </template>
      </template>

      <template v-else>
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
              :aria-pressed="videoSort === s.key"
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
        <div v-else class="search-intro">
          <EmptyState title="搜索视频" message="输入番号、标题或厂商名开始搜索" />
        </div>
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

/* ---- 范围分段控件(与首页内容分段同一口径:滑块用 transform 平移) ---- */
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

/* 按下:滑块不随文字缩放,改为文字轻微变淡 */
.seg-btn:active {
  transform: none;
  opacity: 0.6;
}

.seg-btn:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: -2px;
}

/* ---- 结果区 ---- */
.search-intro,
.search-loading,
.search-error {
  padding-top: var(--spacing-xl);
}

.search-results {
  animation: results-in var(--duration-slow) var(--ease-out);
}

@keyframes results-in {
  from {
    opacity: 0;
  }
}

.scope-video-grid {
  margin-top: var(--spacing-md);
}

.search-result-header {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-md);
  padding-bottom: var(--spacing-sm);
  border-bottom: 1px solid var(--color-hairline);
}

.result-title {
  flex: 1;
  min-width: 0;
  font-size: var(--font-size-tagline);
  font-weight: var(--font-weight-bold);
  line-height: var(--line-height-heading);
  color: var(--color-text-primary);
  letter-spacing: -0.231px;
  overflow-wrap: anywhere;
}

.split-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  min-width: 44px;
  min-height: 36px;
  padding: 0 var(--spacing-sm);
  color: var(--color-primary);
  background: transparent;
  border: 1px solid var(--color-primary);
  border-radius: var(--radius-pill);
  transition: background-color var(--duration-base) var(--ease-standard),
    color var(--duration-base) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

.split-btn:active {
  transform: scale(var(--press-scale));
}

@media (hover: hover) {
  .split-btn:hover {
    background: var(--color-primary);
    color: var(--color-on-primary);
  }
}

.search-progress {
  margin-bottom: var(--spacing-md);
  font-size: var(--font-size-fine-print);
  color: var(--color-text-muted);
}

.search-error-bar {
  padding: var(--spacing-sm);
  margin-bottom: var(--spacing-md);
  font-size: var(--font-size-caption);
  text-align: center;
  color: var(--color-text-secondary);
  background: var(--color-canvas);
  border-radius: var(--radius-sm);
}

/* ---- 视频排序面板(与 SearchForm 的选项行同一口径) ---- */
.sort-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  margin: var(--spacing-xs) 0 var(--spacing-sm);
}

.sort-label {
  flex-shrink: 0;
  min-width: 36px;
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-secondary);
}

.sort-chips {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.sort-chip {
  position: relative;
  min-height: 32px;
  padding: 0 14px;
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  background: var(--color-canvas);
  color: var(--color-text-secondary);
  font-size: var(--font-size-fine-print);
  transition: background-color var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard),
    color var(--duration-fast) var(--ease-standard),
    transform var(--duration-fast) var(--ease-standard);
}

/* 视觉 32px,触控热区上下各扩 6px 达 44px */
.sort-chip::after {
  content: '';
  position: absolute;
  inset: -6px 0;
}

.sort-chip:active {
  transform: scale(var(--press-scale));
}

@media (hover: hover) {
  .sort-chip:not(.active):hover {
    border-color: var(--color-primary);
    color: var(--color-primary);
  }
}

.sort-chip.active {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--color-on-primary);
}
</style>
