import { ref, computed, onMounted } from 'vue'

const THEME_KEY = 'niacg-theme'

export type Theme = 'system' | 'light' | 'dark'

export function useTheme() {
  const stored = localStorage.getItem(THEME_KEY) as Theme | null
  const current = ref<Theme>(stored || 'system')

  function isSystemDark(): boolean {
    const androidDark = (window as any).__ANDROID_DARK_MODE__
    if (typeof androidDark === 'boolean') return androidDark
    return window.matchMedia('(prefers-color-scheme: dark)').matches
  }

  function applyTheme(theme: Theme) {
    const dark =
      theme === 'dark' || (theme === 'system' && isSystemDark())
    document.documentElement.classList.toggle('dark', dark)
    document.documentElement.classList.toggle('light', !dark)
  }

  function setTheme(theme: Theme) {
    current.value = theme
    localStorage.setItem(THEME_KEY, theme)
    applyTheme(theme)
  }

  function toggleTheme() {
    if (current.value === 'light') {
      setTheme('dark')
    } else if (current.value === 'dark') {
      setTheme('system')
    } else {
      setTheme('light')
    }
  }

  const nextLabel = computed(() => {
    if (current.value === 'light') return '☀'
    if (current.value === 'dark') return '☾'
    return '☯'
  })

  onMounted(() => {
    applyTheme(current.value)
    window
      .matchMedia('(prefers-color-scheme: dark)')
      .addEventListener('change', () => {
        if (current.value === 'system') {
          applyTheme('system')
        }
      })
    window.addEventListener('android-ready', () => {
      applyTheme(current.value)
    })
  })

  return { current, setTheme, toggleTheme, nextLabel }
}
