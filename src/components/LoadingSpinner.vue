<script setup lang="ts">
defineProps<{
  message?: string
}>()
</script>

<template>
  <div class="loading-spinner" role="status" aria-live="polite">
    <div class="spinner" aria-hidden="true" />
    <p v-if="message" class="spinner-text">{{ message }}</p>
  </div>
</template>

<style scoped>
.loading-spinner {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-xxl) 0;
  /* 延迟显现:短于 --duration-slow 的加载不出现转圈,避免一闪而过 */
  animation: spinner-in var(--duration-base) var(--ease-out) var(--duration-slow) both;
}

@keyframes spinner-in {
  from {
    opacity: 0;
  }
}

.spinner {
  width: 36px;
  height: 36px;
  border: 3px solid var(--color-skeleton);
  border-top-color: var(--color-primary);
  border-radius: var(--radius-full);
  animation: spin calc(var(--duration-slow) * 2.5) linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.spinner-text {
  margin-top: var(--spacing-sm);
  font-size: var(--font-size-caption);
  letter-spacing: var(--letter-spacing-caption);
  color: var(--color-text-secondary);
}

@media (prefers-reduced-motion: reduce) {
  .loading-spinner {
    animation: none;
  }
}
</style>
