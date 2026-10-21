<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { fetchImage, fetchVideoImage } from '@/services/api'
import type { SourceId } from '@/config/sources'

type ImageSource = SourceId | 'video'

const props = defineProps<{ src: string; source?: ImageSource; alt?: string; loading?: 'eager' | 'lazy' }>()
const emit = defineEmits<{ error: [] }>()
const objectUrl = ref('')
let generation = 0

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
  if (objectUrl.value) URL.revokeObjectURL(objectUrl.value)
  objectUrl.value = ''
  if (!url) return
  try {
    const bytes = props.source === 'video'
      ? await fetchVideoImage(url)
      : await fetchImage(url, props.source)
    if (current !== generation) return
    objectUrl.value = URL.createObjectURL(new Blob([bytes], { type: imageContentType(url, bytes) }))
  } catch {
    if (current === generation) emit('error')
  }
}, { immediate: true })

onBeforeUnmount(() => {
  generation++
  if (objectUrl.value) URL.revokeObjectURL(objectUrl.value)
})
</script>

<template>
  <img v-if="objectUrl" :src="objectUrl" :alt="alt || ''" :loading="loading" />
</template>
