<script setup lang="ts">
// 三板块统一卡片(漫画/视频/小说;推荐/搜索/榜单/历史/关注/收藏共用)。
// 入参只认 MediaItem(utils/mediaItem.ts 的适配函数负责板块差异),卡片内不按数据源分支,
// 只按 kind 决定封面比例与占位图标。视觉按 DESIGN.md store-utility-card:
// canvas 底、hairline 细边、--radius-lg、无阴影。
import { computed, onBeforeUnmount, ref } from 'vue'
import { useRouter } from 'vue-router'
import type { MediaItem } from '@/utils/mediaItem'
import { formatCount } from '@/utils/formatCount'
import { formatTimestamp } from '@/utils/formatTime'
import ProxyImage from '@/components/ProxyImage.vue'
import TagBlacklistSheet from '@/components/TagBlacklistSheet.vue'

const props = withDefaults(
  defineProps<{
    item: MediaItem
    /** grid=封面卡片;row=行式(左缩略 + 文字 + 右侧时间/操作) */
    layout?: 'grid' | 'row'
  }>(),
  { layout: 'grid' },
)
const emit = defineEmits<{ open: [item: MediaItem] }>()
defineSlots<{ actions?: (p: { item: MediaItem }) => unknown }>()

const router = useRouter()
const imgError = ref(false)
const TAG_LIMIT = 3

const showCover = computed(() => !!props.item.cover?.src && !imgError.value)
const tags = computed(() => (props.item.tags ?? []).slice(0, TAG_LIMIT))
// 统计项图标化(用户反馈:♥/▶ unicode 退化恢复 SVG 图标形态)
interface StatEntry {
  /** 点赞=心(点赞色) / 阅读·播放=眼睛;收藏也用红心(用户口径),章节数无图标 */
  icon?: 'heart' | 'eye'
  text: string
}
// 图标规则简化(用户口径):点赞红心、收藏红心、阅读眼睛
const STAT_ICON_PATH: Record<NonNullable<StatEntry['icon']>, string> = {
  heart:
    'M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z',
  eye: 'M12 4.5C7 4.5 2.73 7.61 1 12c1.73 4.39 6 7.5 11 7.5s9.27-3.11 11-7.5c-1.73-4.39-6-7.5-11-7.5zM12 17c-2.76 0-5-2.24-5-5s2.24-5 5-5 5 2.24 5 5-2.24 5-5 5zm0-8c-1.66 0-3 1.34-3 3s1.34 3 3 3 3-1.34 3-3-1.34-3-3-3z',
}
const statEntries = computed<StatEntry[]>(() => {
  const s = props.item.stats
  if (!s) return []
  const entries: StatEntry[] = []
  if (s.likes) entries.push({ icon: 'heart', text: formatCount(s.likes) })
  if (s.views) entries.push({ icon: 'eye', text: formatCount(s.views) })
  if (s.favorites) entries.push({ icon: 'heart', text: formatCount(s.favorites) })
  if (s.chapters) entries.push({ text: `${s.chapters} 章` })
  return entries
})
const extraText = computed(() => (props.item.extra ?? []).join(' · '))
const hasStats = computed(() => statEntries.value.length > 0 || !!extraText.value)
const showStats = computed(() => hasStats.value && !(props.layout === 'grid' && props.item.kind === 'video'))
const viewedText = computed(() => (props.item.kind === 'novel' ? '已读' : '已看'))

// —— 点击:默认走应用内路由;有 href 的条目(漫画)保留 <a> 的中键/新窗口能力 ——
const suppressClick = ref(false)
function handleClick(e: MouseEvent) {
  if (suppressClick.value) {
    e.preventDefault()
    return
  }
  // 修饰键/中键交给浏览器原生 <a> 行为
  if (props.item.href && (e.ctrlKey || e.metaKey || e.shiftKey || e.button === 1)) return
  e.preventDefault()
  emit('open', props.item)
  if (props.item.to) void router.push(props.item.to)
}

// —— 漫画长按/右键快速拉黑(仅 blacklistTags 非空的条目启用) ——
const sheetOpen = ref(false)
let pressTimer: ReturnType<typeof setTimeout> | null = null
let suppressTimer: ReturnType<typeof setTimeout> | null = null
let pressStart = { x: 0, y: 0 }
const longPressEnabled = computed(() => !!props.item.blacklistTags?.length)

function clearPress() {
  if (pressTimer) clearTimeout(pressTimer)
  pressTimer = null
}
function suppressNextClick() {
  suppressClick.value = true
  if (suppressTimer) clearTimeout(suppressTimer)
  suppressTimer = setTimeout(() => {
    suppressClick.value = false
    suppressTimer = null
  }, 700)
}
function onPointerDown(e: PointerEvent) {
  if (!longPressEnabled.value) return
  if (e.pointerType === 'mouse' && e.button !== 0) return
  pressStart = { x: e.clientX, y: e.clientY }
  clearPress()
  pressTimer = setTimeout(() => {
    suppressNextClick()
    sheetOpen.value = true
    navigator.vibrate?.(30)
  }, 520)
}
function onPointerMove(e: PointerEvent) {
  if (Math.hypot(e.clientX - pressStart.x, e.clientY - pressStart.y) > 10) clearPress()
}
function onContextMenu(e: MouseEvent) {
  if (!longPressEnabled.value) return
  e.preventDefault()
  clearPress()
  suppressNextClick()
  sheetOpen.value = true
}
onBeforeUnmount(() => {
  clearPress()
  if (suppressTimer) clearTimeout(suppressTimer)
})
</script>

<template>
  <div class="media-card" :class="[`layout-${layout}`, `kind-${item.kind}`]">
    <a
      class="media-hit"
      :href="item.href ?? undefined"
      :target="item.href ? '_blank' : undefined"
      :rel="item.href ? 'noopener noreferrer' : undefined"
      role="link"
      tabindex="0"
      @click="handleClick"
      @keydown.enter.prevent="handleClick($event as unknown as MouseEvent)"
    >
      <div
        class="media-cover"
        @pointerdown="onPointerDown"
        @pointermove="onPointerMove"
        @pointerup="clearPress"
        @pointercancel="clearPress"
        @contextmenu="onContextMenu"
      >
        <ProxyImage
          v-if="showCover"
          :src="item.cover!.src"
          :source="item.cover!.source"
          :alt="item.title"
          class="media-cover-img"
          loading="lazy"
          @error="imgError = true"
        />
        <div v-else class="media-placeholder">
          <svg
            v-if="item.kind === 'novel'"
            class="placeholder-icon"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
            <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
          </svg>
          <svg
            v-else-if="item.kind === 'video'"
            class="placeholder-icon"
            viewBox="0 0 24 24"
            fill="currentColor"
            aria-hidden="true"
          >
            <path d="M8 5v14l11-7z" />
          </svg>
          <span v-if="layout === 'grid'" class="placeholder-title">{{ item.title }}</span>
        </div>
        <span v-if="item.badges?.length && layout === 'grid'" class="media-badges">
          <span
            v-for="b in item.badges"
            :key="b.text"
            class="media-badge"
            :class="`tone-${b.tone}`"
          >{{ b.text }}</span>
        </span>
        <span v-if="item.viewed" class="media-viewed">{{ viewedText }}</span>
        <span
                  v-if="item.duration"
                  class="media-overlay-duration"
                >{{ item.duration }}</span>
        <span v-if="showStats && layout === 'grid' && item.kind === 'video'" class="media-overlay-stat">
          <span v-for="(st, si) in statEntries" :key="si" class="stat-item" :class="st.icon && `stat-${st.icon}`">
            <svg v-if="st.icon" :data-icon="st.icon" width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path :d="STAT_ICON_PATH[st.icon]" /></svg>
            {{ st.text }}
          </span>
          <span v-if="extraText">{{ extraText }}</span>
        </span>
      </div>

      <div class="media-body">
        <span v-if="layout === 'row' && item.badges?.length" class="media-row-badges">
          <span
            v-for="b in item.badges"
            :key="b.text"
            class="media-badge inline"
            :class="`tone-${b.tone}`"
          >{{ b.text }}</span>
        </span>
        <span v-if="item.kind === 'video' && item.subtitle" class="media-code">{{ item.subtitle }}</span>
        <span class="media-title">{{ item.title }}</span>
        <span v-if="item.kind !== 'video' && item.subtitle" class="media-subtitle">{{ item.subtitle }}</span>
        <span v-if="tags.length && layout === 'grid'" class="media-tags">
          <span v-for="t in tags" :key="t" class="media-tag">{{ t }}</span>
        </span>
        <span v-if="item.progress" class="media-progress">{{ item.progress }}</span>
        <span
          v-if="showStats && !(layout === 'grid' && item.kind === 'video')"
          class="media-stats"
        >
          <span v-for="(st, si) in statEntries" :key="si" class="stat-item" :class="st.icon && `stat-${st.icon}`">
            <svg v-if="st.icon" :data-icon="st.icon" width="12" height="12" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path :d="STAT_ICON_PATH[st.icon]" /></svg>
            {{ st.text }}
          </span>
          <span v-if="extraText">{{ extraText }}</span>
        </span>
      </div>

      <span v-if="layout === 'row' && item.time" class="media-time">{{ formatTimestamp(item.time) }}</span>
    </a>
    <div v-if="$slots.actions" class="media-actions">
      <slot name="actions" :item="item" />
    </div>

    <TagBlacklistSheet
      v-if="sheetOpen"
      :title="item.title"
      :tags="item.blacklistTags ?? []"
      @close="sheetOpen = false"
    />
  </div>
</template>

<style scoped>
.media-card {
  position: relative;
  min-width: 0;
  background: var(--color-card-bg);
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-lg);
  overflow: hidden;
  transition: border-color 0.2s;
}

.media-card:hover {
  border-color: var(--color-text-muted);
}

.media-hit {
  display: flex;
  color: inherit;
  text-decoration: none;
  cursor: pointer;
}

.media-hit:focus-visible {
  outline: 2px solid var(--color-primary);
  outline-offset: -2px;
}

/* ───── 封面 ───── */
.media-cover {
  position: relative;
  flex-shrink: 0;
  overflow: hidden;
  background: var(--color-divider-soft);
  touch-action: pan-y;
  user-select: none;
  -webkit-user-select: none;
  -webkit-touch-callout: none;
}

.media-cover-img,
.media-cover :deep(img) {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.media-placeholder {
  position: absolute;
  inset: 0;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-xs);
  padding: var(--spacing-sm);
  color: var(--color-text-muted);
}

.placeholder-icon {
  width: 32%;
  max-width: 44px;
  height: auto;
  opacity: 0.55;
}

.placeholder-title {
  text-align: center;
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  line-height: 1.4;
  color: var(--color-text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 4;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.media-badges {
  position: absolute;
  top: var(--spacing-xs);
  left: var(--spacing-xs);
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-xxs);
  max-width: calc(100% - 56px);
}

.media-badge {
  padding: 1px 7px;
  border-radius: var(--radius-sm);
  font-size: var(--font-size-micro);
  font-weight: var(--font-weight-semibold);
  line-height: 1.6;
  white-space: nowrap;
  color: var(--color-on-primary);
  background: var(--color-surface-black);
}

.media-badge.tone-accent {
  background: var(--color-primary);
}

.media-badge.tone-cs {
  background: var(--color-badge-cs);
}

.media-badge.tone-leak {
  background: var(--color-badge-leak);
}

.media-badge.tone-neutral {
  background: var(--color-scrim);
}

.media-badge.inline.tone-neutral {
  color: var(--color-text-secondary);
  background: var(--color-tag-bg);
}

.media-viewed {
  position: absolute;
  top: var(--spacing-xs);
  right: var(--spacing-xs);
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  background: var(--color-scrim);
  color: var(--color-badge-viewed);
  font-size: var(--font-size-micro);
}

.media-overlay-duration {
  position: absolute;
  right: 6px;
  bottom: 6px;
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  background: rgba(0, 0, 0, 0.65);
  color: #fff;
  font-size: var(--font-size-micro);
  font-variant-numeric: tabular-nums;
}

.media-overlay-stat {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  position: absolute;
  right: var(--spacing-xs);
  bottom: var(--spacing-xs);
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  background: var(--color-scrim);
  color: var(--color-on-primary);
  font-size: var(--font-size-micro);
}

/* ───── 文字区 ───── */
.media-body {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-xxs);
  min-width: 0;
}

.media-code {
  font-size: var(--font-size-caption);
  font-weight: var(--font-weight-semibold);
  color: var(--color-primary);
}

.media-title {
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  line-height: 1.4;
  color: var(--color-text-primary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  word-break: break-word;
}

.kind-video .media-title {
  font-weight: var(--font-weight-regular);
  color: var(--color-text-secondary);
}

.stat-item {
  display: inline-flex;
  align-items: center;
  gap: var(--spacing-xxs);
}

.stat-item svg {
  flex-shrink: 0;
}

/* 点赞心用点赞色(旧漫画卡片口径);视频封面遮罩上保持白色 */
.media-stats .stat-heart svg {
  color: var(--color-like);
}

.media-subtitle,
.media-stats {
  display: flex;
  align-items: center;
  flex-wrap: wrap;
  column-gap: var(--spacing-xs);
  row-gap: 2px;
}

.media-stats,
.media-progress {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.media-progress {
  color: var(--color-primary);
}

.media-tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-xxs);
}

.media-tag {
  padding: 1px 6px;
  border-radius: var(--radius-sm);
  background: var(--color-tag-bg);
  color: var(--color-tag-text);
  font-size: var(--font-size-micro);
  white-space: nowrap;
}

/* ───── 网格布局 ───── */
.layout-grid .media-hit {
  flex-direction: column;
  height: 100%;
}

.layout-grid .media-cover {
  width: 100%;
  aspect-ratio: 5 / 7;
}

.layout-grid.kind-video .media-cover {
  aspect-ratio: 16 / 9;
}

.layout-grid .media-body {
  padding: 10px var(--spacing-sm) var(--spacing-sm);
}

/* ───── 行式布局 ───── */
.layout-row {
  display: flex;
  align-items: center;
}

.layout-row .media-hit {
  flex: 1;
  min-width: 0;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-xs) var(--spacing-sm);
}

.layout-row .media-cover {
  width: 48px;
  aspect-ratio: 5 / 7;
  border-radius: var(--radius-sm);
}

.layout-row.kind-video .media-cover {
  width: 72px;
  aspect-ratio: 16 / 9;
}

.layout-row .placeholder-icon {
  width: 50%;
}

.layout-row .media-body {
  flex: 1;
}

.layout-row .media-title {
  -webkit-line-clamp: 1;
}

.media-row-badges {
  display: flex;
  gap: var(--spacing-xxs);
}

.media-time {
  flex-shrink: 0;
  font-size: var(--font-size-fine-print);
  color: var(--color-text-muted);
}

.media-actions {
  display: flex;
  align-items: center;
  flex-shrink: 0;
  padding-right: var(--spacing-xs);
}

.layout-grid .media-actions {
  position: absolute;
  right: var(--spacing-xs);
  bottom: var(--spacing-xs);
  padding: 0;
}
</style>
