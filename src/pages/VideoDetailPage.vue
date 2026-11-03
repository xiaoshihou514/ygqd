<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

import videojs from 'video.js'
import zhCN from 'video.js/dist/lang/zh-CN.json'
import 'video.js/dist/video-js.css'
// 中文界面
videojs.addLanguage('zh-CN', zhCN)
import { fetchVideoDetail, recordVideoWatched } from '@/services/api'
import type { VideoDetail } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'
import EmptyState from '@/components/EmptyState.vue'

const route = useRoute()
const router = useRouter()

const detail = ref<VideoDetail | null>(null)
const loading = ref(true)
const error = ref('')
const recovering = ref(false)
let recoverCount = 0
let player: ReturnType<typeof videojs> | null = null
const playerEl = ref<HTMLDivElement | null>(null)

async function nextFrame(): Promise<void> {
  return new Promise((resolve) => requestAnimationFrame(() => resolve()))
}

async function load() {
  player?.dispose()
  player = null
  loading.value = true
  error.value = ''
  detail.value = null
  try {
    const code = String(route.params.code || '').toLowerCase()
    detail.value = await fetchVideoDetail(code)
    void recordVideoWatched(code)
    await setupPlayer(detail.value.m3u8)
  } catch (e) {
    error.value = (e as Error).message || '加载失败'
  } finally {
    loading.value = false
  }
}

async function setupPlayer(m3u8: string) {
  await nextFrame()
  if (!playerEl.value) throw new Error('播放器容器未就绪')

  // 用 video.js 内置 VHS 引擎拉流(HLS 与 AES-128 原生支持);
  // 源地址是本地流代理(Rust 带站点 Referer 转发上游 CDN)
  // v10 类型导出不完整,断言为本地最小接口
  player = videojs(playerEl.value, {
    controls: true,
    fluid: true,
    preload: 'auto',
    language: 'zh-CN',
    playbackRates: [0.5, 0.75, 1, 1.25, 1.5, 2],
    controlBar: { pictureInPictureToggle: false },
    html5: {
      vhs: { limitRenditionByPlayerDimensions: false, useDevicePixelRatio: true },
    },
  })

  player.on('error', async () => {
    const err = player?.error()
    const msg = err?.message ?? ''
    console.error('[player]', err?.code, msg)
    // 上游 403 多为 HLS 签名过期(短时效,频繁快进后过期):
    // 自动重新获取详情换新签名并重载,最多 2 次
    const upstream403 = err?.code === 4 || msg.includes('403')
    if (upstream403 && recoverCount < 2 && detail.value) {
      recoverCount += 1
      recovering.value = true
      try {
        const playerNow = player
        const fresh = await fetchVideoDetail(detail.value.code)
        detail.value = fresh
        playerNow?.src({ src: fresh.m3u8, type: "application/vnd.apple.mpegurl" })
        void playerNow?.play()?.catch(() => undefined)
        void (playerNow?.play())?.catch(() => undefined)
      } catch {
        error.value = '播放地址已过期且刷新失败,请返回重进'
      } finally {
        recovering.value = false
      }
    } else {
      error.value = `播放失败:${msg || '未知错误'}`
    }
  })
}

function retry() {
  recoverCount = 0
  void load()
}

async function copyError() {
  try {
    await navigator.clipboard.writeText(error.value)
  } catch {
    // 剪贴板不可用时忽略
  }
}

onMounted(() => {
  void load()
})

onBeforeUnmount(() => {
  player?.dispose()
  player = null
})
</script>

<template>
  <div class="video-detail-page">
    <div class="player-section">
      <button class="back-btn" @click="router.back()">‹ 返回</button>
      <div class="player-box">
        <LoadingSpinner v-if="loading" message="获取播放地址..." />
        <EmptyState v-if="error && !loading" title="播放失败" :message="error">
          <button class="retry-btn" @click="retry">重试</button>
          <button class="retry-btn ghost" @click="copyError">复制错误</button>
        </EmptyState>
        <div v-show="!loading && !error" ref="playerEl" />
        <div v-if="recovering" class="recover-overlay">
          <span>播放地址已过期,正在自动刷新...</span>
        </div>
      </div>
    </div>

    <div v-if="detail" class="meta-section">
      <div class="provider-row">
        <span class="provider-badge" :class="detail.provider">{{
          detail.provider === 'missav' ? 'missav 优先线路' : 'fs1 线路'
        }}</span>
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
  z-index: 3;
  padding: 4px 12px;
  border: none;
  border-radius: var(--radius-pill, 999px);
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: 14px;
  cursor: pointer;
}

.player-box {
  position: relative;
  aspect-ratio: 16 / 9;
  display: flex;
  align-items: center;
  justify-content: center;
}

.player-box :deep(.video-js) {
  width: 100%;
  height: 100%;
}

.recover-overlay {
  position: absolute;
  inset: 0;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.6);
  color: #fff;
  font-size: 14px;
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

.retry-btn.ghost {
  background: transparent;
  border: 1px solid var(--color-border, var(--color-hairline));
  color: var(--color-text-primary);
}

.meta-section {
  padding: var(--spacing-md) var(--spacing-md) 0;
}

.provider-row {
  display: flex;
  gap: var(--spacing-xs);
  margin-bottom: var(--spacing-xs);
}

.provider-badge {
  padding: 2px 10px;
  border-radius: var(--radius-pill, 999px);
  font-size: 11px;
  background: var(--color-divider-soft);
  color: var(--color-text-secondary);
}

.provider-badge.missav {
  background: var(--color-primary);
  color: #fff;
}

.provider-badge.chinese {
  background: #e8590c;
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
