<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useViewHistory } from '@/composables/useViewHistory'
import ComicGrid from '@/components/ComicGrid.vue'
import EmptyState from '@/components/EmptyState.vue'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

const { history, loading, load } = useViewHistory()

// 历史分组:漫画(现状数据)/视频/小说(批次 4 全局历史表后接入真数据)
type HistoryScope = 'comic' | 'video' | 'novel'
const scope = ref<HistoryScope>('comic')
const SCOPES: Array<{ key: HistoryScope; label: string }> = [
  { key: 'comic', label: '漫画' },
  { key: 'video', label: '视频' },
  { key: 'novel', label: '小说' },
]

onMounted(async () => {
  await load()
})
</script>

<template>
  <div class="history-page">
    <div class="container">
      <h2 class="page-title">浏览历史</h2>

      <div class="scope-bar" role="tablist" aria-label="历史范围">
        <button
          v-for="s in SCOPES"
          :key="s.key"
          type="button"
          class="scope-btn"
          :class="{ active: scope === s.key }"
          role="tab"
          :aria-selected="scope === s.key"
          @click="scope = s.key"
        >
          {{ s.label }}
        </button>
      </div>

      <template v-if="scope === 'comic'">
        <LoadingSpinner v-if="loading" message="加载中..." />

        <EmptyState
          v-else-if="history.length === 0"
          title="还没有浏览记录"
          message="查看漫画详情时会自动记录"
        />

        <ComicGrid
          v-else
          :items="history.map((h) => ({
            source: h.source,
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
      </template>

      <EmptyState
        v-else-if="scope === 'video'"
        title="视频浏览记录即将上线"
        message="全局历史功能正在建设中,上线后视频观看记录会出现在这里"
      />
      <EmptyState
        v-else
        title="小说阅读记录即将上线"
        message="全局历史功能正在建设中,上线后小说阅读进度会出现在这里"
      />

          source: h.source,
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

.scope-bar {
  display: flex;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-md);
}

.scope-btn {
  flex: 1;
  max-width: 120px;
  padding: 6px 0;
  border: 1px solid var(--color-border, var(--color-hairline));
  border-radius: var(--radius-pill, 999px);
  background: transparent;
  color: var(--color-text-secondary);
  font-size: var(--font-size-caption);
  cursor: pointer;
  transition: all 0.15s ease;
}

.scope-btn.active {
  border-color: var(--color-primary);
  background: var(--color-primary);
  color: #fff;
}

.page-title {
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-lg);
  padding-top: var(--spacing-lg);
}
</style>
