<script lang="ts">
// ───── 模块级共享(所有 ProxyImage 实例共用) ─────
// 1) 同一图片并发请求去重(同屏同封面、列表重挂载时不重复走 IPC);
// 2) 小型 Blob LRU(按字节计):返回列表/切换分段重挂载时直接命中,不再走网络。
//    缓存的是 Blob 而非 Blob URL——每个实例自建/自撤 URL,撤销互不影响,无共享 URL 泄漏;
// 3) 共享一个 IntersectionObserver 做懒加载(loading="lazy" 时,离屏不取二进制)。
import { fetchImage, fetchVideoImage } from '@/services/api'
import type { SourceId } from '@/config/sources'

type ImageSource = SourceId | 'video'

const CACHE_LIMIT_BYTES = 24 * 1024 * 1024
/** 单张超过上限 1/8 的大图(阅读页大图)不进缓存,避免把封面挤光 */
const CACHE_ITEM_LIMIT = CACHE_LIMIT_BYTES / 8
const blobCache = new Map<string, Blob>() // Map 插入序即 LRU 序
let cacheBytes = 0
const inflight = new Map<string, Promise<Blob>>()

function cacheKey(url: string, source?: ImageSource): string {
  return `${source ?? 'niacg'}|${url}`
}

function cacheGet(key: string): Blob | undefined {
  const blob = blobCache.get(key)
  if (blob) {
    blobCache.delete(key)
    blobCache.set(key, blob)
  }
  return blob
}

function cachePut(key: string, blob: Blob) {
  if (blob.size > CACHE_ITEM_LIMIT || blobCache.has(key)) return
  blobCache.set(key, blob)
  cacheBytes += blob.size
  for (const [k, b] of blobCache) {
    if (cacheBytes <= CACHE_LIMIT_BYTES) break
    blobCache.delete(k)
    cacheBytes -= b.size
  }
}

function imageContentType(url: string, bytes: ArrayBuffer): string {
  const data = new Uint8Array(bytes)
  if (data[0] === 0xff && data[1] === 0xd8 && data[2] === 0xff) return 'image/jpeg'
  if (data[0] === 0x89 && data[1] === 0x50 && data[2] === 0x4e && data[3] === 0x47) return 'image/png'
  const signature = String.fromCharCode(...data.subarray(0, 12))
  if (signature.startsWith('GIF8')) return 'image/gif'
  if (signature.startsWith('RIFF') && signature.slice(8, 12) === 'WEBP') return 'image/webp'
  if (signature.slice(4, 12).includes('ftypavif') || signature.slice(4, 12).includes('ftypavis')) return 'image/avif'

  const path = new URL(url, window.location.href).pathname.toLowerCase()
  if (path.endsWith('.webp')) return 'image/webp'
  if (path.endsWith('.png')) return 'image/png'
  if (path.endsWith('.avif')) return 'image/avif'
  if (path.endsWith('.gif')) return 'image/gif'
  return 'image/jpeg'
}

function loadBlob(url: string, source: ImageSource | undefined, key: string): Promise<Blob> {
  let pending = inflight.get(key)
  if (!pending) {
    const request = source === 'video' ? fetchVideoImage(url) : fetchImage(url, source)
    pending = request
      .then((bytes) => new Blob([bytes], { type: imageContentType(url, bytes) }))
      .finally(() => inflight.delete(key))
    inflight.set(key, pending)
  }
  return pending
}

/** 换图前先异步解码:替换占位时直接出完整画面(无半帧空白/逐行绘制);坏数据在此判失败 */
async function decodeImage(src: string): Promise<void> {
  const img = new Image()
  img.src = src
  if (typeof img.decode === 'function') {
    await img.decode()
    return
  }
  await new Promise<void>((resolve, reject) => {
    img.onload = () => resolve()
    img.onerror = () => reject(new Error('decode failed'))
  })
}

const lazyCallbacks = new WeakMap<Element, () => void>()
let lazyObserver: IntersectionObserver | null = null

/** 元素接近视口(上下左右 400px 预取)时回调一次;返回取消函数 */
function observeNear(el: Element, onNear: () => void): () => void {
  if (typeof IntersectionObserver === 'undefined') {
    onNear()
    return () => {}
  }
  lazyObserver ??= new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (!entry.isIntersecting) continue
        const fn = lazyCallbacks.get(entry.target)
        lazyObserver?.unobserve(entry.target)
        lazyCallbacks.delete(entry.target)
        fn?.()
      }
    },
    { rootMargin: '400px' },
  )
  lazyCallbacks.set(el, onNear)
  lazyObserver.observe(el)
  return () => {
    lazyObserver?.unobserve(el)
    lazyCallbacks.delete(el)
  }
}
</script>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch } from 'vue'

const props = defineProps<{ src: string; source?: ImageSource; alt?: string; loading?: 'eager' | 'lazy' }>()
/** load:图片已解码并显示(含缓存命中);error:重试后仍失败 */
const emit = defineEmits<{ error: []; load: [] }>()
const objectUrl = ref('')
const failed = ref(false)
/** 经网络取回的图淡入;缓存命中直接显示(返回列表时不闪) */
const fadeIn = ref(false)
const pendingEl = ref<HTMLElement | null>(null)
let generation = 0
let retryTimer = 0
let stopObserve: (() => void) | null = null

function reset() {
  generation++
  clearTimeout(retryTimer)
  stopObserve?.()
  stopObserve = null
  if (objectUrl.value) URL.revokeObjectURL(objectUrl.value)
  objectUrl.value = ''
  failed.value = false
  fadeIn.value = false
}

async function load(current: number, url: string, source: ImageSource | undefined, key: string) {
  // 失败自动重试 1 次(短退避):封面 CDN 偶发限流(403/429)一次退避即可恢复
  for (let attempt = 0; ; attempt++) {
    try {
      const blob = await loadBlob(url, source, key)
      if (current !== generation) return
      const nextUrl = URL.createObjectURL(blob)
      try {
        await decodeImage(nextUrl)
      } catch (e) {
        URL.revokeObjectURL(nextUrl)
        throw e
      }
      if (current !== generation) {
        URL.revokeObjectURL(nextUrl)
        return
      }
      cachePut(key, blob)
      fadeIn.value = true
      objectUrl.value = nextUrl
      emit('load')
      return
    } catch {
      if (current !== generation) return
      if (attempt >= 1) break
      await new Promise((resolve) => { retryTimer = window.setTimeout(resolve, 1200) })
      if (current !== generation) return
    }
  }
  failed.value = true
  emit('error')
}

watch(() => [props.src, props.source] as const, async ([url, source]) => {
  reset()
  const current = generation
  if (!url) return
  const key = cacheKey(url, source)
  const cached = cacheGet(key)
  if (cached) {
    objectUrl.value = URL.createObjectURL(cached)
    emit('load')
    return
  }
  if (props.loading === 'lazy') {
    // 占位元素在本轮渲染后才挂上;取不到(极端时序)就直接加载,不卡死在占位
    await nextTick()
    if (current !== generation) return
    const el = pendingEl.value
    if (el) {
      stopObserve = observeNear(el, () => {
        stopObserve = null
        if (current === generation) void load(current, url, source, key)
      })
      return
    }
  }
  void load(current, url, source, key)
}, { immediate: true })

onBeforeUnmount(reset)
</script>

<template>
  <img
    v-if="objectUrl"
    :src="objectUrl"
    :alt="alt || ''"
    :loading="loading"
    :class="{ 'proxy-fade-in': fadeIn }"
    decoding="async"
  />
  <span v-else-if="failed" class="img-broken" role="img" :aria-label="alt || '图片加载失败'">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <circle cx="9" cy="10" r="1.5" />
      <path d="M5 19l5-6 3 3.5 2.5-3L21 19" />
    </svg>
  </span>
  <span v-else-if="src" ref="pendingEl" class="proxy-pending" aria-hidden="true" />
</template>

<style>
/* 零特异性:消费方给根节点的定位(如卡片封面 absolute)优先;仅为扫光伪元素兜底定位上下文 */
:where(.proxy-pending) {
  position: relative;
  overflow: hidden;
}
</style>

<style scoped>
.proxy-fade-in {
  animation: proxy-fade-in var(--duration-slow) var(--ease-out);
}

@keyframes proxy-fade-in {
  from { opacity: 0; }
  to { opacity: 1; }
}

/* 加载占位:骨架底色 + transform 扫光(只动合成层,不触发重绘);
   无尺寸约束的场景(阅读页大图)按 5:7 预留高度,减少载入时的布局跳动 */
.proxy-pending {
  display: block;
  width: 100%;
  height: 100%;
  aspect-ratio: 5 / 7;
  background: var(--color-skeleton);
}

.proxy-pending::after {
  content: '';
  position: absolute;
  inset: 0;
  background: linear-gradient(90deg, transparent, var(--color-skeleton-highlight), transparent);
  transform: translateX(-100%);
  animation: proxy-shimmer calc(var(--duration-slow) * 4) var(--ease-standard) infinite;
}

@keyframes proxy-shimmer {
  to { transform: translateX(100%); }
}

@media (prefers-reduced-motion: reduce) {
  .proxy-fade-in,
  .proxy-pending::after {
    animation: none;
  }

  .proxy-pending::after {
    display: none;
  }
}

.img-broken {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  min-height: 32px;
  color: var(--color-text-muted);
  background: var(--color-skeleton);
}

.img-broken svg {
  width: 22px;
  height: 22px;
  opacity: 0.55;
}
</style>
