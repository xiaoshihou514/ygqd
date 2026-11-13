<script setup lang="ts">
import { nextTick } from 'vue'
import { RouterLink, useRoute, useRouter } from 'vue-router'

const route = useRoute()
const router = useRouter()

const DOUBLE_CLICK_MS = 300
let lastClickTab = ''
let lastClickAt = 0

function scrollToTop() {
  if ('scrollBehavior' in document.documentElement.style) {
    window.scrollTo({ top: 0, behavior: 'smooth' })
  } else {
    window.scrollTo(0, 0)
  }
}

async function handleTabClick(tab: { to: string }) {
  const now = Date.now()
  const isDoubleClick = lastClickTab === tab.to && now - lastClickAt < DOUBLE_CLICK_MS
  lastClickTab = tab.to
  lastClickAt = now
  if (!isDoubleClick) return

  if (route.path !== tab.to) {
    await router.push(tab.to)
  }
  await nextTick()
  // keep-alive 页面会在激活时恢复滚动位置，等它完成后再滚到顶部
  setTimeout(scrollToTop, 100)
}

const tabs = [
  {
    label: '推荐',
    to: '/',
    icon: 'M3 12l2-2m0 0l7-7 7 7M5 10v10a1 1 0 001 1h3m10-11l2 2m-2-2v10a1 1 0 01-1 1h-3m-6 0a1 1 0 001-1v-4a1 1 0 011-1h2a1 1 0 011 1v4a1 1 0 001 1m-6 0h6',
  },
  {
    label: '搜索',
    to: '/search',
    icon: 'M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z',
  },
  {
    label: '关注',
    to: '/following',
    icon: 'M4.318 6.318a4.5 4.5 0 000 6.364L12 20.364l7.682-7.682a4.5 4.5 0 00-6.364-6.364L12 7.636l-1.318-1.318a4.5 4.5 0 00-6.364 0z',
  },
  {
    label: '历史',
    to: '/history',
    icon: 'M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z',
  },
  {
    label: '设置',
    to: '/settings',
    icon: 'M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z M15 12a3 3 0 11-6 0 3 3 0 016 0z',
  },
]
</script>

<template>
  <nav class="bottom-tab-bar" aria-label="主导航">
    <div class="tab-bar-inner">
      <RouterLink
        v-for="tab in tabs"
        :key="tab.to"
        :to="tab.to"
        class="tab-item"
        :class="{ active: route.path === tab.to }"
        @click="handleTabClick(tab)"
      >
        <svg
          class="tab-icon"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
          aria-hidden="true"
        >
          <path :d="tab.icon" />
        </svg>
        <span class="tab-label">{{ tab.label }}</span>
      </RouterLink>
    </div>
  </nav>
</template>

<style scoped>
/* 毛玻璃底栏:与顶部 NavHeader 同一材质(tab-bar-bg 半透明 + 背景模糊 + 发丝线) */
.bottom-tab-bar {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  height: var(--tab-bar-total-height);
  background: var(--color-tab-bar-bg);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  backdrop-filter: saturate(180%) blur(20px);
  border-top: 1px solid var(--color-hairline);
  z-index: var(--z-nav);
  display: flex;
  align-items: flex-start;
  padding-bottom: var(--tab-bar-safe-bottom);
  padding-left: env(safe-area-inset-left, 0px);
  padding-right: env(safe-area-inset-right, 0px);
}

.tab-bar-inner {
  flex: 1;
  display: flex;
  align-items: stretch;
  height: var(--tab-bar-height);
  max-width: var(--max-width);
  margin: 0 auto;
}

/* 整列可点(热区 = 1/5 宽 × 50px),不止图标+文字那一小块 */
.tab-item {
  flex: 1;
  min-width: 48px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  color: var(--color-text-muted);
  text-decoration: none;
  transition: color var(--duration-base) var(--ease-standard);
  -webkit-tap-highlight-color: transparent;
  -webkit-touch-callout: none;
  user-select: none;
}

.tab-item.active {
  color: var(--color-accent);
}

.tab-item:focus-visible {
  outline: 2px solid var(--color-primary-focus);
  outline-offset: -4px;
  border-radius: var(--radius-sm);
}

.tab-icon {
  width: 24px;
  height: 24px;
  transition: transform var(--duration-base) var(--ease-out);
}

.tab-item.active .tab-icon {
  transform: scale(1.06);
}

/* 按压反馈:图标与文字一起轻微收缩 */
.tab-item:active .tab-icon,
.tab-item:active .tab-label {
  transform: scale(var(--press-scale));
  transition-duration: var(--duration-fast);
}

.tab-label {
  font-size: var(--font-size-micro);
  font-weight: var(--font-weight-semibold);
  line-height: 1.2;
  letter-spacing: 0.2px;
  transition: transform var(--duration-base) var(--ease-out);
}
</style>
