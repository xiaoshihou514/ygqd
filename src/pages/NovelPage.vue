<script setup lang="ts">
// 小说发现页:推荐(首页最新流)/日榜/月榜。站内搜索统一在底部「搜索」页(小说范围),
// 本页不再保留单独的搜索框
import { errorMessage } from '@/utils/errors'
import { useHlibGuard } from '@/composables/useHlibGuard'
import { computed, onActivated, onMounted, ref, watch } from 'vue'
import { novelRanking, novelRecommend } from '@/services/api'
import type { NovelHit } from '@/types'
import MediaCollection from '@/components/MediaCollection.vue'
import { fromNovelHit } from '@/utils/mediaItem'

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

function loadMore() {
  if (!hasMore.value || current.value.loading) return
  void loadTab(novelTab.value, current.value.page + 1)
}

// 发现流/榜单:点条目直接进阅读器(任务 7);榜单带序号角标
const currentItems = computed(() =>
  current.value.items.map((h, idx) =>
    fromNovelHit(h, { open: 'reader', rank: novelTab.value === 'recommend' ? undefined : idx + 1 }),
  ),
)
const loadingHint = computed(() =>
  novelTab.value === 'recommend'
    ? '正在加载(经小说源连接,首次可能需要十几秒)...'
    : '首次加载需要站点校验,约 30 秒...',
)

// 首次切到某个 tab 才加载(榜单需登录,不在进入页面时就打扰)
watch(novelTab, (tab) => {
  if (!states.value[tab].loaded && !states.value[tab].loading) void loadTab(tab, 1)
})

// 任务 2:keep-alive 返回时静默刷新数据(滚动位置由组件状态天然保留)
onActivated(() => {
  void loadTab(novelTab.value, 1)
})

onMounted(() => {
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

      <MediaCollection
        :items="currentItems"
        density="novel"
        root-margin="400px"
        :loading="current.loading && !current.items.length"
        :loading-hint="loadingHint"
        :loading-more="current.loading && current.items.length > 0"
        loading-more-label="加载中…(站点限速约 3 秒/次)"
        :has-more="hasMore"
        :error="current.error"
        end-text="已展示全部"
        empty-title="暂无内容"
        @load-more="loadMore"
        @retry="current.items.length ? loadMore() : refresh()"
      />
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



.refresh-btn {
  margin-left: auto;
  padding: 6px 14px;
}














</style>
