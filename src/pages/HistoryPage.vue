<script setup lang="ts">
import { computed, nextTick, onActivated, onMounted, onUnmounted, ref } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'
import { errorMessage } from '@/utils/errors'
import { fetchItemHistory, fetchFavorites, removeFavorite } from '@/services/api'
import type { FavoriteEntry, ItemHistoryEntry } from '@/types'
import { useTimeline } from '@/composables/useTimeline'
import { useViewHistory } from '@/composables/useViewHistory'
import MediaCollection from '@/components/MediaCollection.vue'
import { fromComicHistory, fromFavorite, fromTimeline, type MediaItem } from '@/utils/mediaItem'

const { history, loading, load } = useViewHistory()

// —— 历史分组:全部(时间线混排)/漫画/视频/收藏;每组独立记忆滚动位置 ——
type HistoryScope = 'all' | 'comic' | 'video' | 'favorites'
const SCOPES: Array<{ key: HistoryScope; label: string }> = [
  { key: 'all', label: '全部' },
  { key: 'comic', label: '漫画' },
  { key: 'video', label: '视频' },
  { key: 'favorites', label: '收藏' },
]
const scope = ref<HistoryScope>('all')
const scopeIndex = computed(() => SCOPES.findIndex((s) => s.key === scope.value))
const scopeScroll: Record<HistoryScope, number> = { all: 0, comic: 0, video: 0, favorites: 0 }

async function switchScope(next: HistoryScope) {
  if (next === scope.value) return
  scopeScroll[scope.value] = window.scrollY
  scope.value = next
  await nextTick()
  window.scrollTo(0, scopeScroll[next])
}

// —— 全局历史(item_history:视频;漫画走存量 view_history) ——
const itemHistory = ref<ItemHistoryEntry[]>([])
const itemLoading = ref(false)
const itemLoaded = ref(false)
let itemSeq = 0

async function loadItemHistory() {
  const seq = ++itemSeq
  itemLoading.value = true
  try {
    const list = await fetchItemHistory(undefined, undefined, 100)
    if (seq === itemSeq) itemHistory.value = list
  } catch (e) {
    console.error('[history]', errorMessage(e))
  } finally {
    if (seq === itemSeq) {
      itemLoading.value = false
      itemLoaded.value = true
    }
  }
}

const { timeline, videoEntries } = useTimeline(history, itemHistory)
const timelineItems = computed(() => timeline.value.map((e) => fromTimeline(e)))
const comicHistoryItems = computed(() => history.value.map(fromComicHistory))
const videoHistoryItems = computed(() => videoEntries.value.map((e) => fromTimeline(e, { kindBadge: false })))

// 骨架只在「从未加载成功」时显示:keep-alive 返回的静默刷新不闪骨架
const comicFirstLoading = computed(() => loading.value && !history.value.length)
const itemFirstLoading = computed(() => itemLoading.value && !itemLoaded.value)

// —— 收藏分组(⑯ 关注页重组:收藏并入历史) ——
const favorites = ref<FavoriteEntry[]>([])
const favoriteItems = computed(() => favorites.value.map(fromFavorite))
const favLoading = ref(false)
const favLoaded = ref(false)
const favError = ref('')
let favSeq = 0

async function loadFavorites() {
  const seq = ++favSeq
  favLoading.value = true
  try {
    const list = await fetchFavorites()
    if (seq === favSeq) favorites.value = list
  } catch (e) {
    console.error('[history]', errorMessage(e))
  } finally {
    if (seq === favSeq) {
      favLoading.value = false
      favLoaded.value = true
    }
  }
}

const favKey = (f: FavoriteEntry) => `fav:${f.source}:${f.comicId}`

/** 收藏条目 key(fav:source:comicId)反查原收藏 */
function favoriteOf(item: MediaItem): FavoriteEntry | undefined {
  return favorites.value.find((f) => favKey(f) === item.key)
}

// —— 删除类操作二次确认:首次点击「待确认」,3 秒内再点才执行(与关注页取关一致) ——
const ARM_TIMEOUT_MS = 3000
const armedKey = ref<string | null>(null)
let armTimer: ReturnType<typeof setTimeout> | undefined
function confirmTap(key: string, action: () => void) {
  clearTimeout(armTimer)
  if (armedKey.value === key) {
    armedKey.value = null
    action()
    return
  }
  armedKey.value = key
  armTimer = setTimeout(() => (armedKey.value = null), ARM_TIMEOUT_MS)
}
onUnmounted(() => clearTimeout(armTimer))

/** 乐观取消收藏:先移除,失败回滚到原位置并提示 */
async function unfavorite(item: MediaItem) {
  const fav = favoriteOf(item)
  if (!fav) return
  favError.value = ''
  const prev = favorites.value
  favorites.value = prev.filter((f) => f !== fav)
  try {
    await removeFavorite(fav.source, fav.comicId)
  } catch (e) {
    favorites.value = prev
    favError.value = `取消收藏失败:${errorMessage(e)}`
  }
}

onMounted(async () => {
  const loadComicHistory = load().catch((e: unknown) => console.error('[history]', errorMessage(e)))
  await Promise.all([loadComicHistory, loadItemHistory(), loadFavorites()])
})

// 离开路由时 DOM 尚在,记录的 scrollY 可靠(deactivated 时页面已移出文档)
onBeforeRouteLeave(() => {
  scopeScroll[scope.value] = window.scrollY
})

// keep-alive 返回:恢复滚动 + 静默刷新(首次挂载也会触发 activated,跳过避免重复请求)。
// 漫画历史由 useViewHistory.record() 在详情页写入时已同步刷新。
let activatedOnce = false
onActivated(() => {
  if (!activatedOnce) {
    activatedOnce = true
    return
  }
  void nextTick(() => window.scrollTo(0, scopeScroll[scope.value]))
  void loadItemHistory()
  void loadFavorites()
})
</script>

<template>
  <div class="history-page">
    <div class="container">
      <div class="page-heading">
        <h2 class="page-title">浏览历史</h2>
      </div>

      <div
        class="segmented"
        role="tablist"
        aria-label="历史范围"
        :style="{ '--seg-count': SCOPES.length, '--seg-index': scopeIndex }"
      >
        <span class="segmented-thumb" aria-hidden="true" />
        <button
          v-for="s in SCOPES"
          :key="s.key"
          type="button"
          class="segmented-btn"
          :class="{ active: scope === s.key }"
          role="tab"
          :aria-selected="scope === s.key"
          @click="switchScope(s.key)"
        >
          {{ s.label }}
        </button>
      </div>

      <!-- 全部:两板块时间线混排(viewedAt 降序交错),行式 -->
      <MediaCollection
        v-if="scope === 'all'"
        :items="timelineItems"
        layout="row"
        :loading="comicFirstLoading || itemFirstLoading"
        end-text=""
        empty-title="还没有浏览记录"
        empty-message="看漫画、播放视频都会自动记录在这里"
      />

      <!-- 漫画 -->
      <MediaCollection
        v-else-if="scope === 'comic'"
        :items="comicHistoryItems"
        density="comic"
        :loading="comicFirstLoading"
        end-text=""
        empty-title="还没有浏览记录"
        empty-message="查看漫画详情时会自动记录"
      />

      <!-- 视频(含播放进度) -->
      <MediaCollection
        v-else-if="scope === 'video'"
        :items="videoHistoryItems"
        density="video"
        :loading="itemFirstLoading"
        end-text=""
        empty-title="还没有视频观看记录"
        empty-message="播放视频时会自动记录"
      />

      <!-- 收藏:行式 + 取消收藏(二次确认) -->
      <template v-else>
        <p v-if="favError" class="status-line is-error" role="alert">{{ favError }}</p>
        <MediaCollection
          :items="favoriteItems"
          layout="row"
          :loading="favLoading && !favLoaded"
          end-text=""
          empty-title="还没有收藏漫画"
          empty-message="在漫画详情页点击「收藏」,两个来源的漫画都可以收藏"
        >
          <template #actions="{ item }">
            <button
              type="button"
              class="favorite-remove"
              :class="{ armed: armedKey === item.key }"
              :aria-label="armedKey === item.key ? `确认取消收藏 ${item.title}` : `取消收藏 ${item.title}`"
              @click.stop="confirmTap(item.key, () => unfavorite(item))"
            >
              <span v-if="armedKey === item.key" class="favorite-remove-confirm">取消收藏</span>
              <svg v-else width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" />
              </svg>
            </button>
          </template>
        </MediaCollection>
      </template>
    </div>
  </div>
</template>

<style scoped>
.history-page {
  padding-bottom: var(--spacing-xl);
}

/* —— 页头(关注/历史/目标作品三页同款) —— */
.page-heading {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding-top: var(--spacing-lg);
  margin-bottom: var(--spacing-lg);
}

.page-title {
  flex: 1;
  min-width: 0;
  margin: 0;
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  line-height: var(--line-height-heading);
  color: var(--color-text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* —— 分段控件(关注/历史两页同款):滑块用 transform 平移 —— */
.segmented {
  position: relative;
  display: grid;
  grid-template-columns: repeat(var(--seg-count), minmax(0, 1fr));
  width: 100%;
  max-width: calc(var(--seg-count) * 112px);
  padding: 2px;
  margin-bottom: var(--spacing-lg);
  border-radius: var(--radius-pill);
  background: var(--color-divider-soft);
}

.segmented-thumb {
  position: absolute;
  top: 2px;
  bottom: 2px;
  left: 2px;
  width: calc((100% - 4px) / var(--seg-count));
  border-radius: var(--radius-pill);
  background: var(--color-card-bg);
  box-shadow: 0 0 0 1px var(--color-border);
  transform: translateX(calc(var(--seg-index) * 100%));
  transition: transform var(--duration-base) var(--ease-out);
}

.segmented-btn {
  position: relative;
  min-height: 40px;
  padding: 0 var(--spacing-sm);
  border-radius: var(--radius-pill);
  color: var(--color-text-secondary);
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  white-space: nowrap;
  transition: color var(--duration-base) var(--ease-standard);
}

.segmented-btn.active {
  color: var(--color-text-primary);
}

.segmented-btn:focus-visible {
  outline: 2px solid var(--color-accent);
  outline-offset: -2px;
}

/* —— 状态文案 —— */
.status-line {
  margin: calc(var(--spacing-sm) * -1) 0 var(--spacing-md);
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
}

.status-line.is-error {
  color: var(--color-error);
}

/* —— 取消收藏:44px 命中区,首次点击变「取消收藏」确认态 —— */
.favorite-remove {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  min-width: 44px;
  min-height: 44px;
  padding: 0 var(--spacing-xs);
  border-radius: var(--radius-pill);
  color: var(--color-like);
  transition:
    background-color var(--duration-fast) var(--ease-standard),
    color var(--duration-fast) var(--ease-standard);
}

.favorite-remove.armed {
  color: var(--color-error);
}

.favorite-remove-confirm {
  padding: var(--spacing-xxs) var(--spacing-sm);
  border: 1px solid currentColor;
  border-radius: var(--radius-pill);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  white-space: nowrap;
}

@media (hover: hover) {
  .favorite-remove:hover {
    background: var(--color-tag-bg);
  }
}
</style>
