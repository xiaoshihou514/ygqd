import { createRouter, createWebHistory } from 'vue-router'

// 路由组件保持懒加载;高频入口的加载器单独命名,供首屏后空闲预取复用(同一 chunk)
const SearchPage = () => import('@/pages/SearchPage.vue')
const ComicDetailPage = () => import('@/pages/ComicDetailPage.vue')
const SettingsPage = () => import('@/pages/TagBlacklistPage.vue')
const VideoPage = () => import('@/pages/VideoPage.vue')
const FollowingPage = () => import('@/pages/FollowingPage.vue')
const HistoryPage = () => import('@/pages/HistoryPage.vue')

const router = createRouter({
  history: createWebHistory(import.meta.env.BASE_URL),
  routes: [
    {
      path: '/',
      name: 'home',
      component: () => import('@/pages/HomePage.vue'),
    },
    {
      path: '/search',
      name: 'search',
      component: SearchPage,
    },
    {
      path: '/comic/:source/:categoryId/:id',
      name: 'comic-detail',
      component: ComicDetailPage,
    },
    {
      path: '/split',
      name: 'split',
      component: () => import('@/pages/SplitPage.vue'),
    },
    {
      path: '/settings',
      name: 'settings',
      component: SettingsPage,
    },
    {
      path: '/settings/blacklist',
      redirect: '/settings',
    },
    {
      path: '/videos',
      name: 'videos',
      component: VideoPage,
    },
    {
      path: '/videos/:code',
      name: 'video-detail',
      component: () => import('@/pages/VideoDetailPage.vue'),
    },
    {
      path: '/videos/target/:source/:kind/:targetId',
      name: 'video-target-works',
      component: () => import('@/pages/TargetWorksPage.vue'),
    },
    {
      path: '/following',
      name: 'following',
      component: FollowingPage,
    },
    {
      path: '/history',
      name: 'history',
      component: HistoryPage,
    },
  ],
})

/*
 * 首屏就绪后在空闲时预取底部 tab 页与漫画详情:首次切 tab / 打开作品不再等 chunk 解析。
 * 不预取视频详情(含播放器,体积最大,按需加载)。失败静默,真正导航时会重新加载。
 */
function prefetchCommonPages() {
  for (const load of [SearchPage, FollowingPage, HistoryPage, SettingsPage, ComicDetailPage, VideoPage]) {
    load().catch(() => {})
  }
}

if (typeof window !== 'undefined') {
  // 滚动恢复由 App.vue 在页面进场时接管:关掉浏览器自动恢复(它在旧页未移除时就动手,
  // 位置被 clamp),且 vue-router 只在 manual 下才把离开时的位置写进 history.state.scroll
  if ('scrollRestoration' in window.history) window.history.scrollRestoration = 'manual'

  void router.isReady().then(() => {
    if ('requestIdleCallback' in window) {
      window.requestIdleCallback(prefetchCommonPages)
    } else {
      // WebKitGTK(Linux 桌面)无 requestIdleCallback:退化为首屏渲染后的下一个宏任务
      setTimeout(prefetchCommonPages)
    }
  })
}

export default router
