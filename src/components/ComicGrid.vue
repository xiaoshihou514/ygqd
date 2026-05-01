<script setup lang="ts">
import type { ComicItem } from '@/types'
import ComicCard from './ComicCard.vue'

defineProps<{
  items: ComicItem[]
  loading?: boolean
}>()
</script>

<template>
  <div class="comic-grid">
    <template v-if="loading">
      <div v-for="n in 12" :key="n" class="skeleton-card">
        <div class="skeleton-image" />
        <div class="skeleton-body">
          <div class="skeleton-line w-80" />
          <div class="skeleton-line w-60" />
        </div>
      </div>
    </template>
    <template v-else>
      <ComicCard
        v-for="item in items"
        :key="item.id + item.categoryId"
        :item="item"
      />
    </template>
  </div>
</template>

<style scoped>
.comic-grid {
  display: grid;
  grid-template-columns: repeat(6, 1fr);
  gap: 16px;
}

@media (max-width: 1200px) {
  .comic-grid {
    grid-template-columns: repeat(5, 1fr);
  }
}

@media (max-width: 992px) {
  .comic-grid {
    grid-template-columns: repeat(4, 1fr);
  }
}

@media (max-width: 768px) {
  .comic-grid {
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }
}

@media (max-width: 480px) {
  .comic-grid {
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }
}

.skeleton-card {
  background: var(--color-card-bg);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.skeleton-image {
  width: 100%;
  padding-top: 140%;
  background: linear-gradient(90deg, var(--color-divider-soft) 25%, var(--color-canvas) 50%, var(--color-divider-soft) 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
}

.skeleton-body {
  padding: var(--spacing-sm);
}

.skeleton-line {
  height: 12px;
  background: linear-gradient(90deg, var(--color-divider-soft) 25%, var(--color-canvas) 50%, var(--color-divider-soft) 75%);
  background-size: 200% 100%;
  animation: shimmer 1.5s infinite;
  border-radius: var(--radius-sm);
  margin-bottom: var(--spacing-xs);
}

.skeleton-line.w-80 { width: 80%; }
.skeleton-line.w-60 { width: 60%; }

@keyframes shimmer {
  0% { background-position: -200% 0; }
  100% { background-position: 200% 0; }
}
</style>
