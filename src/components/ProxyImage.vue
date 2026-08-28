<script setup lang="ts">
import { onBeforeUnmount, ref, watch } from 'vue'
import { fetchImage } from '@/services/api'

const props = defineProps<{ src: string; alt?: string; loading?: 'eager' | 'lazy' }>()
const emit = defineEmits<{ error: [] }>()
const objectUrl = ref('')
let generation = 0

watch(() => props.src, async (url) => {
  const current = ++generation
  if (objectUrl.value) URL.revokeObjectURL(objectUrl.value)
  objectUrl.value = ''
  if (!url) return
  try {
    const bytes = await fetchImage(url)
    if (current !== generation) return
    objectUrl.value = URL.createObjectURL(new Blob([bytes]))
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
