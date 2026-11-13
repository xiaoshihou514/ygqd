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
  padding-bottom: var(--spacing-xl);
}

/* 与历史/关注等列表页标题同一口径 */
.page-title {
  padding-top: var(--spacing-lg);
  margin-bottom: var(--spacing-lg);
  font-size: var(--font-size-headline-title);
  font-weight: var(--font-weight-bold);
  line-height: var(--line-height-heading);
  color: var(--color-text-primary);
}
</style>
