<script setup lang="ts">
import { errorMessage } from '@/utils/errors'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'

// Video.js 10(@videojs/html)自定义元素:导入即注册,无需命令式初始化。
// hlsjs-video 引 hls.js 引擎(MPEG-TS 与 AES-128 全支持),拉本地流代理的 m3u8
import '@videojs/html/video/player'
import '@videojs/html/video/skin'
import '@videojs/html/media/hlsjs-video'
// <media-i18n> 提供界面语言,zh-CN 内置语言包按需懒加载
import '@videojs/html/i18n'
import type { HlsJsAdapter } from '@videojs/hlsjs-video'

import { fetchVideoDetail, recordVideoWatched, recordItemHistory, videoFollowWorks, videoFollow } from '@/services/api'
import type { VideoCard, VideoDetail, VideoMetaLink } from '@/types'
import VideoCardGrid from '@/components/VideoCardGrid.vue'
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
  relatedTarget.value = null
  relatedWorks.value = []
  relatedError.value = ''
  followNotice.value = ''
  try {
    const code = String(route.params.code || '').toLowerCase()
    detail.value = await fetchVideoDetail(code)
    void recordVideoWatched(code)
    // 相似影片:默认按厂商(无厂商取系列,再取首个女优)
    const d = detail.value
    const first = d.makers?.[0] ?? d.series?.[0] ?? d.actresses?.[0]
    if (first) void selectMeta(first)
    // 全局历史时间线:播放开始记录基础条目(位置在暂停/离开时补记)
    void recordItemHistory(detail.value.provider || 'fs1', 'video', detail.value.code, detail.value.title || detail.value.code)
  } catch (e) {
    error.value = errorMessage(e) || '加载失败'
  } finally {
    loading.value = false
  }
}

/** 全局历史:暂停/离开时补记播放位置(秒) */
async function recordProgressSnapshot() {
  const d = detail.value
  if (!d) return
  const pos = (mediaEl.value as unknown as { currentTime?: number } | null)?.currentTime
  if (typeof pos !== 'number' || pos <= 0) return
  try {
    await recordItemHistory(d.provider || 'fs1', 'video', d.code, d.title || d.code, JSON.stringify({ position: Math.floor(pos) }))
  } catch {
    /* 历史记录失败不影响播放 */
  }
}

function onPlayerPause() {
  void recordProgressSnapshot()
}

// —— 元数据(厂商/女优/男优/系列/导演/标签)与相似影片 ——
// 点任一条目即在下方列出该条目的作品(missav 列表页,复用 video_follow_works 通道);
// 厂商/女优可一键关注(关注列表可见)
const META_ROWS: Array<{ key: keyof VideoDetail; label: string }> = [
  { key: 'makers', label: '发行商' },
  { key: 'actresses', label: '女优' },
  { key: 'maleActors', label: '男优' },
  { key: 'series', label: '系列' },
  { key: 'directors', label: '导演' },
  { key: 'labels', label: '厂牌' },
]
const metaRows = computed(() => {
  const d = detail.value
  if (!d) return []
  return META_ROWS.map((r) => ({ label: r.label, links: (d[r.key] as VideoMetaLink[] | undefined) ?? [] }))
    .filter((r) => r.links.length > 0)
})

const relatedTarget = ref<VideoMetaLink | null>(null)
const relatedWorks = ref<VideoCard[]>([])
const relatedPage = ref(1)
const relatedHasNext = ref(false)
const relatedLoading = ref(false)
const relatedError = ref('')
const followNotice = ref('')
let relatedGen = 0

function sameBase(a: string, b: string): boolean {
  const base = (c: string) => c.replace('-chinese-subtitle', '')
  return base(a) === base(b)
}

async function loadRelated(page: number) {
  const target = relatedTarget.value
  const d = detail.value
  if (!target || !d) return
  const gen = ++relatedGen
  relatedLoading.value = true
  relatedError.value = ''
  try {
    const res = await videoFollowWorks('missav', target.kind, target.id, page)
    if (gen !== relatedGen) return
    const fresh = res.items.filter((v) => !sameBase(v.code, d.code))
    if (page === 1) {
      relatedWorks.value = fresh
    } else {
      const seen = new Set(relatedWorks.value.map((v) => v.code))
      relatedWorks.value.push(...fresh.filter((v) => !seen.has(v.code)))
    }
    relatedPage.value = page
    relatedHasNext.value = res.pagination.hasNext
  } catch (e) {
    if (gen !== relatedGen) return
    relatedError.value = errorMessage(e) || '相似影片加载失败'
  } finally {
    if (gen === relatedGen) relatedLoading.value = false
  }
}

function selectMeta(link: VideoMetaLink) {
  relatedTarget.value = link
  relatedWorks.value = []
  relatedHasNext.value = false
  followNotice.value = ''
  void loadRelated(1)
}

function isSelected(link: VideoMetaLink): boolean {
  const t = relatedTarget.value
  return !!t && t.kind === link.kind && t.id === link.id
}

const canFollow = computed(() => {
  const k = relatedTarget.value?.kind
  return k === 'maker' || k === 'actress'
})

/** 关注当前选中的厂商/女优(关注列表可见) */
async function followSelected() {
  const t = relatedTarget.value
  if (!t || !canFollow.value) return
  try {
    await videoFollow('missav', t.kind, t.id, t.name)
    followNotice.value = `已关注「${t.name}」`
  } catch (e) {
    relatedError.value = errorMessage(e) || '关注失败,请重试'
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

// ===== 任务 1:横屏自动全屏/竖屏自动退出(旋转传感器驱动) =====
// Android:伪全屏 class 随横竖屏自动切换(无手动按钮);桌面:保留手动按钮
const landscapeQuery = window.matchMedia('(orientation: landscape)')

function onOrientationChange() {
  if (!IS_ANDROID) return
  isFullscreen.value = landscapeQuery.matches
}

async function toggleFullscreen() {
  if (IS_ANDROID) return
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

// ===== 任务 7:快进快退(±1 分钟) =====
function skipBy(seconds: number) {
  const el = mediaEl.value as unknown as { currentTime?: number } | null
  if (!el || typeof el.currentTime !== 'number') return
  el.currentTime = Math.max(0, el.currentTime + seconds)
}

// ===== 任务 8:倍速档(0.5/1/2 循环) =====
const PLAYBACK_RATES = [0.5, 1, 2] as const
const rateIndex = ref(1)
const currentRate = computed(() => PLAYBACK_RATES[rateIndex.value]!)

function cyclePlaybackRate() {
  const el = mediaEl.value as unknown as { playbackRate?: number } | null
  if (!el) return
  rateIndex.value = (rateIndex.value + 1) % PLAYBACK_RATES.length
  el.playbackRate = PLAYBACK_RATES[rateIndex.value]!
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
  landscapeQuery.addEventListener('change', onOrientationChange)
  // 初始即为横屏(平板横放打开)时自动进入伪全屏
  if (IS_ANDROID && landscapeQuery.matches) isFullscreen.value = true
  void load()
})

// 从相似影片进入另一部(同一路由组件复用):番号变化即重载
watch(
  () => route.params.code,
  (code, prev) => {
    if (code && code !== prev) {
      void recordProgressSnapshot()
      recoverCount = 0
      void load()
      window.scrollTo(0, 0)
    }
  },
)

onBeforeUnmount(() => {
  document.removeEventListener('fullscreenchange', onFullscreenChange)
  landscapeQuery.removeEventListener('change', onOrientationChange)
  if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined)
  // 全局历史:离开页面补记播放位置
  void recordProgressSnapshot()
})
</script>

<template>
  <div class="video-detail-page">
    <div class="player-section" :class="{ 'player-fullscreen': isFullscreen && IS_ANDROID }">
      <button class="back-btn" @click="router.back()">‹ 返回</button>
      <button
        v-if="!IS_ANDROID"
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
                @pause="onPlayerPause"
              ></hlsjs-video>
            </video-skin>
          </video-player>
        </media-i18n>
        <div v-if="recovering" class="recover-overlay">
          <span>播放地址已过期,正在自动刷新...</span>
        </div>
      </div>
      <div class="player-controls">
        <button type="button" class="ctrl-btn" title="快退 1 分钟" @click="skipBy(-60)">
          « 1min
        </button>
        <button type="button" class="ctrl-btn rate" :title="`倍速 ${currentRate}x`" @click="cyclePlaybackRate">
          {{ currentRate }}x
        </button>
        <button type="button" class="ctrl-btn" title="快进 1 分钟" @click="skipBy(60)">
          1min »
        </button>
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
        <span v-for="tag in (detail.tags ?? []).slice(0, 30)" :key="tag" class="video-tag">
          {{ tag }}
        </span>
      </div>

      <div v-if="detail.releaseDate" class="meta-row">
        <span class="meta-label">发行日期</span>
        <span class="meta-text">{{ detail.releaseDate }}</span>
      </div>
      <div v-for="row in metaRows" :key="row.label" class="meta-row">
        <span class="meta-label">{{ row.label }}</span>
        <span class="meta-actors">
          <button
            v-for="link in row.links"
            :key="link.kind + link.id"
            type="button"
            class="meta-link"
            :class="{ selected: isSelected(link) }"
            @click="selectMeta(link)"
          >
            {{ link.name }}
          </button>
        </span>
      </div>

      <div v-if="relatedTarget" class="related-block">
        <div class="related-head">
          <span class="related-heading">「{{ relatedTarget.name }}」的作品</span>
          <button v-if="canFollow" type="button" class="related-follow" @click="followSelected">+ 关注</button>
          <span v-if="followNotice" class="related-notice">{{ followNotice }}</span>
        </div>
        <VideoCardGrid v-if="relatedWorks.length" :items="relatedWorks" />
        <p v-else-if="!relatedLoading && !relatedError" class="related-empty">暂无其它作品</p>
        <p v-if="relatedError" class="related-empty">{{ relatedError }}</p>
        <div v-if="relatedLoading" class="related-loading"><LoadingSpinner message="加载相似影片..." /></div>
        <div v-else-if="relatedHasNext" class="related-more-row">
          <button type="button" class="related-more" @click="loadRelated(relatedPage + 1)">加载更多</button>
        </div>
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

/* 任务 7/8:播放控制条(快退/倍速/快进,player-section 底部悬浮) */
.player-controls {
  position: absolute;
  bottom: 0;
  left: 0;
  right: 0;
  z-index: 3;
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--spacing-md);
  padding: 8px var(--spacing-md) calc(8px + env(safe-area-inset-bottom, 0px));
  background: linear-gradient(transparent, rgba(0, 0, 0, 0.6));
}

.ctrl-btn {
  padding: 6px 14px;
  border: none;
  border-radius: var(--radius-pill, 999px);
  background: rgba(0, 0, 0, 0.55);
  color: #fff;
  font-size: var(--font-size-caption);
  cursor: pointer;
}

.ctrl-btn:hover {
  background: rgba(0, 0, 0, 0.75);
}

.ctrl-btn.rate {
  min-width: 52px;
  font-weight: 600;
}

/* 任务 6:快进/快退指示动画往中心移(skin 默认贴左右边缘) */
.player-skin ::v-deep(.media-seek-indicator[data-direction]) {
  justify-self: center !important;
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

.meta-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-bottom: 6px;
}

.meta-label {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
  flex-shrink: 0;
}

.meta-text {
  font-size: var(--font-size-caption);
  color: var(--color-text-primary);
}

.meta-link {
  border: 1px solid var(--color-hairline);
  background: transparent;
  color: var(--color-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
  padding: 2px 10px;
  border-radius: var(--radius-pill);
}

.meta-link:hover {
  border-color: var(--color-primary);
}

.meta-link.selected {
  background: var(--color-primary);
  border-color: var(--color-primary);
  color: var(--color-on-primary);
}

.meta-actors {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.related-block {
  margin-top: var(--spacing-lg);
  border-top: 1px solid var(--color-hairline);
  padding-top: var(--spacing-md);
}

.related-head {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  margin-bottom: var(--spacing-sm);
}

.related-heading {
  font-size: var(--font-size-caption);
  font-weight: 600;
  color: var(--color-text-primary);
}

.related-follow {
  padding: 2px 10px;
  border-radius: var(--radius-pill);
  border: 1px solid var(--color-primary);
  background: transparent;
  color: var(--color-primary);
  font-size: var(--font-size-fine-print);
  cursor: pointer;
}

.related-notice {
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
}

.related-empty {
  font-size: var(--font-size-caption);
  color: var(--color-text-secondary);
  padding: var(--spacing-sm) 0;
}

.related-loading {
  padding: var(--spacing-md) 0;
}

.related-more-row {
  display: flex;
  justify-content: center;
  padding: var(--spacing-md) 0;
}

.related-more {
  padding: 6px 18px;
  border-radius: var(--radius-pill);
  border: 1px solid var(--color-hairline);
  background: transparent;
  color: var(--color-text-primary);
  font-size: var(--font-size-caption);
  cursor: pointer;
}
</style>
