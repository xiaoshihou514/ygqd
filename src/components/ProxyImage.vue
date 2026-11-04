<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { fetchImage, fetchVideoImage } from '@/services/api'
import type { SourceId } from '@/config/sources'

type ImageSource = SourceId | 'video'

const props = defineProps<{ src: string; source?: ImageSource; alt?: string; loading?: 'eager' | 'lazy' }>()
const emit = defineEmits<{ error: [] }>()
const objectUrl = ref('')
const failed = ref(false)
let generation = 0
let retryTimer = 0

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

watch(() => [props.src, props.source], async ([url]) => {
  const current = ++generation
  clearTimeout(retryTimer)
  if (objectUrl.value) URL.revokeObjectURL(objectUrl.value)
  objectUrl.value = ''
  failed.value = false
  if (!url) return
  // 失败自动重试 1 次(短退避):封面 CDN 偶发限流(403/429)一次退避即可恢复
  for (let attempt = 0; ; attempt++) {
    try {
      const bytes = props.source === 'video'
        ? await fetchVideoImage(url)
        : await fetchImage(url, props.source)
      if (current !== generation) return
      objectUrl.value = URL.createObjectURL(new Blob([bytes], { type: imageContentType(url, bytes) }))
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
}, { immediate: true })

onBeforeUnmount(() => {
  generation++
  clearTimeout(retryTimer)
  if (objectUrl.value) URL.revokeObjectURL(objectUrl.value)
})
</script>

<template>
  <img v-if="objectUrl" :src="objectUrl" :alt="alt || ''" :loading="loading" />
  <span v-else-if="failed" class="img-broken" role="img" :aria-label="alt || '图片加载失败'">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <rect x="3" y="4" width="18" height="16" rx="2" />
      <circle cx="9" cy="10" r="1.5" />
      <path d="M5 19l5-6 3 3.5 2.5-3L21 19" />
    </svg>
  </span>
</template>

<style scoped>
.img-broken {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  min-height: 32px;
  color: var(--color-text-secondary, #86868b);
  opacity: 0.55;
}

.img-broken svg {
  width: 22px;
  height: 22px;
}
</style>
