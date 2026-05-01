<script setup lang="ts">
const props = defineProps<{
  current: number
  total: number
  hasNext: boolean
  hasPrev: boolean
}>()

const emit = defineEmits<{
  change: [page: number]
}>()

const pages = computed(() => {
  const result: (number | string)[] = []
  const start = Math.max(0, props.current - 2)
  const end = Math.min(props.total, start + 5)

  for (let i = start; i <= end; i++) {
    result.push(i)
  }
  return result
})

function goTo(page: number) {
  if (page >= 0 && page <= props.total) {
    emit('change', page)
  }
}
</script>

<template>
  <div v-if="total > 0" class="pagination">
    <button
      class="page-btn"
      :disabled="!hasPrev"
      @click="goTo(current - 1)"
    >
      上一页
    </button>

    <template v-for="page in pages" :key="page">
      <span v-if="typeof page === 'string'" class="page-ellipsis">...</span>
      <button
        v-else
        class="page-btn"
        :class="{ active: page === current }"
        @click="goTo(page)"
      >
        {{ page + 1 }}
      </button>
    </template>

    <button
      class="page-btn"
      :disabled="!hasNext"
      @click="goTo(current + 1)"
    >
      下一页
    </button>
  </div>
</template>

<script lang="ts">
import { computed } from 'vue'
export default { inheritAttrs: false }
</script>

<style scoped>
.pagination {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 24px 0;
}

.page-btn {
  min-width: 36px;
  height: 36px;
  padding: 0 10px;
  border-radius: var(--radius-sm);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-semibold);
  color: var(--color-text-primary);
  background: var(--color-card-bg);
  border: 1px solid var(--color-border);
  transition: all 0.2s;
}

.page-btn:hover:not(:disabled) {
  border-color: var(--color-primary);
  color: var(--color-primary);
}

.page-btn.active {
  background: var(--color-primary);
  color: var(--color-on-primary);
  border-color: var(--color-primary);
}

.page-btn:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.page-ellipsis {
  padding: 0 4px;
  color: var(--color-text-muted);
}
</style>
