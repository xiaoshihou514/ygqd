<script setup lang="ts">
import { RouterLink, useRoute } from 'vue-router'

const route = useRoute()

const navItems = [
  { label: '推荐', to: '/' },
  { label: '搜索', to: '/search' },
]
</script>

<template>
  <header class="nav-header">
    <div class="nav-inner">
      <RouterLink to="/" class="nav-logo">
        <span class="logo-icon">N</span>
        <span class="logo-text">iACG</span>
      </RouterLink>
      <nav class="nav-links">
        <RouterLink
          v-for="item in navItems"
          :key="item.to"
          :to="item.to"
          class="nav-link"
          :class="{ active: route.path === item.to }"
        >
          {{ item.label }}
        </RouterLink>
      </nav>
    </div>
  </header>
</template>

<style scoped>
.nav-header {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: var(--nav-height);
  background: var(--color-navbar-bg);
  box-shadow: var(--shadow-nav);
  z-index: 100;
}

.nav-inner {
  max-width: var(--max-width);
  margin: 0 auto;
  padding: 0 var(--spacing-lg);
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.nav-logo {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  font-weight: var(--font-weight-bold);
  font-size: var(--font-size-tagline);
  color: var(--color-navbar-text);
  letter-spacing: -0.231px;
}

.logo-icon {
  width: 32px;
  height: 32px;
  background: var(--color-primary);
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 18px;
  font-weight: 800;
}

.nav-links {
  display: flex;
  gap: 4px;
}

.nav-link {
  padding: 8px 16px;
  border-radius: var(--radius-sm);
  color: rgba(255, 255, 255, 0.7);
  font-size: var(--font-size-fine-print);
  font-weight: var(--font-weight-regular);
  transition: all 0.2s;
}

.nav-link:hover {
  color: var(--color-navbar-text);
  background: rgba(255, 255, 255, 0.1);
}

.nav-link.active {
  color: var(--color-navbar-text);
  background: var(--color-accent);
}
</style>
