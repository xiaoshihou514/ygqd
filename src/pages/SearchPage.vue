<script setup lang="ts">
import { ref, computed } from 'vue'
import { searchComics } from '@/services/api'
import type { ComicItem, SearchParams } from '@/types'
import SearchForm from '@/components/SearchForm.vue'
import ComicGrid from '@/components/ComicGrid.vue'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

const items = ref<ComicItem[]>([])
const loading = ref(false)
const error = ref('')
const hasSearched = ref(false)
const lastKeyword = ref('')

async function handleSearch(params: SearchParams) {
  loading.value = true
  error.value = ''
  hasSearched.value = true
  lastKeyword.value = params.keyword
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
            <span class="result-count">共 {{ items.length }} 条结果</span>
          </div>

          <ComicGrid v-if="items.length > 0" :items="items" />

          <EmptyState
            v-else
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
