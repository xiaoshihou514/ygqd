<script setup lang="ts">
defineProps<{
  message?: string
}>()
</script>

<template>
  <div class="loading-spinner">
    <div class="spinner" />
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
}

.spinner {
  width: 36px;
  height: 36px;
  border: 3px solid var(--color-divider-soft);
  border-top-color: var(--color-primary);
  border-radius: var(--radius-full);
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.spinner-text {
  margin-top: var(--spacing-sm);
  font-size: var(--font-size-caption);
  color: var(--color-text-secondary);
}
</style>
