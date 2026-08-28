<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import type { ComicItem } from '@/types'
import ProxyImage from '@/components/ProxyImage.vue'

const props = defineProps<{
  item: ComicItem
}>()

const router = useRouter()
const TAG_LIMIT = 4
const imgError = ref(false)

function handleClick() {
  const url = `/comic/${props.item.categoryId}/${props.item.id}`
  router.push(url)
}
</script>

<template>
  <a
    :href="`/comic/${item.categoryId}/${item.id}`"
    target="_blank"
    rel="noopener noreferrer"
    class="comic-card"
    @click.prevent="handleClick"
  >
    <div class="card-image">
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
