<script setup lang="ts">
import { onBeforeUnmount, ref } from 'vue'
import { useRouter } from 'vue-router'
import type { ComicItem } from '@/types'
import ProxyImage from '@/components/ProxyImage.vue'
import { useTagBlacklist } from '@/composables/useTagBlacklist'

defineOptions({ name: 'ComicCard' })

const props = defineProps<{
  item: ComicItem
}>()

const router = useRouter()
const TAG_LIMIT = 4
const imgError = ref(false)
const blacklistOpen = ref(false)
const suppressClick = ref(false)
const { add, has } = useTagBlacklist()
let pressTimer: ReturnType<typeof setTimeout> | null = null
let suppressTimer: ReturnType<typeof setTimeout> | null = null
let pressStart = { x: 0, y: 0 }

function clearPressTimer() {
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

function startLongPress(event: PointerEvent) {
  if (event.pointerType === 'mouse' && event.button !== 0) return
  pressStart = { x: event.clientX, y: event.clientY }
  clearPressTimer()
  pressTimer = setTimeout(() => {
    suppressNextClick()
    blacklistOpen.value = true
    navigator.vibrate?.(30)
  }, 520)
}

function trackLongPress(event: PointerEvent) {
  if (Math.hypot(event.clientX - pressStart.x, event.clientY - pressStart.y) > 10) {
    clearPressTimer()
  }
}

function openBlacklistMenu() {
  clearPressTimer()
  suppressNextClick()
  blacklistOpen.value = true
}

function blacklist(tag: string) {
  add(tag)
}

function handleClick() {
  if (suppressClick.value) {
    suppressClick.value = false
    return
  }
  const url = `/comic/${props.item.categoryId}/${props.item.id}`
  router.push(url)
}

onBeforeUnmount(() => {
  clearPressTimer()
  if (suppressTimer) clearTimeout(suppressTimer)
})
</script>

<template>
  <a
    :href="`/comic/${item.categoryId}/${item.id}`"
    target="_blank"
    rel="noopener noreferrer"
    class="comic-card"
    @click.prevent="handleClick"
  >
    <div
      class="card-image"
      @pointerdown="startLongPress"
      @pointermove="trackLongPress"
      @pointerup="clearPressTimer"
      @pointercancel="clearPressTimer"
      @contextmenu.prevent="openBlacklistMenu"
    >
      <ProxyImage
        v-if="item.thumbnail && !imgError"
        :src="item.thumbnail"
        :alt="item.title"
        loading="lazy"
        @error="imgError = true"
      />
      <div v-if="!item.thumbnail || imgError" class="card-placeholder">
        <span class="placeholder-title">{{ item.title }}</span>
      </div>
      <span class="card-badge">{{ item.category }}</span>
    </div>
    <div class="card-body">
      <h3 class="card-title">{{ item.title }}</h3>
      <div class="card-tags">
        <span
          v-for="(tag, i) in item.tags.slice(0, TAG_LIMIT)"
          :key="i"
          class="card-tag"
        >
          {{ tag }}
        </span>
      </div>
      <div class="card-meta">
        <span class="card-likes">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="var(--color-like)">
            <path d="M12 21.35l-1.45-1.32C5.4 15.36 2 12.28 2 8.5 2 5.42 4.42 3 7.5 3c1.74 0 3.41.81 4.5 2.09C13.09 3.81 14.76 3 16.5 3 19.58 3 22 5.42 22 8.5c0 3.78-3.4 6.86-8.55 11.54L12 21.35z" />
          </svg>
          {{ item.likes }}
        </span>
      </div>
    </div>
    <Teleport to="body">
      <div v-if="blacklistOpen" class="blacklist-backdrop" @click.stop="blacklistOpen = false">
        <section class="blacklist-sheet" role="dialog" aria-modal="true" aria-label="快速拉黑" @click.stop>
          <div class="sheet-handle" />
          <p class="sheet-eyebrow">快速拉黑</p>
          <h2 class="sheet-title">{{ item.title }}</h2>
          <p class="sheet-hint">选择不想再看到的标签</p>
          <div v-if="item.tags.length" class="sheet-tags">
            <button
              v-for="tag in item.tags"
              :key="tag"
              class="sheet-tag"
              :class="{ added: has(tag) }"
              :disabled="has(tag)"
              @click="blacklist(tag)"
            >
              {{ has(tag) ? `已拉黑 · ${tag}` : tag }}
            </button>
          </div>
          <p v-else class="sheet-empty">这个条目没有可用标签</p>
          <button class="sheet-close" @click="blacklistOpen = false">完成</button>
        </section>
      </div>
    </Teleport>
  </a>
</template>

<style scoped>
.comic-card {
  display: block;
  text-decoration: none;
  color: inherit;
  background: var(--color-card-bg);
  border-radius: var(--radius-lg);
  overflow: hidden;
  box-shadow: var(--shadow-card);
  transition: transform 0.2s, box-shadow 0.2s;
}

.comic-card:hover {
  transform: translateY(-2px);
  box-shadow: var(--shadow-card-hover);
}

.card-image {
  position: relative;
  width: 100%;
  padding-top: 140%;
  overflow: hidden;
  background: var(--color-divider-soft);
  touch-action: pan-y;
  user-select: none;
  -webkit-user-select: none;
  -webkit-touch-callout: none;
}

.blacklist-backdrop {
  position: fixed;
  inset: 0;
  z-index: 300;
  display: flex;
  align-items: flex-end;
  justify-content: center;
  padding: var(--spacing-md);
  background: rgba(0, 0, 0, 0.34);
  backdrop-filter: blur(4px);
}

.blacklist-sheet {
  width: min(100%, 520px);
  max-height: min(70vh, 560px);
  overflow-y: auto;
  padding: 10px var(--spacing-lg) calc(var(--spacing-lg) + env(safe-area-inset-bottom));
  border-radius: var(--radius-lg);
  background: var(--color-card-bg);
  box-shadow: 0 18px 60px rgba(0, 0, 0, 0.24);
  animation: sheet-in 180ms ease-out;
}

.sheet-handle {
  width: 36px;
  height: 4px;
  margin: 0 auto var(--spacing-md);
  border-radius: var(--radius-pill);
  background: var(--color-hairline);
}

.sheet-eyebrow {
  color: var(--color-primary);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
}

.sheet-title {
  margin-top: 4px;
  color: var(--color-text-primary);
  font-size: var(--font-size-body);
  line-height: 1.35;
}

.sheet-hint,
.sheet-empty {
  margin-top: var(--spacing-xs);
  color: var(--color-text-muted);
  font-size: var(--font-size-caption);
}

.sheet-tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-xs);
  margin-top: var(--spacing-md);
}

.sheet-tag {
  padding: 8px 13px;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-pill);
  color: var(--color-text-primary);
  background: var(--color-canvas);
  font-size: var(--font-size-caption);
}

.sheet-tag.added {
  color: var(--color-text-muted);
  background: var(--color-divider-soft);
}

.sheet-close {
  width: 100%;
  margin-top: var(--spacing-lg);
  padding: 11px 16px;
  border-radius: var(--radius-pill);
  color: var(--color-on-primary);
  background: var(--color-primary);
  font-size: var(--font-size-body);
}

@keyframes sheet-in {
  from { transform: translateY(20px); opacity: 0; }
  to { transform: translateY(0); opacity: 1; }
}

@media (prefers-reduced-motion: reduce) {
  .blacklist-sheet { animation: none; }
}

.card-image img {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.card-placeholder {
  position: absolute;
  top: 0;
  left: 0;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-sm);
  color: var(--color-text-muted);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
}

.placeholder-title {
  text-align: center;
  display: -webkit-box;
  -webkit-line-clamp: 4;
  -webkit-box-orient: vertical;
  overflow: hidden;
  line-height: 1.4;
  color: var(--color-text-secondary);
}

.card-badge {
  position: absolute;
  top: var(--spacing-xs);
  left: var(--spacing-xs);
  padding: 2px 8px;
  background: var(--color-surface-black);
  color: var(--color-body-on-dark);
  font-size: 11px;
  font-weight: var(--font-weight-semibold);
  border-radius: var(--radius-sm);
  letter-spacing: 0.5px;
}

.card-body {
  padding: 10px var(--spacing-sm) var(--spacing-sm);
}

.card-title {
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  line-height: 1.4;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  margin-bottom: 6px;
  color: var(--color-text-primary);
}

.card-tags {
  display: flex;
  flex-wrap: wrap;
  gap: var(--spacing-xxs);
  margin-bottom: var(--spacing-xs);
}

.card-tag {
  padding: 1px 6px;
  background: var(--color-tag-bg);
  color: var(--color-tag-text);
  font-size: 11px;
  border-radius: var(--radius-sm);
}

.card-meta {
  display: flex;
  align-items: center;
}

.card-likes {
  display: flex;
  align-items: center;
  gap: var(--spacing-xxs);
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
}
</style>
