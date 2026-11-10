<script setup lang="ts">
import { onMounted } from 'vue'
import MediaCollection from '@/components/MediaCollection.vue'
import { useVideoFeed } from '@/composables/useVideoFeed'

const feed = useVideoFeed()
onMounted(feed.ensureLoaded)
</script>

<template>
  <div class="video-page-shell">
    <div class="container">
      <h2 class="page-title">视频</h2>
      <MediaCollection
        :items="feed.items.value"
        density="video"
        root-margin="400px"
        :loading="feed.loading.value"
        :loading-more="feed.loadingMore.value"
        :has-more="feed.hasNext.value"
        :error="feed.error.value"
        @load-more="feed.loadMore"
        @retry="feed.items.value.length ? feed.loadMore() : feed.reload()"
      />
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
