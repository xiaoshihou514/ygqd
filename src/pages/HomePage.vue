<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { fetchCategoryList } from '@/services/api'
import type { ComicItem } from '@/types'
import { parseLikes } from '@/utils/likes'
import ComicGrid from '@/components/ComicGrid.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const items = ref<ComicItem[]>([])
const page = ref(0)
const loading = ref(true)
const loadingMore = ref(false)
const error = ref('')
const hasNext = ref(true)
const minLikes = ref(0)

const filteredItems = computed(() => {
  if (minLikes.value <= 0) return items.value
  return items.value.filter((item) => parseLikes(item.likes) >= minLikes.value)
})

let sentinel: HTMLElement | null = null
let observer: IntersectionObserver | null = null

async function loadFirstPage() {
  loading.value = true
  error.value = ''
  try {
    const result = await fetchCategoryList(9, 0)
    items.value = result.items
    hasNext.value = result.pagination.hasNext
    page.value = 0
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    loading.value = false
  }
}

async function loadNextPage() {
  if (loadingMore.value || !hasNext.value || loading.value) return
  loadingMore.value = true
  try {
    const nextPage = page.value + 1
    const result = await fetchCategoryList(9, nextPage)
    items.value.push(...result.items)
    hasNext.value = result.pagination.hasNext
    page.value = nextPage
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    loadingMore.value = false
  }
}

function setupObserver() {
  sentinel = document.querySelector('.scroll-sentinel')
  if (!sentinel) return

  observer = new IntersectionObserver((entries) => {
    if (entries[0]?.isIntersecting) {
      loadNextPage()
    }
  }, { rootMargin: '200px' })

  observer.observe(sentinel)
}

function retry() {
  loading.value = true
  error.value = ''
  loadFirstPage()
}

onMounted(() => {
  loadFirstPage().then(() => {
    setupObserver()
  })
})

onUnmounted(() => {
  if (observer) {
    observer.disconnect()
    observer = null
  }
})
</script>

<template>
  <div class="home-page">
    <div class="container">
      <div v-if="loading" class="home-loading">
        <LoadingSpinner message="正在加载漫画列表..." />
      </div>

      <div v-else-if="error && items.length === 0" class="home-error">
        <EmptyState title="加载失败" :message="error">
          <button class="retry-btn" @click="retry">
            重新加载
          </button>
        </EmptyState>
      </div>

      <template v-else>
        <div class="page-header">
          <h1 class="page-title">漫画列表</h1>
          <span class="page-count">共 {{ filteredItems.length }} 部</span>
          <div class="filter-group">
            <input
              v-model.number="minLikes"
              type="number"
              class="filter-input"
              placeholder="最低星标数"
              min="0"
            />
          </div>
        </div>

        <ComicGrid :items="filteredItems" />

        <div v-if="loadingMore" class="loading-more">
          <LoadingSpinner message="正在加载更多..." />
        </div>

        <div
          v-if="hasNext"
          class="scroll-sentinel"
        />

        <div v-if="!hasNext && items.length > 0" class="no-more">
          <span>已加载全部漫画</span>
        </div>

        <div v-if="error && items.length > 0" class="load-error">
          <span>{{ error }}</span>
          <button class="retry-btn" @click="loadNextPage">
            重试
          </button>
        </div>

        <div v-if="items.length === 0" class="home-empty">
          <EmptyState title="暂无内容" message="未能获取到漫画内容" />
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.home-page {
  padding-bottom: var(--spacing-xl);
}

.home-loading {
  padding-top: var(--spacing-xxl);
}

.home-error,
.home-empty {
  padding-top: var(--spacing-xxl);
}

.page-header {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-lg);
  flex-wrap: wrap;
}

.page-title {
  font-size: var(--font-size-hint);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
}

.page-count {
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}

.loading-more {
  padding: var(--spacing-lg) 0;
}

.scroll-sentinel {
  height: 1px;
}

.no-more {
  text-align: center;
  padding: var(--spacing-lg) 0;
  font-size: var(--font-size-caption);
  color: var(--color-text-muted);
}

.load-error {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-md) 0;
  font-size: var(--font-size-caption);
  color: var(--color-error);
}

.retry-btn {
  padding: var(--spacing-xs) var(--spacing-lg);
  background: var(--color-primary);
  color: var(--color-on-primary);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-regular);
  transition: background 0.2s;
}

.retry-btn:hover {
  background: var(--color-primary-focus);
}

.filter-group {
  margin-left: auto;
}

.filter-input {
  height: 36px;
  width: 140px;
  padding: 0 var(--spacing-md);
  border: 1px solid rgba(0, 0, 0, 0.08);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-fine-print);
  outline: none;
  transition: border-color 0.2s;
  color: var(--color-text-primary);
  background: var(--color-canvas);
}

.filter-input:focus {
  border-color: var(--color-primary);
}

.filter-input::placeholder {
  color: var(--color-text-muted);
}
</style>
