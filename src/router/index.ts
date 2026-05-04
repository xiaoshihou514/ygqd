import { createRouter, createWebHistory } from 'vue-router'

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
      component: () => import('@/pages/SearchPage.vue'),
    },
    {
      path: '/comic/:categoryId/:id',
      name: 'comic-detail',
      component: () => import('@/pages/ComicDetailPage.vue'),
    },
    {
      path: '/split',
      name: 'split',
      component: () => import('@/pages/SplitPage.vue'),
    },
    {
      path: '/settings/blacklist',
      name: 'tag-blacklist',
      component: () => import('@/pages/TagBlacklistPage.vue'),
    },
  ],
})

export default router
