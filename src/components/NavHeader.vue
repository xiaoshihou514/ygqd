<script setup lang="ts">
</script>

<template>
  <header class="nav-header">
    <div class="nav-inner">
      <RouterLink to="/" class="nav-logo" aria-label="摇杆驱动 首页">
        <span class="logo-icon" aria-hidden="true">摇</span>
        <span class="logo-text">摇杆驱动</span>
      </RouterLink>
    </div>
  </header>
</template>

<style scoped>
/* 毛玻璃顶栏:与 BottomTabBar 同一材质,内容滚到下方时不再与 logo 叠字 */
.nav-header {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  height: var(--nav-total-height);
  padding-top: var(--nav-safe-top);
  z-index: var(--z-nav);
  background: var(--color-tab-bar-bg);
  -webkit-backdrop-filter: saturate(180%) blur(20px);
  backdrop-filter: saturate(180%) blur(20px);
  border-bottom: 1px solid var(--color-hairline);
}

.nav-inner {
  max-width: var(--max-width);
  margin: 0 auto;
  padding: 0 max(var(--spacing-md), env(safe-area-inset-right, 0px)) 0 max(var(--spacing-md), env(safe-area-inset-left, 0px));
  height: 100%;
  display: flex;
  align-items: center;
}

/* 热区撑满栏高(44px),按压轻收缩 */
.nav-logo {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  min-width: 0;
  height: 100%;
  font-weight: var(--font-weight-bold);
  font-size: var(--font-size-tagline);
  line-height: var(--line-height-heading);
  color: var(--color-text-primary);
  letter-spacing: -0.231px;
  text-decoration: none;
  -webkit-tap-highlight-color: transparent;
  transition: transform var(--duration-fast) var(--ease-standard);
}

.nav-logo:active {
  transform: scale(var(--press-scale));
}

.nav-logo:focus-visible {
  outline: 2px solid var(--color-primary-focus);
  outline-offset: 2px;
  border-radius: var(--radius-sm);
}

.logo-icon {
  flex-shrink: 0;
  width: 32px;
  height: 32px;
  background: var(--color-primary);
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: var(--font-size-body);
  font-weight: var(--font-weight-bold);
  color: var(--color-on-primary);
}

.logo-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
