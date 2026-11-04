<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { fetchVideoList, searchVideos, fetchVideoWatchedIds } from '@/services/api'
import type { VideoCard } from '@/types'
import { createSearchHistory } from '@/composables/useSearchHistory'
import ProxyImage from '@/components/ProxyImage.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const router = useRouter()
// 视频搜索历史:独立 key(番号/标题词与漫画作品名语义不同,不共用)
const videoHistory = createSearchHistory('niacg-video-search-history')
const showHistory = ref(false)
const selectedHistoryIndex = ref(-1)
const searchInput = ref<HTMLInputElement | null>(null)
const matchedHistory = computed(() => videoHistory.suggestions(keyword.value))
const items = ref<VideoCard[]>([])
const loading = ref(true)
const loadingMore = ref(false)
const error = ref('')
const keyword = ref('')
const activeKeyword = ref('')
const page = ref(1)
const hasNext = ref(true)
const watched = ref<Set<string>>(new Set())
let observer: IntersectionObserver | null = null
/** 常驻哨兵节点(v-show 显隐),不随条件分支销毁重建,observer 一次绑定终身有效 */
const sentinelEl = ref<HTMLElement | null>(null)

async function load(reset: boolean) {
  if (reset) {
    loading.value = true
    error.value = ''
    page.value = 1
  } else {
    // 追加加载拦截:并发锁 / 无下一页 / 首载中 / 无数据(首载失败或空结果,防止悄悄拉下一页)
    const blocked = loadingMore.value || !hasNext.value || loading.value || !items.value.length
    if (blocked) {
      console.info('[video-browser] 追加加载拦截', {
        loadingMore: loadingMore.value,
        hasNext: hasNext.value,
        loading: loading.value,
        items: items.value.length,
      })
      return
    }
    loadingMore.value = true
  }
  try {
    const result = activeKeyword.value
      ? await searchVideos(activeKeyword.value, page.value)
      : await fetchVideoList(page.value)
    if (reset) {
      items.value = result.items
    } else {
      const seen = new Set(items.value.map((v) => v.code))
      items.value.push(...result.items.filter((v) => !seen.has(v.code)))
    }
    hasNext.value = result.pagination.hasNext
    // 批量查询已看,卡片标灰角标
    try {
      const ids = await fetchVideoWatchedIds(items.value.map((v) => v.code))
      watched.value = new Set(ids)
    } catch { /* 查询失败不影响列表 */ }
    page.value += 1
  } catch (e) {
    error.value = (e as Error).message || '加载失败'
  } finally {
    loading.value = false
    loadingMore.value = false
  }
}

function submitSearch() {
  activeKeyword.value = keyword.value.trim().toLowerCase()
  showHistory.value = false
  selectedHistoryIndex.value = -1
  if (activeKeyword.value) videoHistory.add(activeKeyword.value)
  void load(true)
}

function selectHistory(keywordText: string) {
  keyword.value = keywordText
  showHistory.value = false
  selectedHistoryIndex.value = -1
  activeKeyword.value = keywordText
  videoHistory.add(keywordText)
  void load(true)
}

function removeHistory(keywordText: string) {
  videoHistory.remove(keywordText)
}

function clearAllHistory() {
  videoHistory.clear()
  showHistory.value = false
  selectedHistoryIndex.value = -1
}

function onInputFocus() {
  if (videoHistory.history.value.length > 0) {
    showHistory.value = true
  }
}

function onInputBlur() {
  // 延迟让下拉内 mousedown 先行(SearchForm 同款交互)
  setTimeout(() => {
    showHistory.value = false
    selectedHistoryIndex.value = -1
  }, 150)
}

function onInputKeydown(e: KeyboardEvent) {
  const list = matchedHistory.value
  if (list.length === 0) return
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    showHistory.value = true
    selectedHistoryIndex.value = Math.min(selectedHistoryIndex.value + 1, list.length - 1)
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    selectedHistoryIndex.value = Math.max(selectedHistoryIndex.value - 1, -1)
  } else if (e.key === 'Enter' && selectedHistoryIndex.value >= 0) {
    e.preventDefault()
    selectHistory(list[selectedHistoryIndex.value]!)
  } else if (e.key === 'Escape') {
    showHistory.value = false
    selectedHistoryIndex.value = -1
  }
}

function clearSearch() {
  keyword.value = ''
  activeKeyword.value = ''
  void load(true)
}

/** 徽标展示用基础番号:剥无码流出与中字后缀(先长后短,与后端 title
 *  剥离同序)。跳转 / 去重 / 已看记录仍用完整 code,不受展示层影响。 */
function basicCode(code: string): string {
  return code
    .replace('-uncensored-leak-chinese-subtitle', '')
    .replace('-uncensored-leak', '')
    .replace('-chinese-subtitle', '')
    .toUpperCase()
}

/** 哨兵常驻 DOM(v-show 控制显隐):display:none 的元素与视口不相交,
 *  隐藏期间不触发回调,恢复显示即自动恢复工作——节点不随条件分支
 *  销毁重建,observer 一次绑定终身有效(前两轮「搜索后翻页失效」的
 *  根因即哨兵节点重建导致观察失联,本结构从根上消除该模式)。 */
function bindObserver() {
  if (!sentinelEl.value) {
    console.info('[video-browser] 绑定失败:哨兵节点不存在')
    return
  }
  observer = new IntersectionObserver(
    (entries) => {
      console.info('[video-browser] 哨兵回调', {
        intersecting: entries[0]?.isIntersecting ?? false,
        page: page.value,
        hasNext: hasNext.value,
        loading: loading.value,
        loadingMore: loadingMore.value,
        items: items.value.length,
      })
      if (entries[0]?.isIntersecting) void load(false)
    },
    { rootMargin: '400px' },
  )
  observer.observe(sentinelEl.value)
  console.info('[video-browser] observer 已绑定哨兵')
}

onMounted(() => {
  bindObserver()
  void load(true)
})

onUnmounted(() => {
  observer?.disconnect()
})
</script>

<template>
  <div class="video-browser">
      <div class="video-heading">
        <form class="video-search" @submit.prevent="submitSearch">
          <div class="video-search-wrap">
            <input
              ref="searchInput"
              v-model="keyword"
              type="search"
              class="video-search-input"
              placeholder="输入番号或关键词,如 ssis-406"
              autocomplete="off"
              @focus="onInputFocus"
              @blur="onInputBlur"
              @keydown="onInputKeydown"
            >
            <ul v-if="showHistory && matchedHistory.length > 0" class="history-dropdown">
              <li
                v-for="(s, i) in matchedHistory"
                :key="s"
                class="history-item"
                :class="{ highlighted: i === selectedHistoryIndex }"
                @mousedown.prevent="selectHistory(s)"
              >
                <svg class="history-icon" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                  <polyline points="15 18 9 12 15 6" />
                </svg>
                <span class="history-text">{{ s }}</span>
                <button
                  type="button"
                  class="history-remove"
                  title="删除该条历史"
                  aria-label="删除该条历史"
                  @mousedown.stop.prevent="removeHistory(s)"
                >
                  <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
                    <line x1="18" y1="6" x2="6" y2="18" />
                    <line x1="6" y1="6" x2="18" y2="18" />
                  </svg>
                </button>
              </li>
              <li class="history-clear-row">
                <button type="button" class="history-clear" @mousedown.stop.prevent="clearAllHistory()">
                  清空搜索历史
                </button>
              </li>
            </ul>
          </div>
          <button type="submit" class="video-search-btn">搜索</button>
          <button v-if="activeKeyword" type="button" class="video-search-btn" @click="clearSearch">
            清除
          </button>
        </form>
      </div>

      <LoadingSpinner v-if="loading" message="加载中..." />
      <EmptyState
        v-else-if="error"
        :title="activeKeyword ? '搜索失败' : '加载失败'"
        :message="error"
      />
      <EmptyState
        v-else-if="!items.length"
        :title="activeKeyword ? '没有找到结果' : '暂无内容'"
        :message="activeKeyword ? `没有与「${activeKeyword}」匹配的视频,换个番号试试` : '稍后再来看看'"
      />
      <div v-else class="video-grid">
        <button
          v-for="card in items"
          :key="card.code"
          class="video-card"
          @click="router.push(`/videos/${card.code}`)"
        >
          <div class="video-thumb-wrap">
            <ProxyImage :src="card.thumbnail" source="video" class="video-thumb" loading="lazy" />
            <span v-if="card.views" class="video-views">▶ {{ card.views }}</span>
            <span v-if="watched.has(card.code)" class="video-watched">已看</span>
          </div>
          <div class="video-card-body">
            <span class="video-code-row">
              <span class="video-code">{{ basicCode(card.code) }}</span>
              <span v-if="card.code.includes('-uncensored-leak')" class="video-leak-badge">无码</span>
              <span v-if="card.chinese" class="video-cs-badge">中字</span>
            </span>
            <span class="video-card-title">{{ card.title || basicCode(card.code) }}</span>
          </div>
        </button>
      </div>

      <div v-if="loadingMore" class="loading-more">
        <LoadingSpinner message="正在加载更多..." />
      </div>
      <!-- 常驻哨兵:v-show 显隐(display:none 不与视口相交,隐藏期间不触发),
           节点不销毁重建,observer 绑定终身有效 -->
      <div v-show="!loading && !error && items.length > 0" ref="sentinelEl" class="scroll-sentinel" />
      <div v-if="!loading && !error && !hasNext && items.length" class="no-more">
        <span>没有更多了</span>
      </div>
  </div>
</template>

<style scoped>
.video-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-md);
  flex-wrap: wrap;
  margin-bottom: var(--spacing-md);
}

.video-search {
  display: flex;
  gap: var(--spacing-xs);
  flex: 1;
  max-width: 420px;
}

.video-search-wrap {
  flex: 1;
  min-width: 0;
  position: relative;
}

.video-search-input {
  width: 100%;
  padding: 8px 14px;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: var(--color-card-bg);
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
  box-sizing: border-box;
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

.video-search-btn {
  padding: 8px 16px;
  border: none;
  border-radius: var(--radius-pill, 999px);
  background: var(--color-primary);
  color: #fff;
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.video-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: var(--spacing-md);
}

.video-card {
  display: flex;
  flex-direction: column;
  padding: 0;
  border: none;
  background: var(--color-card-bg);
  border-radius: var(--radius-md);
  overflow: hidden;
  cursor: pointer;
  text-align: left;
}

.video-thumb-wrap {
  position: relative;
  aspect-ratio: 16 / 9;
  background: var(--color-divider-soft);
}

.video-thumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.video-watched {
  position: absolute;
  left: 6px;
  top: 6px;
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.65);
  color: #9be9a8;
  font-size: 11px;
}

.video-views {
  position: absolute;
  right: 6px;
  bottom: 6px;
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.65);
  color: #fff;
  font-size: 11px;
}

.video-card-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 10px;
}

.video-code-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.video-code {
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-primary);
}

.video-cs-badge {
  padding: 0 5px;
  border-radius: 4px;
  background: #e8590c;
  color: #fff;
  font-size: 10px;
  flex-shrink: 0;
}

.video-leak-badge {
  padding: 0 5px;
  border-radius: 4px;
  background: #6741d9;
  color: #fff;
  font-size: 10px;
  flex-shrink: 0;
}

.video-card-title {
  font-size: 12px;
  color: var(--color-text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.loading-more,
.no-more {
  display: flex;
  justify-content: center;
  padding: var(--spacing-lg) 0;
  color: var(--color-text-secondary);
  font-size: var(--font-size-caption);
}
</style>
