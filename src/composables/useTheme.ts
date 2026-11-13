import { ref, computed, onMounted } from 'vue'

const THEME_KEY = 'niacg-theme'

export type Theme = 'system' | 'light' | 'dark'

function readStored(): Theme {
  try {
    const v = localStorage.getItem(THEME_KEY)
    return v === 'light' || v === 'dark' || v === 'system' ? v : 'system'
  } catch {
    return 'system'
  }
}

// 模块级单例:多处/多次挂载共享同一状态,系统主题监听只注册一次(此前每次进入设置页都会再挂一个)
const current = ref<Theme>(readStored())
let listening = false

function isSystemDark(): boolean {
  const androidDark = (window as any).__ANDROID_DARK_MODE__
  if (typeof androidDark === 'boolean') return androidDark
  return window.matchMedia('(prefers-color-scheme: dark)').matches
}

function applyTheme(theme: Theme) {
  const root = document.documentElement
  const dark = theme === 'dark' || (theme === 'system' && isSystemDark())
  if (root.classList.contains('dark') === dark && root.classList.contains('light') === !dark) return
  // 换色瞬间关闭全局过渡,避免各组件 background/color 逐个渐变造成闪烁(见 global.css)
  root.classList.add('theme-switching')
  root.classList.toggle('dark', dark)
  root.classList.toggle('light', !dark)
  // 强制一次样式计算后再移除,确保新配色在无过渡状态下落定
  void root.offsetHeight
  requestAnimationFrame(() => root.classList.remove('theme-switching'))
}

function ensureListeners() {
  if (listening) return
  listening = true
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', () => {
    if (current.value === 'system') applyTheme('system')
  })
  window.addEventListener('android-ready', () => applyTheme(current.value))
}

export function useTheme() {
  function setTheme(theme: Theme) {
    current.value = theme
    try {
      localStorage.setItem(THEME_KEY, theme)
    } catch {
      // 存储不可用时仅本次会话生效
    }
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
    ensureListeners()
  })

  return { current, setTheme, toggleTheme, nextLabel }
}
