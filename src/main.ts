import { createApp } from 'vue'
import App from './App.vue'
import router from './router'
import './styles/global.css'

const app = createApp(App)
app.use(router)
app.mount('#app')

if (typeof window !== 'undefined') {
  ;(window as any).__VUE_ROUTER__ = router
}
