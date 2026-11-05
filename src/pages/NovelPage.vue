<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { gatewayOpen, novelAdd, novelRemove, novelShelf, novelProgress, novelSearchPage, novelLatest } from '@/services/api'
import type { NovelHit, NovelSeries } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'
import NovelSeriesCard from '@/components/NovelSeriesCard.vue'
import { createSearchHistory } from '@/composables/useSearchHistory'

const router = useRouter()
const shelf = ref<NovelSeries[]>([])
const loading = ref(true)
const adding = ref(false)
const addError = ref('')
const addHint = ref('')
const input = ref('')
const progressMap = ref<Record<string, [number, number]>>({})
const hits = ref<NovelHit[]>([])
const searchKeyword = ref('')
const searching = ref(false)
const searchError = ref('')
const addingHitId = ref('')
// 搜索分页:站点每页 30 条(热门词数百页);反爬闸按频率触发,
// 相邻抓取保底 3 秒间隔(不足由加载更多内部等待补齐)
const searchPage = ref(1)
const searchTotalPages = ref(1)
const loadingMoreHits = ref(false)
const activeKeyword = ref('')
const hasMoreHits = computed(() => searchPage.value < searchTotalPages.value)

// 页内分段:书架(默认)/发现(搜索+最新)
const segment = ref<'shelf' | 'discover'>('shelf')
const discoverLoaded = ref(false)
// 结果区来源:最近一次是「最新」流还是「搜索」流,加载更多据此分发
const discoverMode = ref<'latest' | 'search'>('latest')
const loadingLatest = ref(false)
const latestPage = ref(0)
// 发现流无总页数:hits 非空即继续,空页终止
const hasMoreLatest = ref(false)

// 搜索历史:独立 key(书名/作者与番号语义不同)
const novelHistory = createSearchHistory('niacg-novel-search-history')
const showNovelHistory = ref(false)
const selectedNovelHistoryIndex = ref(-1)
const searchInput = ref<HTMLInputElement | null>(null)
const matchedNovelHistory = computed(() => novelHistory.suggestions(searchKeyword.value))

// hlib 统一节流:搜索与发现流共用同一站点反爬闸,相邻抓取保底 3 秒
let lastHlibFetchAt = 0
const HLIB_MIN_INTERVAL = 3000

async function guardHlibInterval() {
  const elapsed = Date.now() - lastHlibFetchAt
  if (elapsed < HLIB_MIN_INTERVAL) {
    await new Promise((resolve) => setTimeout(resolve, HLIB_MIN_INTERVAL - elapsed))
  }
  lastHlibFetchAt = Date.now()
}

const hasMoreNext = computed(() =>
  discoverMode.value === 'search' ? hasMoreHits.value : hasMoreLatest.value,
)
const moreLabel = computed(() =>
  discoverMode.value === 'search'
    ? `加载更多(第 ${searchPage.value + 1}/${searchTotalPages.value} 页)`
    : '加载更多',
)

async function refresh() {
  loading.value = true
  try {
    shelf.value = await novelShelf()
    for (const s of shelf.value) {
      try {
        const p = await novelProgress(s.id)
        if (p) progressMap.value[s.id] = p
      } catch { /* 进度读取失败忽略 */ }
    }
  } finally {
    loading.value = false
  }
}

async function openHlib() {
  try {
    await gatewayOpen('hlib')
  } catch { /* 网关创建失败忽略,用户可重试 */ }
}

function switchDiscover() {
  segment.value = 'discover'
  // 首次切到发现分段自动拉最新(hits 非空即继续的分页模式)
  if (!discoverLoaded.value) {
    discoverLoaded.value = true
    void loadLatestFirst()
  }
}

async function loadLatestFirst() {
  discoverMode.value = 'latest'
  searchError.value = ''
  loadingLatest.value = true
  try {
    await guardHlibInterval()
    const res = await novelLatest(1)
    hits.value = res
    latestPage.value = 1
    hasMoreLatest.value = res.length > 0
  } catch (e) {
    searchError.value = errorMessage(e) || '加载失败'
  } finally {
    loadingLatest.value = false
  }
}

function appendHits(list: NovelHit[]) {
  const seen = new Set(hits.value.map((h) => h.id))
  hits.value.push(...list.filter((h) => !seen.has(h.id)))
}

async function handleSearch() {
  const kw = searchKeyword.value.trim()
  if (!kw || searching.value) return
  searching.value = true
  searchError.value = ''
  try {
    await guardHlibInterval()
    const res = await novelSearchPage(kw, 1)
    hits.value = res.hits
    searchPage.value = res.page
    searchTotalPages.value = res.totalPages
    activeKeyword.value = kw
    discoverMode.value = 'search'
    novelHistory.add(kw)
    if (!hits.value.length) searchError.value = '没有匹配的系列'
  } catch (e) {
    searchError.value = errorMessage(e) || '搜索失败'
  } finally {
    searching.value = false
  }
}

/** 加载更多:按结果来源分发(搜索分页/最新流分页),统一 hlib 节流 */
async function loadMore() {
  if (loadingMoreHits.value || !hasMoreNext.value) return
  if (discoverMode.value === 'search' && !activeKeyword.value) return
  loadingMoreHits.value = true
  searchError.value = ''
  try {
    await guardHlibInterval()
    if (discoverMode.value === 'search') {
      const res = await novelSearchPage(activeKeyword.value, searchPage.value + 1)
      appendHits(res.hits)
      searchPage.value = res.page
      searchTotalPages.value = res.totalPages
    } else {
      const res = await novelLatest(latestPage.value + 1)
      appendHits(res)
      latestPage.value += 1
      hasMoreLatest.value = res.length > 0
    }
  } catch (e) {
    searchError.value = errorMessage(e) || '加载更多失败'
  } finally {
    loadingMoreHits.value = false
  }
}

// 搜索历史交互(镜像 SearchForm/VideoBrowser 交互)
function selectNovelHistory(kw: string) {
  searchKeyword.value = kw
  showNovelHistory.value = false
  selectedNovelHistoryIndex.value = -1
  void handleSearch()
}

function removeNovelHistory(kw: string) {
  novelHistory.remove(kw)
}

function clearNovelHistory() {
  novelHistory.clear()
  showNovelHistory.value = false
  selectedNovelHistoryIndex.value = -1
}

function onSearchFocus() {
  if (novelHistory.history.value.length > 0) showNovelHistory.value = true
}

function onSearchBlur() {
  setTimeout(() => {
    showNovelHistory.value = false
    selectedNovelHistoryIndex.value = -1
  }, 150)
}

function onSearchKeydown(e: KeyboardEvent) {
  const list = matchedNovelHistory.value
  if (list.length === 0) return
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    showNovelHistory.value = true
    selectedNovelHistoryIndex.value = Math.min(selectedNovelHistoryIndex.value + 1, list.length - 1)
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    selectedNovelHistoryIndex.value = Math.max(selectedNovelHistoryIndex.value - 1, -1)
  } else if (e.key === 'Enter' && selectedNovelHistoryIndex.value >= 0) {
    e.preventDefault()
    selectNovelHistory(list[selectedNovelHistoryIndex.value]!)
  } else if (e.key === 'Escape') {
    showNovelHistory.value = false
    selectedNovelHistoryIndex.value = -1
  }
}

/** 从发现/搜索结果一键入库 */
async function addHit(hit: NovelHit) {
  if (addingHitId.value) return
  addingHitId.value = hit.id
  try {
    await novelAdd(hit.id)
    await refresh()
  } catch (e) {
    searchError.value = errorMessage(e) || '添加失败'
  } finally {
    addingHitId.value = ''
  }
}

async function handleAdd() {
  const value = input.value.trim()
  if (!value || adding.value) return
  adding.value = true
  addError.value = ''
  addHint.value = '正在通过网关抓取系列目录…若首次使用会弹出窗口,请完成登录与安全验证'
  try {
    await novelAdd(value)
    input.value = ''
    addHint.value = '已加入书架'
    await refresh()
  } catch (e) {
    addError.value = errorMessage(e) || '添加失败'
  } finally {
    adding.value = false
    setTimeout(() => { addHint.value = '' }, 3000)
  }
}

async function handleRemove(id: string) {
  try {
    await novelRemove(id)
    shelf.value = shelf.value.filter((s) => s.id !== id)
  } catch { /* 忽略 */ }
}

function open(series: NovelSeries) {
  router.push(`/novels/${series.id}`)
}

onMounted(() => {
  void refresh()
})
</script>

<template>
  <div class="novel-page">
    <div class="container">
      <div class="page-heading">
        <h2 class="page-title">小说</h2>
        <button class="hlib-btn" @click="openHlib">登录 / 浏览 hlib</button>
      </div>

      <div class="segment-bar" role="tablist" aria-label="小说板块">
        <button
          type="button"
          class="segment-btn"
          :class="{ active: segment === 'shelf' }"
          role="tab"
          :aria-selected="segment === 'shelf'"
          @click="segment = 'shelf'"
        >
          书架
        </button>
        <button
          type="button"
          class="segment-btn"
          :class="{ active: segment === 'discover' }"
          role="tab"
          :aria-selected="segment === 'discover'"
          @click="switchDiscover"
        >
          发现
        </button>
      </div>

      <template v-if="segment === 'shelf'">
        <LoadingSpinner v-if="loading" message="加载书架..." />
        <EmptyState
          v-else-if="!shelf.length"
          title="书架还是空的"
          message="切到「发现」用搜索找小说或浏览最新作品,点「+ 书架」收藏;首次使用会弹出 hlib 窗口,完成一次登录即可"
        >
          <button class="hlib-btn" @click="openHlib">打开 hlib</button>
        </EmptyState>
        <div v-else class="shelf-grid">
          <div v-for="s in shelf" :key="s.id" class="shelf-cell">
            <NovelSeriesCard
              :series="s"
              :progress="progressMap[s.id] ?? null"
              @open="open(s)"
            />
            <button
              class="shelf-remove"
              title="移出书架"
              aria-label="移出书架"
              @click.stop="handleRemove(s.id)"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" fill="currentColor" />
              </svg>
            </button>
          </div>
        </div>
      </template>

      <template v-else>
        <form class="add-bar" @submit.prevent="handleSearch">
          <div class="search-wrap">
            <input
              ref="searchInput"
              v-model="searchKeyword"
              type="search"
              class="add-input"
              placeholder="站内搜索小说(书名/作者)"
              autocomplete="off"
              @focus="onSearchFocus"
              @blur="onSearchBlur"
              @keydown="onSearchKeydown"
            >
            <ul
              v-if="showNovelHistory && matchedNovelHistory.length > 0"
              class="history-dropdown"
            >
              <li
                v-for="(s, i) in matchedNovelHistory"
                :key="s"
                class="history-item"
                :class="{ highlighted: i === selectedNovelHistoryIndex }"
                @mousedown.prevent="selectNovelHistory(s)"
              >
                <svg class="history-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <polyline points="15 18 9 12 15 6" />
                </svg>
                <span class="history-text">{{ s }}</span>
                <button
                  type="button"
                  class="history-remove"
                  title="删除历史"
                  aria-label="删除历史"
                  @mousedown.stop.prevent="removeNovelHistory(s)"
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                  </svg>
                </button>
              </li>
              <li class="history-clear-row">
                <button type="button" class="history-clear" @mousedown.stop.prevent="clearNovelHistory()">
                  清空搜索历史
                </button>
              </li>
            </ul>
          </div>
          <button type="submit" class="add-btn" :disabled="searching">
            {{ searching ? '搜索中…' : '搜索' }}
          </button>
          <button type="button" class="add-btn ghost" :disabled="loadingLatest" @click="loadLatestFirst">
            {{ loadingLatest ? '加载中…' : '最新' }}
          </button>
        </form>
        <p v-if="searchError" class="add-error">{{ searchError }}</p>

        <div v-if="hits.length" class="hit-list">
          <div v-for="hit in hits" :key="hit.id" class="hit-row">
            <div class="hit-main">
              <span class="hit-title">{{ hit.title }}</span>
              <span
                v-if="hit.author || (hit.tags && hit.tags.length > 0)"
                class="hit-meta"
              >
                <template v-if="hit.author">{{ hit.author }}</template>
                <span
                  v-for="tag in (hit.tags ?? []).slice(0, 3)"
                  :key="tag"
                  class="hit-tag"
                >
                  {{ tag }}
                </span>
              </span>
            </div>
            <button class="hit-add" :disabled="addingHitId === hit.id" @click="addHit(hit)">
              {{ addingHitId === hit.id ? '添加中…' : '+ 书架' }}
            </button>
          </div>
        </div>

        <div v-if="hits.length && hasMoreNext" class="hit-more">
          <button
            class="hit-more-btn"
            :disabled="loadingMoreHits"
            :title="discoverMode === 'search' ? `继续搜索:${activeKeyword}` : '继续加载最新'"
            @click="loadMore"
          >
            {{
              loadingMoreHits
                ? '加载中…(站点限速约 3 秒/次)'
                : moreLabel
            }}
          </button>
        </div>
        <div v-else-if="hits.length && !hasMoreNext" class="hit-end">
          已展示全部
        </div>

        <form class="add-bar paste-bar" @submit.prevent="handleAdd">
          <input
            v-model="input"
            type="text"
            class="add-input"
            placeholder="高级:粘贴 hlib 系列链接(/s/xxx)或系列 ID"
            autocomplete="off"
          >
          <button type="submit" class="add-btn" :disabled="adding">
            {{ adding ? '添加中…' : '添加' }}
          </button>
        </form>
        <p v-if="addHint" class="add-hint">{{ addHint }}</p>
        <p v-if="addError" class="add-error">{{ addError }}</p>
      </template>
    </div>
  </div>
</template>

<style scoped>
.novel-page {
  padding: var(--spacing-md) 0 var(--spacing-xl);
}

.page-title {
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-md);
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

.search-wrap {
  flex: 1;
  min-width: 0;
  position: relative;
}

.history-dropdown {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  margin: 0;
  padding: 4px 0;
  background: var(--color-card-bg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  z-index: 100;
  overflow: hidden;
  list-style: none;
}

.history-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px var(--spacing-md);
  font-size: var(--font-size-body);
  color: var(--color-text-primary);
  cursor: pointer;
  transition: background 0.15s;
}

.history-item:hover,
.history-item.highlighted {
  background: var(--color-primary-focus);
}

.history-icon {
  flex-shrink: 0;
  color: var(--color-text-muted);
  opacity: 0.5;
}

.history-text {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.history-remove {
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s, background 0.15s;
}

.history-item:hover .history-remove {
  opacity: 0.7;
}

.history-remove:hover {
  opacity: 1;
  background: rgba(0, 0, 0, 0.08);
}

.history-clear-row {
  border-top: 1px solid var(--color-hairline);
  margin-top: 4px;
  padding: 6px var(--spacing-md);
}

.history-clear {
  width: 100%;
  padding: 4px 0;
  border: none;
  background: transparent;
  color: var(--color-text-muted);
  font-size: var(--font-size-fine-print);
  cursor: pointer;
  text-align: center;
  transition: color 0.15s;
}

.history-clear:hover {
  color: var(--color-text-primary);
}

.paste-bar {
  margin-top: var(--spacing-lg);
}

.page-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-md);
  margin-bottom: var(--spacing-md);
}

.hlib-btn {
  padding: 8px 16px;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  color: var(--color-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.add-bar {
  display: flex;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-sm);
}

.add-input {
  flex: 1;
  min-width: 0;
  padding: 8px 14px;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: var(--color-card-bg);
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
}

.add-btn {
  padding: 8px 18px;
  border: none;
  border-radius: var(--radius-pill, 999px);
  background: var(--color-primary);
  color: #fff;
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.add-btn:disabled {
  opacity: 0.5;
}

.add-btn.ghost {
  background: transparent;
  border: 1px solid var(--color-border, var(--color-hairline));
  color: var(--color-text-primary);
}

.hit-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: var(--spacing-lg);
  max-height: 42vh;
  overflow-y: auto;
}

.hit-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: 8px 12px;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  background: var(--color-card-bg);
}

.hit-main {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.hit-title {
  font-size: var(--font-size-caption);
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.hit-meta {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  gap: var(--spacing-xs);
  font-size: var(--font-size-fine-print, 12px);
  color: var(--color-text-secondary);
  overflow: hidden;
}

.hit-tag {
  padding: 1px 8px;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-pill, 999px);
  font-size: 11px;
  color: var(--color-text-secondary);
  background: var(--color-canvas);
  white-space: nowrap;
}

.hit-more {
  display: flex;
  justify-content: center;
  padding: var(--spacing-sm) 0;
}

.hit-more-btn {
  padding: 8px var(--spacing-lg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-pill, 999px);
  background: var(--color-card-bg);
  color: var(--color-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
  transition: border-color 0.15s;
}

.hit-more-btn:hover:not(:disabled) {
  border-color: var(--color-primary);
}

.hit-more-btn:disabled {
  opacity: 0.55;
  cursor: default;
}

.hit-end {
  text-align: center;
  padding: var(--spacing-sm) 0;
  font-size: var(--font-size-fine-print, 12px);
  color: var(--color-text-secondary);
}

.hit-add {
  border: 1px solid var(--color-primary);
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  color: var(--color-primary);
  font-size: 12px;
  padding: 4px 12px;
  cursor: pointer;
  flex-shrink: 0;
}

.hit-add:disabled {
  opacity: 0.5;
}

.add-hint {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
  margin: 0 0 var(--spacing-sm);
}

.add-error {
  font-size: var(--font-size-fine-print);
  color: var(--color-error);
  margin: 0 0 var(--spacing-sm);
}

.shelf-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: var(--spacing-md);
}

.shelf-cell {
  position: relative;
}

.shelf-remove {
  position: absolute;
  right: 6px;
  top: 6px;
  z-index: 2;
  border: none;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  cursor: pointer;
  padding: 6px;
  border-radius: 50%;
}

.shelf-remove:hover {
  color: var(--color-error);
}
</style>
