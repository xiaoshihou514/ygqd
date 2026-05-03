import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueDevTools from 'vite-plugin-vue-devtools'

const KOTLIN_BACKEND = process.env.KOTLIN_BACKEND || 'http://localhost:8080'

export default defineConfig({
  plugins: [vue(), vueDevTools()],
  server: {
    proxy: {
      '/api': {
        target: KOTLIN_BACKEND,
        changeOrigin: true,
      },
    },
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
})
