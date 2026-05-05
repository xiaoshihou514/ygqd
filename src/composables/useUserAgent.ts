import { computed } from 'vue'

export function useUserAgent() {
  const isAndroid = computed(() => {
    if (typeof navigator === 'undefined') return false
    return /android/i.test(navigator.userAgent)
  })

  return { isAndroid }
}
