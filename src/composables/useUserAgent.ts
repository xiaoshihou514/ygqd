import { computed } from 'vue'
import isMobileLib from 'is-mobile'

export function useUserAgent() {
  const isAndroid = computed(() => isMobileLib())

  return { isAndroid }
}
