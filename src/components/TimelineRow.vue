<script setup lang="ts">
// 任务 5:三板块统一时间线行组件(用户口径「统一用一个组件显示」)
// 封面位:漫画 ProxyImage/视频构造封面/小说图标占位;
// 进度位:小说「读到第 N 章」/视频「看到 X 分 X 秒」
import { computed } from 'vue'
import ProxyImage from '@/components/ProxyImage.vue'
import type { TimelineEntry } from '@/composables/useTimeline'

const props = defineProps<{ entry: TimelineEntry }>()

const emit = defineEmits<{ open: [entry: TimelineEntry] }>()

const kindLabel = computed(() =>
  props.entry.kind === 'comic' ? '漫画' : props.entry.kind === 'video' ? '视频' : '小说',
)

const progressText = computed(() => {
  const e = props.entry
  try {
    if (e.kind === 'novel' && e.progress) {
      const p = JSON.parse(e.progress) as { chapter?: number; page?: number }
      const parts = [p.chapter ? `读到第 ${p.chapter} 章` : '', p.page ? ` 第 ${p.page} 页` : '']
      return parts.filter(Boolean).join(' ')
    }
    if (e.kind === 'video' && e.progress) {
      const pos = (JSON.parse(e.progress) as { position?: number }).position
      if (typeof pos === 'number' && pos > 0) {
        return `看到 ${Math.floor(pos / 60)} 分 ${Math.floor(pos % 60)} 秒`
      }
    }
  } catch {
    /* progress JSON 损坏忽略 */
  }
  return ''
})

function formatTime(ts: number): string {
  const d = new Date(ts)
  const M = String(d.getMonth() + 1).padStart(2, '0')
  const D = String(d.getDate()).padStart(2, '0')
  const hh = String(d.getHours()).padStart(2, '0')
  const mm = String(d.getMinutes()).padStart(2, '0')
  return `${M}-${D} ${hh}:${mm}`
}
</script>

<template>
  <button type="button" class="timeline-row" @click="emit('open', entry)">
    <span
      v-if="entry.thumbnail && entry.kind === 'video'"
      class="timeline-thumb"
    >
      <ProxyImage :src="entry.thumbnail" source="video" class="timeline-thumb-img" loading="lazy" />
    </span>
    <span
      v-else-if="entry.thumbnail && entry.kind === 'comic'"
      class="timeline-thumb"
    >
      <ProxyImage
        :src="entry.thumbnail"
        :source="entry.source as import('@/config/sources').SourceId"
        class="timeline-thumb-img"
        loading="lazy"
      />
    </span>
    <span v-else class="timeline-thumb placeholder" :class="`ph-${entry.kind}`">
      <svg v-if="entry.kind === 'video'" width="18" height="18" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M8 5v14l11-7z" />
      </svg>
      <svg v-else-if="entry.kind === 'novel'" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
        <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
      </svg>
      <svg v-else width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <rect x="3" y="3" width="18" height="18" rx="2" />
        <circle cx="8.5" cy="8.5" r="1.5" />
        <path d="M21 15l-5-5L5 21" />
      </svg>
    </span>
    <span class="timeline-kind-badge" :class="`kind-${entry.kind}`">{{ kindLabel }}</span>
    <span class="timeline-title">{{ entry.title }}</span>
    <span v-if="progressText" class="timeline-progress">{{ progressText }}</span>
    <span class="timeline-time">{{ formatTime(entry.viewedAt) }}</span>
  </button>
</template>

<style scoped>
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
