<script setup lang="ts">
// 小说发现页:推荐(首页最新流)/日榜/月榜。站内搜索统一在底部「搜索」页(小说范围),
// 本页不再保留单独的搜索框
import { errorMessage } from '@/utils/errors'
import { formatCount } from '@/utils/formatCount'
import { useInfiniteScroll } from '@/composables/useInfiniteScroll'
import { useHlibGuard } from '@/composables/useHlibGuard'
import { parseLikes } from '@/utils/likes'
import { computed, nextTick, onActivated, onMounted, onUnmounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { novelRanking, novelRecommend } from '@/services/api'
import type { NovelHit } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

const router = useRouter()

type NovelTab = 'recommend' | 'daily' | 'monthly'
const NOVEL_TABS: Array<{ key: NovelTab; label: string }> = [
  { key: 'recommend', label: '推荐' },
  { key: 'daily', label: '日榜' },
  { key: 'monthly', label: '月榜' },
]
const novelTab = ref<NovelTab>('recommend')

/** 每个 tab 独立的列表状态(切 tab 不丢已加载内容) */
interface TabState {
  items: NovelHit[]
  page: number
  totalPages: number
  loaded: boolean
  loading: boolean
  error: string
}
const emptyState = (): TabState => ({
  items: [],
  page: 0,
  totalPages: 1,
  loaded: false,
  loading: false,
  error: '',
})
const states = ref<Record<NovelTab, TabState>>({
  recommend: emptyState(),
  daily: emptyState(),
  monthly: emptyState(),
})
const current = computed(() => states.value[novelTab.value])
const hasMore = computed(() => current.value.page < current.value.totalPages)

// hlib 统一节流:各 tab 共用同一站点反爬闸,相邻抓取保底 3 秒
const { guard: guardHlibInterval, lastAt: lastHlibFetchAt } = useHlibGuard()

function fetchPage(tab: NovelTab, page: number) {
  if (tab === 'recommend') return novelRecommend(page)
  return novelRanking(tab === 'daily' ? 'day' : 'month', page)
}

async function loadTab(tab: NovelTab, page: number) {
  const st = states.value[tab]
  if (st.loading) return
  st.loading = true
  st.error = ''
  try {
    await guardHlibInterval()
    const res = await fetchPage(tab, page)
    if (page <= 1) {
      st.items = res.hits
    } else {
      const seen = new Set(st.items.map((h) => h.id))
      st.items.push(...res.hits.filter((h) => !seen.has(h.id)))
    }
    st.page = res.page
    st.totalPages = res.totalPages
    st.loaded = true
  } catch (e) {
    const msg = errorMessage(e)
    // 深页空页后端抛「未解析到系列链接」:视为到达末尾
    if (page > 1 && msg.includes('未解析到系列链接')) {
      st.totalPages = st.page
    } else {
      st.error = msg || '加载失败'
    }
  } finally {
    st.loading = false
  }
}

function refresh() {
  void loadTab(novelTab.value, 1)
}

// 统一 11:发现流/榜单无限加载(哨兵模式;小说 3 秒限速保留在 loadMore 内)
const { setup: setupSentinel } = useInfiniteScroll({
  target: () => document.querySelector('.scroll-sentinel'),
  onHit: () => {
    void loadMore()
    return nextTick(setupSentinel)
  },
  enabled: () => hasMore.value && !current.value.loading,
  rootMargin: '400px',
})

function loadMore() {
  if (!hasMore.value) return
  void loadTab(novelTab.value, current.value.page + 1)
}

function openSeries(hit: NovelHit) {
  // 任务 7:点条目直接进阅读器(阅读器自动取首章/续读进度,不经章节列表)
  void router.push(`/novels/${hit.id}`)
}

// 首次切到某个 tab 才加载(榜单需登录,不在进入页面时就打扰)
watch(novelTab, (tab) => {
  if (!states.value[tab].loaded && !states.value[tab].loading) void loadTab(tab, 1)
})

// 任务 2:keep-alive 返回时静默刷新数据(滚动位置由组件状态天然保留)
onActivated(() => {
  void loadTab(novelTab.value, 1).then(() => nextTick(setupSentinel))
})

onMounted(() => {
  void nextTick(setupSentinel)
  void loadTab('recommend', 1)
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
        <button
          type="button"
          class="add-btn ghost refresh-btn"
          :disabled="current.loading"
          @click="refresh"
        >
          {{ current.loading ? '加载中…' : '刷新' }}
        </button>
      </div>

      <p v-if="current.error" class="add-error">{{ current.error }}</p>

      <div v-if="current.loading && !current.items.length" class="tab-loading">
        <LoadingSpinner message="正在加载(经小说源连接,首次可能需要十几秒)..." />
      </div>

      <div v-if="current.items.length" class="hit-list">
        <button
          v-for="(hit, idx) in current.items"
          :key="hit.id"
          type="button"
          class="hit-row"
          @click="openSeries(hit)"
        >
          <span v-if="novelTab !== 'recommend'" class="hit-rank">{{ idx + 1 }}</span>
          <div class="hit-main">
            <span class="hit-title">{{ hit.title }}</span>
            <span
              v-if="hit.author || (hit.tags && hit.tags.length > 0) || hit.wordCount || hit.views || hit.favorites || hit.publishedDate"
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
              <template v-if="hit.wordCount"> · {{ hit.wordCount }}</template>
              <template v-if="hit.views"> · 阅读 {{ formatCount(parseLikes(hit.views)) }}</template>
              <template v-if="hit.favorites"> · 收藏 {{ formatCount(parseLikes(hit.favorites)) }}</template>
              <template v-if="hit.publishedDate"> · {{ hit.publishedDate }}</template>
            </span>
          </div>
        </button>
      </div>
      <p v-else-if="current.loaded && !current.loading && !current.error" class="hit-end">暂无内容</p>

      <div v-if="current.items.length && hasMore" class="scroll-sentinel" />
      <div v-if="current.loading && current.items.length" class="hit-loading-more">
        <LoadingSpinner message="加载中…(站点限速约 3 秒/次)" />
      </div>
      <div v-else-if="current.items.length && !hasMore" class="hit-end">
        已展示全部
      </div>
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
}

.tab-loading {
  padding: var(--spacing-lg) 0;
}

.refresh-btn {
  margin-left: auto;
  padding: 6px 14px;
}

.hit-rank {
  flex-shrink: 0;
  min-width: 22px;
  text-align: center;
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-primary);
}

.hit-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  width: 100%;
  padding: 8px 12px;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  background: var(--color-card-bg);
  text-align: left;
  cursor: pointer;
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
  /* 任务 15:标题完全显示(可换行) */
  word-break: break-word;
  overflow-wrap: anywhere;
  white-space: normal;
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

.add-error {
  font-size: var(--font-size-fine-print);
  color: var(--color-error);
  margin: 0 0 var(--spacing-sm);
}


</style>
