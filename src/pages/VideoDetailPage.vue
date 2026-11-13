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

import { fetchVideoDetail, recordVideoWatched, recordItemHistory } from '@/services/api'
import type { VideoDetail, VideoMetaLink } from '@/types'
import LoadingSpinner from '@/components/LoadingSpinner.vue'

const route = useRoute()
const router = useRouter()

const detail = ref<VideoDetail | null>(null)
const loading = ref(true)
const error = ref('')
const recovering = ref(false)
let recoverCount = 0
const mediaEl = ref<HlsJsAdapter | null>(null)

/** 请求代次:快速切换番号/重试/签名刷新时,只认最新一次请求的结果 */
let loadSeq = 0
/** 组件已卸载:终止 skin 轮询等异步尾巴 */
let disposed = false

/** 当前路由番号(统一小写,与后端缓存键一致) */
function routeCode(): string {
  return String(route.params.code || '').toLowerCase()
}

/** 媒体元素的播放位置(HlsJsAdapter 类型未暴露 currentTime) */
type TimeHost = { currentTime?: number }

async function load() {
  const seq = ++loadSeq
  // detail 置空会让 v-if 卸载播放器元素(自定义元素断开即销毁 hls 实例)
  detail.value = null
  loading.value = true
  error.value = ''
  recovering.value = false
  const code = routeCode()
  try {
    const d = await fetchVideoDetail(code)
    if (seq !== loadSeq || disposed) return // 已切到别的番号/重试,丢弃过期响应
    detail.value = d
    loading.value = false
    recordVideoWatched(code).catch(() => undefined)
    // 全局历史时间线:播放开始记录基础条目(位置在暂停/离开时补记)
    recordItemHistory(
      d.provider || 'fs1',
      'video',
      d.code,
      d.title || d.code,
      undefined,
      `https://fourhoi.com/${d.code}/cover-t.jpg`,
    ).catch(() => undefined)
    await nextTick()
    void injectSkinControls(seq)
  } catch (e) {
    if (seq !== loadSeq || disposed) return
    error.value = errorMessage(e) || '加载失败'
    loading.value = false
  }
}

/** 全局历史:暂停/离开时补记播放位置(秒) */
async function recordProgressSnapshot() {
  const d = detail.value
  if (!d) return
  const pos = (mediaEl.value as unknown as TimeHost | null)?.currentTime
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

// —— 元数据(厂商/女优/男优/系列/导演/标签) ——
// 点任一条目进入该条目的独立作品列表页(missav 列表页,复用 video_follow_works 通道)
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

const visibleTags = computed(() => (detail.value?.tags ?? []).slice(0, 30))

function selectMeta(link: VideoMetaLink) {
  void router.push(`/videos/target/missav/${link.kind}/${encodeURIComponent(link.id)}`)
}

/** hls.js 错误附带的原始数据,403 时含 response.code */
interface HlsErrorData {
  response?: { code?: number }
}

/** 签名过期自动刷新上限(每个番号/每次手动重试重新计数) */
const MAX_RECOVER = 2

function onMediaError() {
  const err = mediaEl.value?.error ?? null
  const msg = err?.message ?? ''
  const hlsData = err ? (err as { data?: HlsErrorData }).data : undefined
  console.error('[player]', err?.code, msg)
  if (recovering.value) return // 刷新进行中,旧地址的连锁错误忽略
  // 上游 403 多为 HLS 签名过期(短时效,频繁快进后过期):
  // 自动重新获取详情换新签名;src 绑定随 detail 更新触发重载
  const upstream403 = hlsData?.response?.code === 403 || err?.code === 4 || msg.includes('403')
  const current = detail.value
  if (upstream403 && recoverCount < MAX_RECOVER && current) {
    recoverCount += 1
    recovering.value = true
    const seq = loadSeq
    fetchVideoDetail(current.code)
      .then(async (fresh) => {
        if (seq !== loadSeq || disposed) return
        detail.value = fresh
        // 等 Vue 把新签名写入 :src.prop 后再 play,否则元素仍持有旧地址,恢复空转
        await nextTick()
        try {
          await mediaEl.value?.play()
        } catch {
          /* 自动播放被拒:地址已换新,用户手动点播放即可,不算刷新失败 */
        }
      })
      .catch(() => {
        if (seq !== loadSeq || disposed) return
        error.value = '播放地址已过期且刷新失败,请重试'
      })
      .finally(() => {
        if (seq === loadSeq) recovering.value = false
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
// (fixed 铺满,随横竖屏自动切换),桌面走元素全屏。
const IS_ANDROID = /android/i.test(navigator.userAgent)
const playerBox = ref<HTMLDivElement | null>(null)
const isFullscreen = ref(false)

function onFullscreenChange() {
  isFullscreen.value = !!document.fullscreenElement
}

// 横屏自动全屏/竖屏自动退出(旋转传感器驱动);桌面保留手动按钮
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

// 快进快退(±1 分钟)
const SKIP_SECONDS = 60

function skipBy(seconds: number) {
  const el = mediaEl.value as unknown as TimeHost | null
  if (!el || typeof el.currentTime !== 'number') return
  el.currentTime = Math.max(0, el.currentTime + seconds)
}

// ===== shadow DOM 注入(scoped CSS 穿不进 shadowRoot,需向 skin.shadowRoot 追加) =====
const SKIN_POLL_MS = 200
const SKIN_WAIT_MS = 5000

/** 轮询本页播放器内的 skin shadowRoot;番号切换或卸载即放弃 */
async function waitSkinRoot(seq: number): Promise<ShadowRoot | null> {
  for (let elapsed = 0; elapsed <= SKIN_WAIT_MS; elapsed += SKIN_POLL_MS) {
    if (disposed || seq !== loadSeq) return null
    const root = playerBox.value?.querySelector('video-skin')?.shadowRoot
    if (root) return root
    await new Promise((resolve) => setTimeout(resolve, SKIN_POLL_MS))
  }
  return null
}

// 自定义属性可穿透 shadow 边界继承,注入样式直接引用设计 token
const SKIN_STYLE = `
  .media-mute-button, .media-volume-popover,
  [data-action="volume"], [data-action="mute"],
  [part="mute-button"], [part="volume"] {
    display: none !important;
  }
  .media-seek-indicator[data-direction] {
    justify-self: center !important;
  }
  /* 控制区安全留白(两侧+底部,系统导航键/手势条不遮挡快进快退) */
  .media-controls {
    padding-left: max(var(--spacing-xs), env(safe-area-inset-left)) !important;
    padding-right: max(var(--spacing-xs), env(safe-area-inset-right)) !important;
    padding-bottom: max(6px, env(safe-area-inset-bottom)) !important;
  }
  .skin-skip-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 30px;
    margin: 0 var(--spacing-xxs);
    padding: 0;
    border: none;
    border-radius: var(--radius-pill);
    background: rgba(255, 255, 255, 0.14);
    color: var(--color-on-primary);
    cursor: pointer;
    -webkit-tap-highlight-color: transparent;
    transition:
      transform var(--duration-fast) var(--ease-standard),
      background-color var(--duration-fast) var(--ease-standard);
  }
  .skin-skip-btn:hover {
    background: rgba(255, 255, 255, 0.28);
  }
  .skin-skip-btn:active {
    transform: scale(var(--press-scale));
  }
  .skin-skip-btn svg {
    width: 18px;
    height: 18px;
  }
`

const SKIP_ICON_BACK =
  '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M11 18V6l-8.5 6 8.5 6zm.5-6l8.5 6V6l-8.5 6z"/></svg>'
const SKIP_ICON_FWD =
  '<svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M4 6v12l8.5-6L4 6zm8.5 0v12l8.5-6-8.5-6z"/></svg>'

/** 向 skin.shadowRoot 追加样式表(隐藏音量/静音,seek 指示器居中)并在播放键
 *  两侧注入快退/快进按钮。skin 随番号切换重建,每次 load 成功后调用一次 */
async function injectSkinControls(seq: number) {
  const root = await waitSkinRoot(seq)
  if (!root) return
  // 防重复标记:同一 skin 已注入则跳过(重建的 skin 是新 shadowRoot)
  if (root.querySelector('.skin-skip-btn')) return

  const style = document.createElement('style')
  style.textContent = SKIN_STYLE
  root.appendChild(style)

  const playBtn =
    root.querySelector('.media-play-button') ??
    root.querySelector('[data-action="play"]') ??
    root.querySelector('[part="play-button"]')
  const parent = playBtn?.parentElement
  if (!playBtn || !parent) return

  const mkBtn = (icon: string, secs: number): HTMLButtonElement => {
    const b = document.createElement('button')
    b.type = 'button'
    b.className = 'skin-skip-btn'
    b.innerHTML = icon
    b.title = secs > 0 ? '快进 1 分钟' : '快退 1 分钟'
    b.setAttribute('aria-label', b.title)
    b.addEventListener('click', () => skipBy(secs))
    return b
  }
  parent.insertBefore(mkBtn(SKIP_ICON_BACK, -SKIP_SECONDS), playBtn)
  parent.insertBefore(mkBtn(SKIP_ICON_FWD, SKIP_SECONDS), playBtn.nextSibling)
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

// 从其他作品进入另一部(同一路由组件复用):番号变化即补记旧片进度并重载
watch(
  () => route.params.code,
  (code, prev) => {
    if (!code || code === prev) return
    void recordProgressSnapshot()
    recoverCount = 0
    void load()
    window.scrollTo(0, 0)
  },
)

onBeforeUnmount(() => {
  disposed = true
  document.removeEventListener('fullscreenchange', onFullscreenChange)
  landscapeQuery.removeEventListener('change', onOrientationChange)
  if (document.fullscreenElement) void document.exitFullscreen().catch(() => undefined)
  // 全局历史:离开页面补记播放位置(此时模板 ref 仍有效)
  void recordProgressSnapshot()
})
</script>

<template>
  <div class="video-detail-page">
    <div class="player-section" :class="{ 'player-fullscreen': isFullscreen && IS_ANDROID }">
      <button type="button" class="overlay-btn back-btn" @click="router.back()">‹ 返回</button>
      <button
        v-if="!IS_ANDROID"
        type="button"
        class="overlay-btn fs-btn"
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
        <div v-else-if="error" class="player-error" role="alert">
          <p class="player-error-title">播放失败</p>
          <p class="player-error-msg">{{ error }}</p>
          <div class="player-error-actions">
            <button type="button" class="pill-btn primary" @click="retry">重试</button>
            <button type="button" class="pill-btn ghost" @click="copyError">复制错误</button>
          </div>
        </div>
        <media-i18n v-else-if="detail" lang="zh-CN">
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
        <Transition name="fade">
          <div v-if="recovering" class="recover-overlay" role="status">
            <span>播放地址已过期,正在自动刷新...</span>
          </div>
        </Transition>
      </div>
    </div>

    <!-- 详情加载中:元数据骨架占位,避免内容到达时下方区域跳动 -->
    <div v-if="loading" class="meta-section meta-skeleton" aria-hidden="true">
      <div class="sk sk-badge" />
      <div class="sk sk-line w-90" />
      <div class="sk sk-line w-60" />
      <div class="sk-chips">
        <div v-for="n in 4" :key="n" class="sk sk-chip" />
      </div>
    </div>

    <Transition name="rise" appear>
      <div v-if="detail" :key="detail.code" class="meta-section">
        <div class="provider-row">
          <span class="provider-badge" :class="detail.provider">{{
            detail.provider === 'missav' ? 'missav 优先线路' : 'fs1 线路'
          }}</span>
          <span v-if="detail.chinese" class="provider-badge chinese">中文字幕</span>
        </div>
        <h1 class="video-title">{{ detail.title || detail.code.toUpperCase() }}</h1>
        <div v-if="visibleTags.length" class="video-tags">
          <span v-for="tag in visibleTags" :key="tag" class="video-tag">{{ tag }}</span>
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
              @click="selectMeta(link)"
            >
              {{ link.name }}
            </button>
          </span>
        </div>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.video-detail-page {
  max-width: 960px;
  margin: 0 auto;
  padding-bottom: var(--spacing-xl);
}

/* 播放区恒为暗底:就地把文字 token 切到暗色语义,
   加载提示/错误文案在明暗主题下都可读 */
.player-section {
  position: relative;
  background: var(--color-surface-black);
  color: var(--color-body-on-dark);
  --color-text-primary: var(--color-body-on-dark);
  --color-text-secondary: var(--color-body-muted);
  --color-text-muted: var(--color-body-muted);
}

/* 播放器上方的浮层按钮:视觉紧凑,命中区经 ::after 扩到 44px */
.overlay-btn {
  position: absolute;
  top: var(--spacing-xs);
  z-index: 3;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  height: 30px;
  border: none;
  background: var(--color-scrim);
  color: var(--color-body-on-dark);
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  transition: var(--transition-press);
}

.overlay-btn::after {
  content: '';
  position: absolute;
  inset: -7px;
}

.overlay-btn:active {
  transform: scale(var(--press-scale));
}

.back-btn {
  left: max(var(--spacing-xs), env(safe-area-inset-left));
  padding: 0 var(--spacing-sm);
  border-radius: var(--radius-pill);
  font-size: var(--font-size-caption);
}

.fs-btn {
  right: max(var(--spacing-xs), env(safe-area-inset-right));
  width: 34px;
  border-radius: var(--radius-sm);
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

/* Android 伪全屏:播放区 fixed 铺满视口,必须盖住顶栏与底栏
   ——横屏视口矮,层级不足时底栏会直接压住进度条;保持 16:9 居中
   (aspect-ratio + max-height 由浏览器按比例反推宽度,不拉伸视频) */
.player-section.player-fullscreen {
  position: fixed;
  inset: 0;
  z-index: var(--z-overlay);
  display: flex;
  align-items: center;
  justify-content: center;
}

.player-section.player-fullscreen .player-box {
  width: 100%;
  max-height: 100%;
}

/* 固定 16:9,加载/错误/播放三态切换不引起布局跳动 */
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

/* 播放区内的紧凑错误态(16:9 窄屏高度有限,不用整页 EmptyState) */
.player-error {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--spacing-xs);
  max-width: 100%;
  padding: var(--spacing-md);
  text-align: center;
}

.player-error-title {
  margin: 0;
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
}

.player-error-msg {
  display: -webkit-box;
  margin: 0;
  overflow: hidden;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  font-size: var(--font-size-fine-print);
  color: var(--color-text-secondary);
  word-break: break-all;
}

.player-error-actions {
  display: flex;
  gap: var(--spacing-xs);
  margin-top: var(--spacing-xxs);
}

.pill-btn {
  min-height: 36px;
  padding: 0 var(--spacing-lg);
  border: none;
  border-radius: var(--radius-pill);
  font-size: var(--font-size-caption);
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  transition: var(--transition-press);
}

.pill-btn:active {
  transform: scale(var(--press-scale));
}

.pill-btn.primary {
  background: var(--color-primary);
  color: var(--color-on-primary);
}

.pill-btn.ghost {
  background: transparent;
  border: 1px solid var(--color-body-muted);
  color: var(--color-body-on-dark);
}

.recover-overlay {
  position: absolute;
  inset: 0;
  z-index: 2;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: var(--spacing-md);
  background: var(--color-scrim);
  color: var(--color-body-on-dark);
  font-size: var(--font-size-caption);
  text-align: center;
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
  border-radius: var(--radius-pill);
  font-size: var(--font-size-fine-print);
  line-height: var(--line-height-body);
  background: var(--color-tag-bg);
  color: var(--color-text-secondary);
}

.provider-badge.missav {
  background: var(--color-primary);
  color: var(--color-on-primary);
}

.provider-badge.chinese {
  background: var(--color-badge-cs);
  color: var(--color-on-primary);
}

.video-title {
  margin: 0 0 var(--spacing-sm);
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-semibold);
  line-height: var(--line-height-body);
  letter-spacing: var(--letter-spacing-body);
  color: var(--color-text-primary);
}

.video-tags {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-bottom: var(--spacing-sm);
}

.video-tag {
  padding: 2px 10px;
  border-radius: var(--radius-pill);
  background: var(--color-tag-bg);
  color: var(--color-text-secondary);
  font-size: var(--font-size-fine-print);
  line-height: var(--line-height-body);
}

.meta-row {
  display: flex;
  align-items: baseline;
  gap: 10px;
  margin-bottom: var(--spacing-xs);
}

.meta-label {
  flex-shrink: 0;
  min-width: 4em;
  font-size: var(--font-size-fine-print);
  color: var(--color-text-muted);
}

.meta-text {
  font-size: var(--font-size-caption);
  color: var(--color-text-primary);
}

.meta-actors {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
}

.meta-link {
  display: inline-flex;
  align-items: center;
  min-height: 32px;
  padding: 0 var(--spacing-sm);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-pill);
  background: transparent;
  color: var(--color-accent);
  font-size: var(--font-size-caption);
  cursor: pointer;
  -webkit-tap-highlight-color: transparent;
  transition:
    transform var(--duration-fast) var(--ease-standard),
    border-color var(--duration-fast) var(--ease-standard);
}

.meta-link:hover {
  border-color: var(--color-accent);
}

.meta-link:active {
  transform: scale(var(--press-scale));
}

.meta-link:focus-visible,
.overlay-btn:focus-visible,
.pill-btn:focus-visible {
  outline: 2px solid var(--color-primary-focus);
  outline-offset: 2px;
}

/* —— 元数据骨架 —— */
.sk {
  border-radius: var(--radius-pill);
  background: var(--color-skeleton);
  animation: sk-pulse 1.4s var(--ease-standard) infinite alternate;
}

.sk-badge {
  width: 88px;
  height: 20px;
  margin-bottom: var(--spacing-sm);
}

.sk-line {
  height: 16px;
  margin-bottom: var(--spacing-xs);
  border-radius: var(--radius-xs);
}

.sk-line.w-90 {
  width: 90%;
}

.sk-line.w-60 {
  width: 60%;
}

.sk-chips {
  display: flex;
  gap: 6px;
  margin-top: var(--spacing-sm);
}

.sk-chip {
  width: 56px;
  height: 22px;
}

@keyframes sk-pulse {
  from {
    opacity: 1;
  }
  to {
    opacity: 0.45;
  }
}

/* —— 过渡 —— */
.fade-enter-active,
.fade-leave-active {
  transition: opacity var(--duration-base) var(--ease-standard);
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

.rise-enter-active {
  transition:
    opacity var(--duration-slow) var(--ease-out),
    transform var(--duration-slow) var(--ease-out);
}

.rise-enter-from {
  opacity: 0;
  transform: translateY(6px);
}
</style>
