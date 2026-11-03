import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import vueDevTools from 'vite-plugin-vue-devtools'

// Video.js 10(@videojs/html)自定义元素,模板中不作为 Vue 组件解析
const videoJsElements = new Set(['video-player', 'video-skin', 'hlsjs-video', 'media-i18n'])

export default defineConfig({
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => videoJsElements.has(tag),
        },
      },
    }),
    vueDevTools(),
  ],
  server: {
    watch: {
      ignored: ['**/src-tauri/**', '**/.scratch/**'],
    },
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url)),
    },
  },
})
