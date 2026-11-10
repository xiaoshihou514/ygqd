<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { useTimeline } from '@/composables/useTimeline'
import TimelineList from '@/components/TimelineList.vue'
import NovelSeriesCard from '@/components/NovelSeriesCard.vue'
import { computed, onActivated, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { fetchItemHistory, fetchFavorites, removeFavorite } from '@/services/api'
import { formatTimestamp } from '@/utils/formatTime'
import type { FavoriteEntry } from '@/types'
import { useViewHistory } from '@/composables/useViewHistory'
import ProxyImage from '@/components/ProxyImage.vue'
import VideoCardGrid from '@/components/VideoCardGrid.vue'
import MediaCollection from '@/components/MediaCollection.vue'
import { fromComicHistory } from '@/utils/mediaItem'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import type { ItemHistoryEntry } from '@/types'

const router = useRouter()
const { history, loading, load } = useViewHistory()

// 历史分组:全部(三板块时间线混排)/漫画/视频/小说
type HistoryScope = 'all' | 'comic' | 'video' | 'novel' | 'favorites'
const scope = ref<HistoryScope>('all')
const SCOPES: Array<{ key: HistoryScope; label: string }> = [
  { key: 'all', label: '全部' },
  { key: 'comic', label: '漫画' },
  { key: 'video', label: '视频' },
  { key: 'novel', label: '小说' },
  { key: 'favorites', label: '收藏' },
]

// 全局历史(item_history:视频/小说;漫画走存量 view_history)
const itemHistory = ref<ItemHistoryEntry[]>([])
const itemLoading = ref(false)

// 重构笔 4:时间线混排逻辑迁移 useTimeline(行为零变化)
const { timeline, videoEntries, novelEntries, entryPosition, openTimelineEntry, kindLabel, formatPosition } = useTimeline(history, itemHistory)
const comicHistoryItems = computed(() => history.value.map(fromComicHistory))
// —— 收藏分组(⑯ 关注页重组:收藏并入历史) ——
const favorites = ref<FavoriteEntry[]>([])
const favLoading = ref(false)

async function loadFavorites() {
  favLoading.value = true
  try {
    favorites.value = await fetchFavorites()
  } catch (e) {
    console.error('[history]', errorMessage(e))
  } finally {
    favLoading.value = false
  }
}

async function unfavorite(fav: FavoriteEntry) {
  try {
    await removeFavorite(fav.source, fav.comicId)
    favorites.value = favorites.value.filter(
      (f) => !(f.source === fav.source && f.comicId === fav.comicId),
    )
  } catch (e) {
    console.error('[history]', errorMessage(e))
  }
}

async function loadItemHistory() {
  itemLoading.value = true
  try {
    itemHistory.value = await fetchItemHistory(undefined, undefined, 100)
  } catch (e) {
    console.error('[history]', errorMessage(e))
  } finally {
    itemLoading.value = false
  }
}

// 任务 2:keep-alive 返回时静默刷新(数据更新,滚动位置保留)
onActivated(() => {
  void loadItemHistory()
  void loadFavorites()
})

onMounted(async () => {
  await Promise.all([load(), loadItemHistory(), loadFavorites()])
})
</script>

<template>
  <div class="history-page">
    <div class="container">
      <h2 class="page-title">浏览历史</h2>

      <div class="scope-bar" role="tablist" aria-label="历史范围">
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

      <!-- 全部:三板块时间线混排(viewedAt 降序交错) -->
      <template v-if="scope === 'all'">
        <LoadingSpinner v-if="loading || itemLoading" message="加载历史..." />
        <EmptyState
          v-else-if="timeline.length === 0"
          title="还没有浏览记录"
          message="看漫画、播放视频、读小说都会自动记录在这里"
        />
        <TimelineList
          v-else
          :entries="timeline"
          :loading="loading || itemLoading"
          empty-title="还没有浏览记录"
          empty-message="看漫画、播放视频、读小说都会自动记录在这里"
          @open="openTimelineEntry"
        />
      </template>

      <!-- 漫画:存量网格 -->
      <template v-else-if="scope === 'comic'">
        <LoadingSpinner v-if="loading" message="加载中..." />

        <EmptyState
          v-else-if="history.length === 0"
          title="还没有浏览记录"
          message="查看漫画详情时会自动记录"
        />

        <MediaCollection v-else :items="comicHistoryItems" density="comic" end-text="" />
      </template>

      <!-- 视频:封面网格(与视频板块 VideoCardGrid 统一,backend 布局建议) -->
      <template v-else-if="scope === 'video'">
        <LoadingSpinner v-if="itemLoading" message="加载视频记录..." />
        <EmptyState
          v-else-if="videoEntries.length === 0"
          title="还没有视频观看记录"
          message="播放视频时会自动记录"
        />
        <VideoCardGrid
          v-else
          :items="videoEntries.map((e) => ({
            code: e.itemId,
            title: e.title,
            thumbnail: e.thumbnail ?? '',
            tags: [],
            views: '',
            likes: '',
          }))"
        />
      </template>

      <!-- 收藏:哔咔/本站收藏列表(⑯ 关注页重组迁入) -->
      <template v-else-if="scope === 'favorites'">
        <LoadingSpinner v-if="favLoading" message="加载收藏..." />
        <EmptyState
          v-else-if="!favorites.length"
          title="还没有收藏漫画"
          message="在漫画详情页点击「收藏」,两个来源的漫画都可以收藏"
        />
        <div v-else class="favorite-list">
          <div
            v-for="fav in favorites"
            :key="`${fav.source}:${fav.comicId}`"
            class="favorite-card"
            @click="router.push(`/comic/${fav.source}/${fav.categoryId ?? 9}/${fav.comicId}`)"
          >
            <ProxyImage
              :src="fav.thumbnail"
              :source="fav.source"
              class="favorite-thumb"
              loading="lazy"
            />
            <div class="favorite-info">
              <span class="favorite-title">{{ fav.title }}</span>
              <span class="favorite-meta">{{ fav.author || '' }} · 收藏于 {{ formatTimestamp(fav.createdAt) }}</span>
            </div>
            <button
              type="button"
              class="favorite-remove"
              title="取消收藏"
              aria-label="取消收藏"
              @click.stop="unfavorite(fav)"
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" />
              </svg>
            </button>
          </div>
        </div>
      </template>

      <!-- 小说:封面网格(与漫画同一形态,用户口径「像漫画一样」) -->
      <template v-else>
        <LoadingSpinner v-if="itemLoading" message="加载阅读记录..." />
        <EmptyState
          v-else-if="novelEntries.length === 0"
          title="还没有阅读记录"
          message="阅读小说时会自动记录进度"
        />
        <div v-else class="novel-grid">
          <NovelSeriesCard
            v-for="e in novelEntries"
            :key="e.itemId"
            :series="{
              id: e.itemId,
              title: e.title,
              author: e.source,
              cover: '',
              chapters: [],
              addedAt: e.viewedAt,
            }"
            :progress="null"
            @open="router.push(`/novels/series/${e.itemId}`)"
          />
        </div>
      </template>

    </div>
  </div>
</template>

<style scoped>
.history-page {
  padding-bottom: var(--spacing-xl);
}

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

.page-title {
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-lg);
  padding-top: var(--spacing-lg);
}

.timeline-thumb {
  flex-shrink: 0;
  width: 44px;
  height: 33px;
  border-radius: var(--radius-sm);
  overflow: hidden;
  background: var(--color-divider-soft);
  display: flex;
  align-items: center;
  justify-content: center;
}

.timeline-thumb.placeholder {
  color: var(--color-text-muted);
}

.timeline-thumb.placeholder.ph-video {
  color: #6741d9;
  opacity: 0.6;
}

.timeline-thumb.placeholder.ph-novel {
  color: #8a6d2f;
  opacity: 0.6;
}

.timeline-thumb-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.favorite-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-xs);
}

.favorite-card {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-sm) var(--spacing-md);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  background: var(--color-card-bg);
  cursor: pointer;
  transition: border-color 0.15s;
}

.favorite-card:hover {
  border-color: var(--color-primary);
}

.favorite-thumb {
  flex-shrink: 0;
  width: 44px;
  height: 33px;
  border-radius: var(--radius-sm);
  overflow: hidden;
  object-fit: cover;
}

.favorite-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.favorite-title {
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.favorite-meta {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
}

.favorite-remove {
  flex-shrink: 0;
  border: none;
  background: transparent;
  color: #e8590c;
  cursor: pointer;
  padding: 6px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.favorite-remove:hover {
  background: rgba(232, 89, 12, 0.1);
}

.timeline-kind-badge {
  flex-shrink: 0;
  padding: 1px 8px;
  border-radius: 4px;
  font-size: 10px;
  background: var(--color-divider-soft);
  color: var(--color-text-secondary);
}

.timeline-kind-badge.kind-video {
  background: rgba(103, 65, 217, 0.14);
  color: #6741d9;
}

.timeline-kind-badge.kind-novel {
  background: rgba(138, 109, 47, 0.14);
  color: #8a6d2f;
}

.timeline-title {
  flex: 1;
  min-width: 0;
  font-size: var(--font-size-caption);
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.timeline-progress {
  flex-shrink: 0;
  font-size: var(--font-size-fine-print);
  color: var(--color-primary);
}

.timeline-time {
  flex-shrink: 0;
  font-size: var(--font-size-fine-print);
  color: var(--color-text-muted);
}

.novel-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: var(--spacing-sm);
}
</style>
