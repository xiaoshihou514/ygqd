<script setup lang="ts">
defineProps<{
  title?: string
  message?: string
}>()
</script>

<template>
  <div class="empty-state">
    <div class="empty-icon">
      <svg width="48" height="48" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">
        <circle cx="11" cy="11" r="8" />
        <path d="M21 21l-4.35-4.35" />
        <path d="M8 11h6" />
      </svg>
    </div>
    <h3 v-if="title" class="empty-title">{{ title }}</h3>
    <p v-if="message" class="empty-message">{{ message }}</p>
    <slot />
  </div>
</template>

<style scoped>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-xxl) var(--spacing-md);
  text-align: center;
  animation: empty-in var(--duration-slow) var(--ease-out);
}

/* 轻入场:空态/错误态替换骨架时不硬切 */
@keyframes empty-in {
  from {
    opacity: 0;
    transform: translateY(var(--spacing-xs));
  }
}

.empty-icon {
  color: var(--color-text-muted);
  margin-bottom: var(--spacing-md);
  opacity: 0.5;
}

.empty-title {
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  letter-spacing: var(--letter-spacing-body);
  color: var(--color-text-secondary);
  margin-bottom: var(--spacing-xs);
}

.empty-message {
  max-width: 32em;
  font-size: var(--font-size-caption);
  line-height: var(--line-height-body);
  letter-spacing: var(--letter-spacing-caption);
  color: var(--color-text-muted);
  overflow-wrap: anywhere;
}

@media (prefers-reduced-motion: reduce) {
  .empty-state {
    animation: none;
  }
}
</style>
