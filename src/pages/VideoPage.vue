<script setup lang="ts">
import VideoBrowser from '@/components/VideoBrowser.vue'
</script>

<template>
  <div class="video-page-shell">
    <div class="container">
      <h2 class="page-title">视频</h2>
      <VideoBrowser />
    </div>
  </div>
</template>

<style scoped>
.video-page-shell {
  padding: var(--spacing-md) 0 var(--spacing-xl);
}

.page-title {
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  color: var(--color-text-primary);
  margin-bottom: var(--spacing-md);
}
</style>
