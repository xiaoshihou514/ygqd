<script setup lang="ts">
// 三板块唯一的列表编排容器:网格/行式、首屏骨架、空态、错误与重试、加载更多
// (无限滚动或按钮)。只负责展示与发事件——限速、去重、本地分页留在页面层。
//
// 无限滚动哨兵常驻 DOM、只用 v-show 显隐(不随条件分支销毁重建):这是
// 「搜索后翻页失效」的结构性修复,勿改回 v-if。哨兵用模板 ref,不做全局查找;
// keep-alive 页面在 activated/deactivated 时重新绑定/解绑。
import { computed, nextTick, onActivated, onDeactivated, onMounted, ref, watch } from 'vue'
import type { MediaItem, MediaKind } from '@/utils/mediaItem'
import { useInfiniteScroll } from '@/composables/useInfiniteScroll'
import MediaCard from '@/components/MediaCard.vue'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

const props = withDefaults(
  defineProps<{
    items: MediaItem[]
    layout?: 'grid' | 'row'
    /** 网格密度口径:comic 沿用 6→2 列断点;video/novel 用 auto-fill。缺省按首条 kind */
    density?: MediaKind
    /** 首屏加载(无数据时显示骨架) */
    loading?: boolean
    loadingMore?: boolean
    hasMore?: boolean
    /** 错误:无数据时整块错误态;有数据时顶部错误条 */
    error?: string
    pager?: 'infinite' | 'button' | 'none'
    moreLabel?: string
    loadingMoreLabel?: string
    emptyTitle?: string
    emptyMessage?: string
    /** 末尾提示;传空串不显示 */
    endText?: string
    skeletonCount?: number
    /** 首屏加载时骨架上方的说明(如站点校验耗时预期) */
    loadingHint?: string
    /** 无限滚动提前触发距离 */
    rootMargin?: string
  }>(),
  {
    layout: 'grid',
    density: undefined,
    loading: false,
    loadingMore: false,
    hasMore: false,
    error: '',
    pager: 'infinite',
    moreLabel: '加载更多',
    loadingMoreLabel: '正在加载更多...',
    emptyTitle: '暂无内容',
    emptyMessage: '',
    endText: '没有更多了',
    skeletonCount: 12,
    loadingHint: '',
    rootMargin: '200px',
  },
)
const emit = defineEmits<{ loadMore: []; retry: []; open: [item: MediaItem] }>()
defineSlots<{
  actions?: (p: { item: MediaItem }) => unknown
  empty?: () => unknown
}>()

const densityKind = computed<MediaKind>(() => props.density ?? props.items[0]?.kind ?? 'comic')
const showSkeleton = computed(() => props.loading && props.items.length === 0)
const showError = computed(() => !props.loading && !!props.error && props.items.length === 0)
const showEmpty = computed(
  () => !props.loading && !props.error && props.items.length === 0,
)
const sentinelActive = computed(
  () =>
    props.pager === 'infinite' &&
    props.hasMore &&
    props.items.length > 0 &&
    !props.loading &&
    !props.loadingMore,
)

// —— 无限滚动(常驻哨兵 + 模板 ref) ——
const sentinelEl = ref<HTMLElement | null>(null)
let active = true
const { setup: setupScroll, teardown: teardownScroll } = useInfiniteScroll({
  target: () => (active ? sentinelEl.value : null),
  enabled: () => sentinelActive.value,
  rootMargin: props.rootMargin,
  onHit: () => {
    console.info('[media-collection] 哨兵触发,请求下一页')
    emit('loadMore')
  },
})
function rebind() {
  void nextTick(() => {
    setupScroll()
  })
}
watch(sentinelActive, rebind)
watch(() => props.items.length, rebind)
onMounted(rebind)
onActivated(() => {
  active = true
  rebind()
})
onDeactivated(() => {
  active = false
  teardownScroll()
})
</script>

<template>
  <div class="media-collection" :class="[`layout-${layout}`, `density-${densityKind}`]">
    <div v-if="error && items.length > 0" class="mc-error-bar" role="alert">
      <span>{{ error }}</span>
      <button type="button" class="mc-link-btn" @click="emit('retry')">重试</button>
    </div>

    <p v-if="showSkeleton && loadingHint" class="mc-loading-hint">{{ loadingHint }}</p>
    <div v-if="showSkeleton" class="mc-items" aria-busy="true">
      <div v-for="n in (layout === 'row' ? 6 : skeletonCount)" :key="n" class="mc-skeleton">
        <div class="mc-skeleton-cover" />
        <div class="mc-skeleton-body">
          <div class="mc-skeleton-line w-80" />
          <div class="mc-skeleton-line w-60" />
        </div>
      </div>
    </div>

    <EmptyState v-else-if="showError" title="加载失败" :message="error">
      <button type="button" class="mc-retry" @click="emit('retry')">重试</button>
    </EmptyState>

    <template v-else-if="showEmpty">
      <slot name="empty">
        <EmptyState :title="emptyTitle" :message="emptyMessage" />
      </slot>
    </template>

    <div v-else class="mc-items">
      <MediaCard
        v-for="item in items"
        :key="item.key"
        :item="item"
        :layout="layout"
        @open="emit('open', $event)"
      >
        <template v-if="$slots.actions" #actions="slotProps">
          <slot name="actions" v-bind="slotProps" />
        </template>
      </MediaCard>
    </div>

    <div v-if="loadingMore && items.length > 0" class="mc-loading-more">
      <LoadingSpinner :message="loadingMoreLabel" />
    </div>
    <div
      v-else-if="pager === 'button' && hasMore && items.length > 0"
      class="mc-more-row"
    >
      <button type="button" class="mc-more-btn" @click="emit('loadMore')">{{ moreLabel }}</button>
    </div>
    <!-- 常驻哨兵:v-show 显隐,节点不销毁重建 -->
    <div v-show="sentinelActive" ref="sentinelEl" class="mc-sentinel" />
    <div
      v-if="endText && !hasMore && !loading && !loadingMore && items.length > 0"
      class="mc-end"
    >
      {{ endText }}
    </div>
  </div>
</template>

<style scoped>
.mc-items {
  display: grid;
  gap: var(--spacing-md);
}

/* 漫画:沿用用户习惯的 6→5→4→3→2 列密度 */
.layout-grid.density-comic .mc-items {
  grid-template-columns: repeat(6, minmax(0, 1fr));
  gap: var(--spacing-md);
}

@media (max-width: 1200px) {
  .layout-grid.density-comic .mc-items {
    grid-template-columns: repeat(5, minmax(0, 1fr));
  }
}

@media (max-width: 992px) {
  .layout-grid.density-comic .mc-items {
    grid-template-columns: repeat(4, minmax(0, 1fr));
  }
}

@media (max-width: 768px) {
  .layout-grid.density-comic .mc-items {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--spacing-sm);
  }
}

@media (max-width: 480px) {
  .layout-grid.density-comic .mc-items {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--spacing-xs);
  }
}

/* 视频/小说:auto-fill(视频 16:9 卡更宽) */
.layout-grid.density-video .mc-items {
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
}

.layout-grid.density-novel .mc-items {
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
}

@media (max-width: 480px) {
  .layout-grid.density-video .mc-items,
  .layout-grid.density-novel .mc-items {
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--spacing-xs);
  }
}

.layout-row .mc-items {
  grid-template-columns: minmax(0, 1fr);
  gap: var(--spacing-xs);
}

/* ───── 骨架 ───── */
.mc-skeleton {
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-lg);
  overflow: hidden;
  background: var(--color-card-bg);
}

.layout-row .mc-skeleton {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-xs) var(--spacing-sm);
}

.mc-skeleton-cover,
.mc-skeleton-line {
  background: linear-gradient(
    90deg,
    var(--color-divider-soft) 25%,
    var(--color-canvas) 50%,
    var(--color-divider-soft) 75%
  );
  background-size: 200% 100%;
  animation: mc-shimmer 1.5s infinite;
}

.mc-skeleton-cover {
  width: 100%;
  aspect-ratio: 5 / 7;
}

.density-video .mc-skeleton-cover {
  aspect-ratio: 16 / 9;
}

.layout-row .mc-skeleton-cover {
  width: 48px;
  border-radius: var(--radius-sm);
}

.mc-skeleton-body {
  flex: 1;
  padding: var(--spacing-sm);
}

.mc-skeleton-line {
  height: 12px;
  margin-bottom: var(--spacing-xs);
  border-radius: var(--radius-sm);
}

.mc-skeleton-line.w-80 {
  width: 80%;
}

.mc-skeleton-line.w-60 {
  width: 60%;
}

@keyframes mc-shimmer {
  0% {
    background-position: -200% 0;
  }

  100% {
    background-position: 200% 0;
  }
}

@media (prefers-reduced-motion: reduce) {
  .mc-skeleton-cover,
  .mc-skeleton-line {
    animation: none;
  }
}

/* ───── 状态行 ───── */
.mc-error-bar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-sm);
  padding: var(--spacing-xs) var(--spacing-sm);
  border-radius: var(--radius-sm);
  background: var(--color-divider-soft);
  color: var(--color-error);
  font-size: var(--font-size-caption);
}

.mc-link-btn {
  flex-shrink: 0;
  color: var(--color-primary);
  background: transparent;
  font-size: var(--font-size-caption);
}

.mc-retry,
.mc-more-btn {
  padding: 8px 22px;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-pill);
  background: var(--color-card-bg);
  color: var(--color-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.mc-more-btn:hover {
  border-color: var(--color-primary);
}

.mc-more-row,
.mc-loading-more {
  display: flex;
  justify-content: center;
  padding: var(--spacing-lg) 0;
}

.mc-loading-hint {
  margin: 0 0 var(--spacing-sm);
  font-size: var(--font-size-caption);
  color: var(--color-text-secondary);
}

.mc-sentinel {
  height: 1px;
}

.mc-end {
  padding: var(--spacing-lg) 0;
  text-align: center;
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}
</style>
