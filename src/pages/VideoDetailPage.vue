<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import Hls from 'hls.js'
import { fetchVideoDetail } from '@/services/api'
import type { VideoDetail } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const route = useRoute()
const router = useRouter()

const detail = ref<VideoDetail | null>(null)
const loading = ref(true)
const error = ref('')
const playing = ref(false)
let videoEl: HTMLVideoElement | null = null
let hls: Hls | null = null

async function load() {
  hls?.destroy()
  hls = null
  loading.value = true
  error.value = ''
  detail.value = null
  try {
    const code = String(route.params.code || '').toLowerCase()
    detail.value = await fetchVideoDetail(code)
    await setupPlayer(detail.value.m3u8)
  } catch (e) {
    error.value = (e as Error).message || '加载失败'
  } finally {
    loading.value = false
  }
}

async function setupPlayer(m3u8: string) {
  await new Promise<void>((resolve) => {
    const check = () => {
      videoEl = document.querySelector('video.video-player')
      if (videoEl) resolve()
      else requestAnimationFrame(check)
    }
    check()
  })
  const el = videoEl!
  el.addEventListener('playing', () => {
    playing.value = true
  })
  el.addEventListener('pause', () => {
    playing.value = false
  })
  el.addEventListener('error', () => {
    if (!error.value) error.value = '播放器无法读取视频流，请检查网络后重试'
  })

  if (Hls.isSupported()) {
    const player = new Hls({
      maxBufferLength: 30,
      manifestLoadingMaxRetry: 2,
      levelLoadingMaxRetry: 2,
      fragLoadingMaxRetry: 2,
      manifestLoadingTimeOut: 20000,
      levelLoadingTimeOut: 20000,
      fragLoadingTimeOut: 20000,
    })
    hls = player
    player.on(Hls.Events.ERROR, (_event, data) => {
      if (!data.fatal) return
      error.value = data.type === Hls.ErrorTypes.NETWORK_ERROR
        ? '视频流连接失败，请检查网络或稍后重试'
        : '视频流解析或解码失败，请稍后重试'
      player.destroy()
      if (hls === player) hls = null
    })
    player.loadSource(m3u8)
    player.attachMedia(el)
  } else if (el.canPlayType('application/vnd.apple.mpegurl')) {
    // 少数 WebView 不支持 Media Source Extensions,尝试系统 HLS 播放器。
    el.src = m3u8
    el.load()
  } else {
    throw new Error('当前设备不支持 HLS 播放')
  }
}

function retry() {
  void load()
}

onMounted(() => {
  void load()
})

onBeforeUnmount(() => {
  hls?.destroy()
  hls = null
})
</script>

<template>
  <div class="video-detail-page">
    <div class="player-section">
      <button class="back-btn" @click="router.back()">‹ 返回</button>
      <div class="player-box">
        <LoadingSpinner v-if="loading" message="获取播放地址..." />
        <video
          v-show="!loading && !error"
          class="video-player"
          controls
          playsinline
          preload="metadata"
        />
        <EmptyState
          v-if="error && !loading"
          :title="'播放失败'"
          :message="error"
        >
          <button class="retry-btn" @click="retry">重试</button>
        </EmptyState>
      </div>
    </div>

    <div v-if="detail" class="meta-section">
      <div class="provider-row">
        <span class="provider-badge" :class="detail.provider">{{ detail.provider === 'missav' ? 'missav 备用线路' : 'fs1 线路' }}</span>
        <span v-if="detail.chinese" class="provider-badge chinese">中文字幕</span>
      </div>
      <h1 class="video-title">{{ detail.title || detail.code.toUpperCase() }}</h1>
      <div class="video-tags">
        <span v-for="tag in detail.tags.slice(0, 30)" :key="tag" class="video-tag">
          {{ tag }}
        </span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.video-detail-page {
  max-width: 960px;
  margin: 0 auto;
  padding-bottom: var(--spacing-xl);
}

.player-section {
  position: relative;
  background: #000;
}

.back-btn {
  position: absolute;
  top: 10px;
  left: 10px;
  z-index: 2;
  padding: 4px 12px;
  border: none;
  border-radius: var(--radius-pill, 999px);
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 14px;
  cursor: pointer;
}

.player-box {
  aspect-ratio: 16 / 9;
  display: flex;
  align-items: center;
  justify-content: center;
}

.video-player {
  width: 100%;
  height: 100%;
  background: #000;
}

.retry-btn {
  margin-top: var(--spacing-sm);
  padding: 8px 24px;
  border: none;
  border-radius: var(--radius-pill, 999px);
  background: var(--color-primary);
  color: #fff;
  cursor: pointer;
}

.meta-section {
  padding: var(--spacing-md) var(--spacing-md) 0;
}

.provider-row {
  margin-bottom: var(--spacing-xs);
}

.provider-badge {
  padding: 2px 10px;
  border-radius: var(--radius-pill, 999px);
  font-size: 11px;
  background: var(--color-divider-soft);
  color: var(--color-text-secondary);
}

.provider-badge.chinese {
  background: #e8590c;
  color: #fff;
}

.provider-badge.missav {
  background: var(--color-primary);
  color: #fff;
}

.video-title {
  font-size: var(--font-size-body);
  font-weight: 600;
  color: var(--color-text-primary);
  margin: 0 0 var(--spacing-sm);
}

.video-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.video-tag {
  padding: 2px 10px;
  border-radius: var(--radius-pill, 999px);
  background: var(--color-divider-soft);
  color: var(--color-text-secondary);
  font-size: 12px;
}
</style>
