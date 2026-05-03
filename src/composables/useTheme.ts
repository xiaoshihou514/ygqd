import { ref, computed, onMounted } from 'vue'

const THEME_KEY = 'niacg-theme'

export type Theme = 'system' | 'light' | 'dark'

export function useTheme() {
  const stored = localStorage.getItem(THEME_KEY) as Theme | null
  const current = ref<Theme>(stored || 'system')

  function applyTheme(theme: Theme) {
    const isDark =
      theme === 'dark' ||
      (theme === 'system' &&
        window.matchMedia('(prefers-color-scheme: dark)').matches)

    document.documentElement.classList.toggle('dark', isDark)
    document.documentElement.classList.toggle('light', !isDark)
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
  })

  return { current, setTheme, toggleTheme, nextLabel }
}
