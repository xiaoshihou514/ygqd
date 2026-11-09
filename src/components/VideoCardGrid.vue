<script setup lang="ts">
// 视频卡片网格(封面 + 番号 + 角标 + 标题),统一搜索与详情页「相似影片」共用;
// 样式与首页 VideoBrowser 的卡片一致
import { useRouter } from 'vue-router'
import { formatCount } from '@/utils/formatCount'
import { videoBaseCode } from '@/utils/videoCode'
import { parseLikes } from '@/utils/likes'
import type { VideoCard } from '@/types'
import ProxyImage from '@/components/ProxyImage.vue'

defineProps<{ items: VideoCard[]; watched?: Set<string> }>()

const router = useRouter()

</script>

<template>
  <div class="video-grid">
    <button
      v-for="card in items"
      :key="card.code"
      type="button"
      class="video-card"
      @click="router.push(`/videos/${card.code}`)"
    >
      <div class="video-thumb-wrap">
        <ProxyImage :src="card.thumbnail" source="video" class="video-thumb" loading="lazy" />
        <span v-if="card.views" class="video-views">▶ {{ formatCount(parseLikes(card.views)) }}</span>
        <span v-if="watched?.has(card.code)" class="video-watched">已看</span>
      </div>
      <div class="video-card-body">
        <span class="video-code-row">
          <span class="video-code">{{ videoBaseCode(card.code) }}</span>
          <span v-if="card.code.includes('-uncensored-leak')" class="video-leak-badge">无码</span>
          <span v-if="card.chinese" class="video-cs-badge">中字</span>
        </span>
        <span class="video-card-title">{{ card.title || videoBaseCode(card.code) }}</span>
      </div>
    </button>
  </div>
</template>

<style scoped>
.video-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: var(--spacing-md);
}

.video-card {
  display: flex;
  flex-direction: column;
  padding: 0;
  border: none;
  background: var(--color-card-bg);
  border-radius: var(--radius-md);
  overflow: hidden;
  cursor: pointer;
  text-align: left;
}

.video-thumb-wrap {
  position: relative;
  aspect-ratio: 16 / 9;
  background: var(--color-divider-soft);
}

.video-thumb {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.video-watched {
  position: absolute;
  left: 6px;
  top: 6px;
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.65);
  color: #9be9a8;
  font-size: 11px;
}

.video-views {
  position: absolute;
  right: 6px;
  bottom: 6px;
  padding: 1px 6px;
  border-radius: 4px;
  background: rgba(0, 0, 0, 0.65);
  color: #fff;
  font-size: 11px;
}

.video-card-body {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 8px 10px;
}

.video-code-row {
  display: flex;
  align-items: center;
  gap: 6px;
}

.video-code {
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-primary);
}

.video-cs-badge {
  padding: 0 5px;
  border-radius: 4px;
  background: #e8590c;
  color: #fff;
  font-size: 10px;
  flex-shrink: 0;
}

.video-leak-badge {
  padding: 0 5px;
  border-radius: 4px;
  background: #6741d9;
  color: #fff;
  font-size: 10px;
  flex-shrink: 0;
}

.video-card-title {
  font-size: 12px;
  color: var(--color-text-secondary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
