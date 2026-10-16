<script setup lang="ts">
</script>

<template>
  <header class="nav-header">
    <div class="nav-inner">
      <RouterLink to="/" class="nav-logo">
        <span class="logo-icon">摇</span>
        <span class="logo-text">摇杆驱动</span>
        <span class="logo-marks" aria-hidden="true">
          <span class="src-dot niacg" />
          <span class="src-dot bika" />
        </span>
      </RouterLink>
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
  z-index: 90;
  pointer-events: none;
}

.nav-inner {
  max-width: var(--max-width);
  margin: 0 auto;
  padding: 0 var(--spacing-md);
  height: 100%;
  display: flex;
  align-items: center;
  pointer-events: auto;
}

.nav-logo {
  display: flex;
  align-items: center;
  gap: var(--spacing-xs);
  font-weight: var(--font-weight-bold);
  font-size: var(--font-size-tagline);
  color: var(--color-text-primary);
  letter-spacing: -0.231px;
  text-decoration: none;
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
  color: #fff;
}

.logo-marks {
  display: inline-flex;
  gap: 3px;
  align-items: center;
  margin-left: 2px;
}

.logo-marks .src-dot {
  width: 5px;
  height: 5px;
}
</style>
