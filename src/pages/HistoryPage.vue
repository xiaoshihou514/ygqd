<script setup lang="ts">
import { onMounted } from 'vue'
import { useViewHistory } from '@/composables/useViewHistory'
import ComicGrid from '@/components/ComicGrid.vue'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

const { history, loading, load } = useViewHistory()

onMounted(async () => {
  await load()
})
</script>

<template>
  <div class="history-page">
    <div class="container">
      <h2 class="page-title">浏览历史</h2>

      <LoadingSpinner v-if="loading" message="加载中..." />

      <EmptyState
        v-else-if="history.length === 0"
        title="还没有浏览记录"
        message="查看漫画详情时会自动记录"
      />

      <ComicGrid
        v-else
        :items="history.map((h) => ({
          id: h.comicId,
          title: h.title,
          thumbnail: h.thumbnail,
          category: '',
          categoryId: h.categoryId,
          tags: [],
          likes: '',
          link: '',
        }))"
      />
    </div>
  </div>
</template>

<style scoped>
.history-page {
  padding-bottom: var(--spacing-xl);
}

.page-title {
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-lg);
  padding-top: var(--spacing-lg);
}
</style>
