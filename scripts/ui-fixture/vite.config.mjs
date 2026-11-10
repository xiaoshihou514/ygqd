// UI 夹具页构建配置:真实组件 + 假数据,供 run.mjs 在无头 Chromium 下做 DOM 断言
import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  root: fileURLToPath(new URL('.', import.meta.url)),
  base: './',
  plugins: [vue()],
  resolve: { alias: { '@': fileURLToPath(new URL('../../src', import.meta.url)) } },
  build: { rollupOptions: { input: { index: fileURLToPath(new URL('./index.html', import.meta.url)), pages: fileURLToPath(new URL('./pages.html', import.meta.url)) } }, outDir: fileURLToPath(new URL('../../.scratch/ui-fixture-dist', import.meta.url)), emptyOutDir: true },
  logLevel: 'warn',
})
