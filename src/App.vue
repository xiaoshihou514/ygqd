<script setup lang="ts">
import { RouterView } from 'vue-router'
import NavHeader from '@/components/NavHeader.vue'
</script>

<template>
  <NavHeader />
  <main class="main-content">
    <RouterView v-slot="{ Component }">
      <transition name="page" mode="out-in">
        <component :is="Component" />
      </transition>
    </RouterView>
  </main>
</template>

<style>
@import '@/styles/global.css';
</style>
