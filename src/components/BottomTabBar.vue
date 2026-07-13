<script setup lang="ts">
import { RouterLink, useRoute } from 'vue-router'
import { useTheme } from '@/composables/useTheme'

const route = useRoute()
const { current, toggleTheme } = useTheme()

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
    to: '/settings/blacklist',
    icon: 'M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.066 2.573c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.573 1.066c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.066-2.573c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z M15 12a3 3 0 11-6 0 3 3 0 016 0z',
  },
]
</script>

<template>
  <nav class="bottom-tab-bar">
    <div class="tab-bar-inner">
      <RouterLink
        v-for="tab in tabs"
        :key="tab.to"
        :to="tab.to"
        class="tab-item"
        :class="{ active: route.path === tab.to }"
      >
        <svg
          class="tab-icon"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <path :d="tab.icon" />
        </svg>
        <span class="tab-label">{{ tab.label }}</span>
      </RouterLink>
    </div>
    <button class="theme-fab" @click="toggleTheme" :title="'Theme: ' + current">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path v-if="current === 'dark'" d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z" />
        <path v-else d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z" />
      </svg>
    </button>
  </nav>
</template>

<style scoped>
.bottom-tab-bar {
  position: fixed;
  bottom: 0;
  left: 0;
  right: 0;
  height: var(--tab-bar-height);
  background: var(--color-tab-bar-bg);
  border-top: 1px solid var(--color-hairline);
  z-index: 100;
  display: flex;
  align-items: center;
  padding-bottom: var(--tab-bar-safe-bottom);
}

.tab-bar-inner {
  flex: 1;
  display: flex;
  justify-content: space-around;
  align-items: center;
  height: 100%;
  max-width: var(--max-width);
  margin: 0 auto;
}

.tab-item {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 2px;
  padding: 4px 0;
  color: var(--color-text-muted);
  text-decoration: none;
  transition: color 0.2s;
  min-width: 48px;
}

.tab-item.active {
  color: var(--color-accent);
}

.tab-icon {
  width: 24px;
  height: 24px;
}

.tab-label {
  font-size: 10px;
  font-weight: var(--font-weight-semibold);
  letter-spacing: 0.2px;
}

.theme-fab {
  position: absolute;
  right: 8px;
  top: -28px;
  width: 40px;
  height: 40px;
  border-radius: 50%;
  background: var(--color-accent);
  color: var(--color-on-primary);
  border: none;
  display: flex;
  align-items: center;
  justify-content: center;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.25);
  cursor: pointer;
  transition: transform 0.15s, background 0.2s;
}

.theme-fab:active {
  transform: scale(0.9);
}

.theme-fab svg {
  width: 22px;
  height: 22px;
}

@media (min-width: 769px) {
  .theme-fab {
    position: static;
    width: 36px;
    height: 36px;
    border-radius: var(--radius-sm);
    box-shadow: none;
    background: rgba(255, 255, 255, 0.1);
    color: var(--color-navbar-text);
    margin-left: auto;
  }
}
</style>
