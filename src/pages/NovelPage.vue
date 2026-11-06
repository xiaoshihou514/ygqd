<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { novelAdd, novelSearchPage, novelLatest } from '@/services/api'
import type { NovelHit } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import { createSearchHistory } from '@/composables/useSearchHistory'

const router = useRouter()
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

// 结果区来源:最近一次是「最新」流还是「搜索」流,加载更多据此分发
const discoverMode = ref<'latest' | 'search'>('latest')
const loadingLatest = ref(false)
const latestPage = ref(0)
// 发现流无总页数:hits 非空即继续,空页终止
const hasMoreLatest = ref(false)

// 板块 tab:推荐(最新流,批次 1)/日榜/月榜(批次 2 数据端点,先上骨架)
type NovelTab = 'recommend' | 'daily' | 'monthly'
const novelTab = ref<NovelTab>('recommend')
const NOVEL_TABS: Array<{ key: NovelTab; label: string }> = [
  { key: 'recommend', label: '推荐' },
  { key: 'daily', label: '日榜' },
  { key: 'monthly', label: '月榜' },
]

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

async function loadLatestFirst() {
  discoverMode.value = 'latest'
  searchError.value = ''
  loadingLatest.value = true
  try {
    await guardHlibInterval()
    const res = await novelLatest(1)
    hits.value = res.hits
    latestPage.value = res.page
    hasMoreLatest.value = res.page < res.totalPages
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
      appendHits(res.hits)
      latestPage.value = res.page
      hasMoreLatest.value = res.page < res.totalPages
    }
  } catch (e) {
    const msg = errorMessage(e)
    // 空页后端抛「未解析到系列链接」:视为到达末尾,静默收终态不报错
    if (msg.includes('未解析到系列链接')) {
      hasMoreLatest.value = false
    } else {
      searchError.value = msg || '加载更多失败'
    }
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
  } catch (e) {
    searchError.value = errorMessage(e) || '添加失败'
  } finally {
    addingHitId.value = ''
  }
}

onMounted(() => {
  // 打开即内容流
  void loadLatestFirst()
})
</script>

<template>
  <div class="novel-page">
    <div class="container">
      <div class="page-heading">
        <h2 class="page-title">小说</h2>
      </div>

      <div class="segment-bar" role="tablist" aria-label="小说内容">
        <button
          v-for="t in NOVEL_TABS"
          :key="t.key"
          type="button"
          class="segment-btn"
          :class="{ active: novelTab === t.key }"
          role="tab"
          :aria-selected="novelTab === t.key"
          @click="novelTab = t.key"
        >
          {{ t.label }}
        </button>
      </div>

      <div v-if="novelTab !== 'recommend'" class="tab-placeholder">
        {{ novelTab === 'daily' ? '日榜即将上线' : '月榜即将上线' }}
      </div>

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

.tab-placeholder {
  padding: var(--spacing-xl) var(--spacing-md);
  text-align: center;
  font-size: var(--font-size-caption);
  color: var(--color-text-secondary);
  border: 1px dashed var(--color-hairline);
  border-radius: var(--radius-md);
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
  /* 输入在 search-wrap(非 flex 容器)内,须显式撑满:否则按 UA 默认
     宽度渲染,窄屏溢出与按钮重合 */
  width: 100%;
  box-sizing: border-box;
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


</style>
