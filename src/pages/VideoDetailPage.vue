<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'

// Video.js 10(@videojs/html)自定义元素:导入即注册,无需命令式初始化。
// hlsjs-video 引 hls.js 引擎(MPEG-TS 与 AES-128 全支持),拉本地流代理的 m3u8
import '@videojs/html/video/player'
import '@videojs/html/video/skin'
import '@videojs/html/media/hlsjs-video'
// <media-i18n> 提供界面语言,zh-CN 内置语言包按需懒加载
import '@videojs/html/i18n'
import type { HlsJsAdapter } from '@videojs/hlsjs-video'

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
const mediaEl = ref<HlsJsAdapter | null>(null)

async function load() {
  // detail 置空会让 v-if 卸载播放器元素,等于旧版的 dispose
  detail.value = null
  loading.value = true
  error.value = ''
  try {
    const code = String(route.params.code || '').toLowerCase()
    detail.value = await fetchVideoDetail(code)
    void recordVideoWatched(code)
  } catch (e) {
    error.value = errorMessage(e) || '加载失败'
  } finally {
    loading.value = false
  }
}

/** hls.js 错误附带的原始数据,403 时含 response.code */
interface HlsErrorData {
  response?: { code?: number }
}

function onMediaError() {
  const err = mediaEl.value?.error ?? null
  const msg = err?.message ?? ''
  const hlsData = err ? (err as { data?: HlsErrorData }).data : undefined
  console.error('[player]', err?.code, msg)
  // 上游 403 多为 HLS 签名过期(短时效,频繁快进后过期):
  // 自动重新获取详情换新签名;src 绑定随 detail 更新触发重载,最多 2 次
  const upstream403 = hlsData?.response?.code === 403 || err?.code === 4 || msg.includes('403')
  if (upstream403 && recoverCount < 2 && detail.value) {
    recoverCount += 1
    recovering.value = true
    fetchVideoDetail(detail.value.code)
      .then(async (fresh) => {
        detail.value = fresh
        // 等 Vue 把新签名写入 :src.prop 后再 play,否则元素仍持有旧地址,恢复空转
        await nextTick()
        await mediaEl.value?.play()
      })
      .catch(() => {
        error.value = '播放地址已过期且刷新失败,请返回重进'
      })
      .finally(() => {
        recovering.value = false
      })
  } else {
    error.value = `播放失败:${msg || '未知错误'}`
  }
}

function retry() {
  recoverCount = 0
  void load()
}

// 全屏:Android WebView 的全屏请求被 wry 宿主立即取消
// (RustWebChromeClient.onShowCustomView 首行 onCustomViewHidden),
// Tauri 窗口全屏在 Android 亦为 no-op——故 Android 走页面级伪全屏
// (fixed 铺满 + 横屏锁定尝试),桌面走元素全屏(恢复 v8 时代能力)。
const IS_ANDROID = /android/i.test(navigator.userAgent)
const playerBox = ref<HTMLDivElement | null>(null)
const isFullscreen = ref(false)

function onFullscreenChange() {
  isFullscreen.value = !!document.fullscreenElement
}

async function toggleFullscreen() {
  if (IS_ANDROID) {
    isFullscreen.value = !isFullscreen.value
    try {
      // 伪全屏下尝试横屏锁定(Android WebView 一般可用,失败静默)
      if (isFullscreen.value) {
        await screen.orientation.lock('landscape')
      } else {
        screen.orientation.unlock()
      }
    } catch {
      /* 不支持或被拒,忽略 */
    }
    return
  }
  const box = playerBox.value
  if (!box) return
  try {
    if (!document.fullscreenElement) {
      await box.requestFullscreen()
    } else {
      await document.exitFullscreen()
    }
  } catch {
    /* 请求被拒(如非用户手势),忽略 */
  }
}

async function copyError() {
  try {
    await navigator.clipboard.writeText(error.value)
  } catch {
    // 剪贴板不可用时忽略
  }
}

onMounted(() => {
  document.addEventListener('fullscreenchange', onFullscreenChange)
  void load()
})

onBeforeUnmount(() => {
  document.removeEventListener('fullscreenchange', onFullscreenChange)
  if (IS_ANDROID && isFullscreen.value) screen.orientation.unlock()
  if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined)
})
</script>

<template>
  <div class="video-detail-page">
    <div class="player-section" :class="{ 'player-fullscreen': isFullscreen && IS_ANDROID }">
      <button class="back-btn" @click="router.back()">‹ 返回</button>
      <button
        class="fs-btn"
        :title="isFullscreen ? '退出全屏' : '全屏'"
        :aria-label="isFullscreen ? '退出全屏' : '全屏'"
        @click="toggleFullscreen"
      >
        <svg v-if="!isFullscreen" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3" />
        </svg>
        <svg v-else viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M8 3v3a2 2 0 0 1-2 2H3m18 0h-3a2 2 0 0 1-2-2V3m0 18v-3a2 2 0 0 0 2-2h3M3 8h3a2 2 0 0 0 2-2V3" />
        </svg>
      </button>
      <div ref="playerBox" class="player-box">
        <LoadingSpinner v-if="loading" message="获取播放地址..." />
        <EmptyState v-if="error && !loading" title="播放失败" :message="error">
          <button class="retry-btn" @click="retry">重试</button>
          <button class="retry-btn ghost" @click="copyError">复制错误</button>
        </EmptyState>
        <media-i18n v-if="!loading && !error && detail" lang="zh-CN">
          <video-player>
            <video-skin class="player-skin">
              <hlsjs-video
                ref="mediaEl"
                :src.prop="detail.m3u8"
                preload="auto"
                playsinline
                @error="onMediaError"
              ></hlsjs-video>
            </video-skin>
          </video-player>
        </media-i18n>
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

.fs-btn {
  position: absolute;
  top: 10px;
  right: 10px;
  z-index: 3;
  width: 32px;
  height: 26px;
  border: none;
  border-radius: 8px;
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.fs-btn svg {
  width: 15px;
  height: 15px;
}

/* 桌面元素全屏:播放器铺满视口 */
.player-box:fullscreen {
  width: 100vw;
  height: 100vh;
}

/* Android 伪全屏:播放区 fixed 铺满视口,必须盖住顶栏(z-90)与底栏(z-100)
   ——横屏视口矮,层级不足时底栏会直接压住进度条;保持 16:9 居中
   (aspect-ratio + max-height 由浏览器按比例反推宽度,不拉伸视频) */
.player-section.player-fullscreen {
  position: fixed;
  inset: 0;
  z-index: 200;
  display: flex;
  align-items: center;
  justify-content: center;
}

.player-section.player-fullscreen .player-box {
  width: 100%;
  max-height: 100%;
}

.player-box {
  position: relative;
  aspect-ratio: 16 / 9;
  display: flex;
  align-items: center;
  justify-content: center;
}

/* 自定义元素注册完成前隐藏,避免未样式化内容闪烁 */
video-skin:not(:defined) {
  visibility: hidden;
}

.player-skin {
  display: block;
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
