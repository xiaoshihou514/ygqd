<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { searchComics } from '@/services/api'
import type { ComicItem, SearchParams } from '@/types'
import { parseLikes } from '@/utils/likes'
import SearchForm from '@/components/SearchForm.vue'
import ComicGrid from '@/components/ComicGrid.vue'
import Pagination from '@/components/Pagination.vue'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

const PAGE_SIZE = 20

const items = ref<ComicItem[]>([])
const loading = ref(false)
const error = ref('')
const hasSearched = ref(false)
const lastKeyword = ref('')
const minLikes = ref(0)
const sortOrder = ref('default')
const currentPage = ref(0)

const processedItems = computed(() => {
  let result = items.value
  if (minLikes.value > 0) {
    result = result.filter((item) => parseLikes(item.likes) >= minLikes.value)
  }
  if (sortOrder.value === 'likes_desc') {
    result = [...result].sort((a, b) => parseLikes(b.likes) - parseLikes(a.likes))
  } else if (sortOrder.value === 'likes_asc') {
    result = [...result].sort((a, b) => parseLikes(a.likes) - parseLikes(b.likes))
  }
  return result
})

const totalPages = computed(() => Math.max(1, Math.ceil(processedItems.value.length / PAGE_SIZE)))

const paginatedItems = computed(() => {
  const start = currentPage.value * PAGE_SIZE
  return processedItems.value.slice(start, start + PAGE_SIZE)
})

const paginationInfo = computed(() => ({
  current: currentPage.value,
  total: totalPages.value - 1,
  hasNext: currentPage.value < totalPages.value - 1,
  hasPrev: currentPage.value > 0,
}))

watch([minLikes, sortOrder], () => {
  currentPage.value = 0
})

async function handleSearch(params: SearchParams, likesFilter: number, order: string) {
  loading.value = true
  error.value = ''
  hasSearched.value = true
  lastKeyword.value = params.keyword
  minLikes.value = likesFilter
  sortOrder.value = order
  currentPage.value = 0
  try {
    const result = await searchComics(params)
    items.value = result.items
  } catch (e) {
    error.value = (e as Error).message
    items.value = []
  } finally {
    loading.value = false
  }
}

function handlePageChange(page: number) {
  currentPage.value = page
  window.scrollTo({ top: 0, behavior: 'smooth' })
}

const resultTitle = computed(() => {
  if (!hasSearched.value) return ''
  if (loading.value) return '搜索中...'
  return `"${lastKeyword.value}" 的搜索结果`
})
</script>

<template>
  <div class="search-page">
    <div class="container">
      <SearchForm @search="handleSearch" />

      <div v-if="!hasSearched" class="search-intro">
        <EmptyState
          title="搜索漫画资源"
          message="输入关键词，选择分类和搜索范围开始搜索"
        />
      </div>

      <template v-else>
        <div v-if="loading" class="search-loading">
          <LoadingSpinner message="正在搜索..." />
        </div>

        <div v-else-if="error" class="search-error">
          <EmptyState title="搜索失败" :message="error" />
        </div>

        <div v-else>
          <div class="search-result-header">
            <h2 class="result-title">{{ resultTitle }}</h2>
            <span class="result-count">共 {{ processedItems.length }} 条结果</span>
          </div>

          <ComicGrid v-if="paginatedItems.length > 0" :items="paginatedItems" />

          <Pagination
            v-if="processedItems.length > PAGE_SIZE"
            v-bind="paginationInfo"
            @change="handlePageChange"
          />

          <EmptyState
            v-else-if="processedItems.length === 0"
            title="未找到相关内容"
            message="请尝试更换关键词或调整搜索范围"
          />
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.search-page {
  padding-bottom: var(--spacing-xl);
}

.search-intro {
  padding-top: var(--spacing-xl);
}

.search-loading {
  padding-top: var(--spacing-xl);
}

.search-error {
  padding-top: var(--spacing-xl);
}

.search-result-header {
  display: flex;
  align-items: baseline;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-md);
  padding-bottom: var(--spacing-sm);
  border-bottom: 1px solid var(--color-hairline);
}

.result-title {
  font-size: var(--font-size-tagline);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  letter-spacing: -0.231px;
}

.result-count {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-muted);
}
</style>
