<script setup lang="ts">
// 任务 5:时间线列表编排组件(混排/分组视图同一编排)
import TimelineRow from '@/components/TimelineRow.vue'
import EmptyState from '@/components/EmptyState.vue'
import type { TimelineEntry } from '@/composables/useTimeline'

defineProps<{
  entries: TimelineEntry[]
  loading?: boolean
  emptyTitle?: string
  emptyMessage?: string
}>()

const emit = defineEmits<{ open: [entry: TimelineEntry] }>()
</script>

<template>
  <div v-if="$props.loading" class="timeline-loading">
    <slot name="loading">加载中...</slot>
  </div>
  <EmptyState
    v-else-if="!entries.length"
    :title="emptyTitle ?? '暂无内容'"
    :message="emptyMessage ?? ''"
  />
  <div v-else class="timeline-list">
    <TimelineRow v-for="(e, i) in entries" :key="`${e.kind}-${e.itemId}-${i}`" :entry="e" @open="emit('open', $event)" />
    <slot />
  </div>
</template>

<style scoped>
.timeline-list {
  display: flex;
  flex-direction: column;
  gap: var(--spacing-xs);
}

.timeline-loading {
  padding: var(--spacing-md) 0;
  font-size: var(--font-size-caption);
  color: var(--color-text-secondary);
}
</style>
