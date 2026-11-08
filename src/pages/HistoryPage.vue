<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { computed, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { fetchItemHistory, fetchFavorites, removeFavorite } from '@/services/api'
import type { FavoriteEntry } from '@/types'
import { useViewHistory } from '@/composables/useViewHistory'
import ProxyImage from '@/components/ProxyImage.vue'
import VideoCardGrid from '@/components/VideoCardGrid.vue'
import ComicGrid from '@/components/ComicGrid.vue'
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

/** 时间线条目统一形态(漫画存量转换 + item_history) */
interface TimelineEntry {
  kind: 'comic' | 'video' | 'novel'
  source: string
  itemId: string
  title: string
  viewedAt: number
  /** 封面(仅漫画存量有;视频/小说历史无封面数据,展示 kind 图标占位) */
  thumbnail?: string
  /** 小说阅读进度(progress JSON 解析) */
  chapter?: number
  /** 视频/小说的进度 JSON 原文(视频含播放位置秒数) */
  progress?: string
  /** 漫画详情跳转用(存量字段) */
  categoryId?: number
}

/** 三板块时间线混排:viewedAt 降序交错(用户「时间线混排」要求) */
const timeline = computed<TimelineEntry[]>(() => {
  const comicEntries: TimelineEntry[] = history.value.map((h) => ({
    kind: 'comic',
    source: h.source,
    itemId: h.comicId,
    title: h.title,
    viewedAt: h.viewedAt,
    thumbnail: h.thumbnail,
    categoryId: h.categoryId,
  }))
  const novelEntries: TimelineEntry[] = itemHistory.value
    .filter((e) => e.kind === 'novel')
    .map((e) => {
      let chapter: number | undefined
      try {
        const p = e.progress ? (JSON.parse(e.progress) as { chapter?: number }) : undefined
        chapter = p?.chapter
      } catch {
        /* progress JSON 损坏忽略 */
      }
      return { kind: 'novel' as const, source: e.source, itemId: e.itemId, title: e.title, viewedAt: e.viewedAt, chapter, progress: e.progress }
    })
  const videoEntries: TimelineEntry[] = itemHistory.value
    .filter((e) => e.kind === 'video')
    .map((e) => ({ kind: 'video' as const, source: e.source, itemId: e.itemId, title: e.title, viewedAt: e.viewedAt, thumbnail: videoThumb(e.itemId) }))
  return [...comicEntries, ...novelEntries, ...videoEntries].sort((a, b) => b.viewedAt - a.viewedAt)
})

const videoEntries = computed(() => timeline.value.filter((e) => e.kind === 'video'))
const novelEntries = computed(() =>
  timeline.value
    .filter((e) => e.kind === 'novel')
    .map((e) => {
      let page: number | undefined
      try {
        page = e.progress ? (JSON.parse(e.progress) as { page?: number }).page : undefined
      } catch {
        /* progress JSON 损坏忽略 */
      }
      return { ...e, page }
    }),
)

/** 视频封面构造(与后端 parse_cards 同源公式,fetch_video_image 代理加载) */
function videoThumb(code: string): string {
  return `https://fourhoi.com/${code}/cover-t.jpg`
}

/** 漫画分组网格数据(模板内联 map 的 computed 化,修复表达式断裂) */
const comicGridItems = computed(() =>
  history.value.map((h) => ({
    source: h.source,
    id: h.comicId,
    title: h.title,
    thumbnail: h.thumbnail,
    category: '',
    categoryId: h.categoryId,
    tags: [] as string[],
    likes: '',
    link: '',
  })),
)

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

/** 时间线条目跳转:小说→系列详情/视频→详情/漫画→详情(存量字段) */
function openTimelineEntry(entry: TimelineEntry) {
  if (entry.kind === 'novel') {
    router.push(`/novels/series/${entry.itemId}`)
  } else if (entry.kind === 'video') {
    router.push(`/videos/${entry.itemId}`)
  } else {
    router.push(`/comic/${entry.source}/${entry.categoryId ?? 9}/${entry.itemId}`)
  }
}

function kindLabel(kind: string): string {
  return kind === 'comic' ? '漫画' : kind === 'video' ? '视频' : '小说'
}

function formatTime(ts: number): string {
  const d = new Date(ts)
  const M = String(d.getMonth() + 1).padStart(2, '0')
  const D = String(d.getDate()).padStart(2, '0')
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  return `${M}-${D} ${hh}:${mm}`
}

function formatPosition(seconds: number): string {
  const m = Math.floor(seconds / 60)
  const s = Math.floor(seconds % 60)
  return `看到 ${m} 分 ${s} 秒`
}

onMounted(async () => {
  await Promise.all([load(), loadItemHistory(), loadFavorites()])
})

/** 视频条目的播放位置(progress JSON 解析) */
function entryPosition(e: TimelineEntry): number | undefined {
  try {
    return e.progress ? (JSON.parse(e.progress) as { position?: number }).position : undefined
  } catch {
    return undefined
  }
}
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
        <div v-else class="timeline">
          <button
            v-for="(e, i) in timeline"
            :key="`${e.kind}-${e.itemId}-${i}`"
            type="button"
            class="timeline-row"
            @click="openTimelineEntry(e)"
          >
            <span v-if="e.thumbnail && e.kind === 'video'" class="timeline-thumb">
              <ProxyImage :src="e.thumbnail" source="video" class="timeline-thumb-img" loading="lazy" />
            </span>
            <span v-else-if="e.thumbnail && e.kind === 'comic'" class="timeline-thumb">
              <ProxyImage :src="e.thumbnail" :source="e.source as import('@/config/sources').SourceId" class="timeline-thumb-img" loading="lazy" />
            </span>
            <span v-else class="timeline-thumb placeholder" :class="`ph-${e.kind}`">
              <svg v-if="e.kind === 'video'" width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
                <path d="M8 5v14l11-7z" />
              </svg>
              <svg v-else-if="e.kind === 'novel'" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
                <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
              </svg>
              <svg v-else width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <rect x="3" y="3" width="18" height="18" rx="2" />
                <circle cx="8.5" cy="8.5" r="1.5" />
                <path d="M21 15l-5-5L5 21" />
              </svg>
            </span>
            <span class="timeline-kind-badge" :class="`kind-${e.kind}`">{{ kindLabel(e.kind) }}</span>
            <span class="timeline-title">{{ e.title }}</span>
            <span v-if="e.kind === 'novel' && e.chapter" class="timeline-progress">
              读到第 {{ e.chapter }} 章
            </span>
            <span v-else-if="e.kind === 'video' && entryPosition(e)" class="timeline-progress">
              {{ formatPosition(entryPosition(e)!) }}
            </span>
            <span class="timeline-time">{{ formatTime(e.viewedAt) }}</span>
          </button>
        </div>
      </template>

      <!-- 漫画:存量网格 -->
      <template v-else-if="scope === 'comic'">
        <LoadingSpinner v-if="loading" message="加载中..." />

        <EmptyState
          v-else-if="history.length === 0"
          title="还没有浏览记录"
          message="查看漫画详情时会自动记录"
        />

        <ComicGrid v-else :items="comicGridItems" />
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
              <span class="favorite-meta">{{ fav.author || '' }} · 收藏于 {{ formatTime(fav.createdAt) }}</span>
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

      <!-- 小说:全局历史条目(含章节进度) -->
      <template v-else>
        <LoadingSpinner v-if="itemLoading" message="加载阅读记录..." />
        <EmptyState
          v-else-if="novelEntries.length === 0"
          title="还没有阅读记录"
          message="阅读小说时会自动记录进度"
        />
        <div v-else class="timeline">
          <button
            v-for="(e, i) in novelEntries"
            :key="`${e.itemId}-${i}`"
            type="button"
            class="timeline-row"
            @click="openTimelineEntry(e)"
          >
            <span class="timeline-kind-badge" :class="`kind-${e.kind}`">{{ kindLabel(e.kind) }}</span>
            <span class="timeline-title">{{ e.title }}</span>
            <span v-if="e.chapter" class="timeline-progress">读到第 {{ e.chapter }} 章{{ e.page ? ` 第 ${e.page} 页` : '' }}</span>
            <span class="timeline-time">{{ formatTime(e.viewedAt) }}</span>
          </button>
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

.timeline {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-xs);
}

.timeline-row {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: 10px var(--spacing-md);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  background: var(--color-card-bg);
  cursor: pointer;
  text-align: left;
  transition: border-color 0.15s;
}

.timeline-row:hover {
  border-color: var(--color-primary);
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
</style>
