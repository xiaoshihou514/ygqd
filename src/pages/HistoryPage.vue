<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { useTimeline } from '@/composables/useTimeline'
import { computed, onActivated, onMounted, ref } from 'vue'
import { fetchItemHistory, fetchFavorites, removeFavorite } from '@/services/api'
import type { FavoriteEntry } from '@/types'
import { useViewHistory } from '@/composables/useViewHistory'
import MediaCollection from '@/components/MediaCollection.vue'
import { fromComicHistory, fromFavorite, fromTimeline, type MediaItem } from '@/utils/mediaItem'
import type { ItemHistoryEntry } from '@/types'

const { history, loading, load } = useViewHistory()

// 历史分组:全部(三板块时间线混排)/漫画/视频/小说
type HistoryScope = 'all' | 'comic' | 'video' | 'favorites'
const scope = ref<HistoryScope>('all')
const SCOPES: Array<{ key: HistoryScope; label: string }> = [
  { key: 'all', label: '全部' },
  { key: 'comic', label: '漫画' },
  { key: 'video', label: '视频' },
  { key: 'favorites', label: '收藏' },
]

// 全局历史(item_history:视频/小说;漫画走存量 view_history)
const itemHistory = ref<ItemHistoryEntry[]>([])
const itemLoading = ref(false)

// 重构笔 4:时间线混排逻辑迁移 useTimeline(行为零变化)
const { timeline, videoEntries } = useTimeline(history, itemHistory)
const timelineItems = computed(() => timeline.value.map((e) => fromTimeline(e)))
const comicHistoryItems = computed(() => history.value.map(fromComicHistory))
const videoHistoryItems = computed(() => videoEntries.value.map((e) => fromTimeline(e, { kindBadge: false })))
// —— 收藏分组(⑯ 关注页重组:收藏并入历史) ——
const favorites = ref<FavoriteEntry[]>([])
const favoriteItems = computed(() => favorites.value.map(fromFavorite))
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

/** 收藏条目 key(fav:source:comicId)反查原收藏 */
function favoriteOf(item: MediaItem): FavoriteEntry | undefined {
  return favorites.value.find((f) => `fav:${f.source}:${f.comicId}` === item.key)
}

async function unfavorite(fav: FavoriteEntry | undefined) {
  if (!fav) return
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

      <!-- 全部:三板块时间线混排(viewedAt 降序交错),行式 -->
      <MediaCollection
        v-if="scope === 'all'"
        :items="timelineItems"
        layout="row"
        :loading="loading || itemLoading"
        end-text=""
        empty-title="还没有浏览记录"
        empty-message="看漫画、播放视频都会自动记录在这里"
      />

      <!-- 漫画 -->
      <MediaCollection
        v-else-if="scope === 'comic'"
        :items="comicHistoryItems"
        density="comic"
        :loading="loading"
        end-text=""
        empty-title="还没有浏览记录"
        empty-message="查看漫画详情时会自动记录"
      />

      <!-- 视频(含播放进度) -->
      <MediaCollection
        v-else-if="scope === 'video'"
        :items="videoHistoryItems"
        density="video"
        :loading="itemLoading"
        end-text=""
        empty-title="还没有视频观看记录"
        empty-message="播放视频时会自动记录"
      />

      <!-- 收藏:行式 + 取消收藏操作 -->
      <MediaCollection
        v-else
        :items="favoriteItems"
        layout="row"
        :loading="favLoading"
        end-text=""
        empty-title="还没有收藏漫画"
        empty-message="在漫画详情页点击「收藏」,两个来源的漫画都可以收藏"
      >
        <template #actions="{ item }">
          <button
            type="button"
            class="favorite-remove"
            title="取消收藏"
            aria-label="取消收藏"
            @click.stop="unfavorite(favoriteOf(item))"
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
              <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" />
            </svg>
          </button>
        </template>
      </MediaCollection>
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







</style>
