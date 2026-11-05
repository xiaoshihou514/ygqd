<script setup lang="ts">
import ProxyImage from '@/components/ProxyImage.vue'
import type { NovelSeries } from '@/types'

defineProps<{
  series: NovelSeries
  /** 阅读进度 [章节序, 页码],无记录为 null */
  progress?: [number, number] | null
}>()

defineEmits<{ open: [] }>()
</script>

<template>
  <button class="series-card" @click="$emit('open')">
    <div class="series-cover">
      <!-- hlib 不提供系列封面,cover 恒为空串:占位用书本图标 + 标题 -->
      <ProxyImage
        v-if="series.cover"
        :src="series.cover"
        class="series-cover-img"
        loading="lazy"
      />
      <div v-else class="series-cover-fallback">
        <svg
          class="fallback-icon"
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
        <span class="fallback-title">{{ series.title }}</span>
      </div>
      <span v-if="progress" class="series-progress-badge">
        读到 {{ progress[0] }} 章
      </span>
    </div>
    <div class="series-body">
      <span class="series-title">{{ series.title }}</span>
      <span class="series-author">{{ series.author || '未知作者' }}</span>
      <span class="series-chapters">{{ series.chapters.length }} 章</span>
    </div>
  </button>
</template>

<style scoped>
.series-card {
  display: flex;
  flex-direction: column;
  padding: 0;
  border: 1px solid var(--color-hairline);
  border-radius: var(--radius-md);
  background: var(--color-card-bg);
  overflow: hidden;
  cursor: pointer;
  text-align: left;
  transition: border-color 0.15s;
}

.series-card:hover {
  border-color: var(--color-primary);
}

.series-cover {
  position: relative;
  aspect-ratio: 3 / 4;
  background: var(--color-divider-soft);
}

.series-cover-img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.series-cover-fallback {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 10px;
  color: var(--color-text-muted);
}

.fallback-icon {
  width: 28px;
  height: 28px;
  opacity: 0.5;
}

.fallback-title {
  font-size: 11px;
  line-height: 1.3;
  color: var(--color-text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 3;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.series-progress-badge {
  position: absolute;
  left: 6px;
  top: 6px;
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.65);
  color: #9be9a8;
  font-size: 11px;
}

.series-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 10px;
}

.series-title {
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.series-author {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.series-chapters {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-muted);
}
</style>
