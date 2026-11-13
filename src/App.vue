<script setup lang="ts">
import { RouterView, START_LOCATION, useRouter } from 'vue-router'
import NavHeader from '@/components/NavHeader.vue'
import BottomTabBar from '@/components/BottomTabBar.vue'

// 全局样式由 main.ts 引入一次(此前这里再 @import 一遍,开发态会重复注入)

/*
 * 页面切换的滚动位置:新页插入 DOM 的那一刻(transition enter 钩子)同步滚到目标,
 * 早于 keep-alive 页面的 onActivated —— 那些页面(历史/关注)在 onActivated 里读
 * window.scrollY 作为自己的位置,必须先让它读到正确值。
 *   - 返回/前进:vue-router 在离开时把滚动写进 history.state.scroll,原样恢复;
 *   - 新打开:回到顶部(此前会停在上一页被 clamp 后的随机位置);
 *   - 首页/搜索等自管滚动的页面随后在 onActivated 的 nextTick 里覆盖,行为不变。
 * 同一路由记录内的参数/query 变化不经过页面过渡,滚动交给页面自己(保持原状)。
 */
const router = useRouter()
let pendingScroll: { left: number; top: number } | null = null

router.afterEach((to, from, failure) => {
  if (failure) return
  const toRecord = to.matched[to.matched.length - 1]
  const fromRecord = from.matched[from.matched.length - 1]
  if (from === START_LOCATION || !toRecord || toRecord === fromRecord) {
    pendingScroll = null
    return
  }
  const saved = (window.history.state as { scroll?: { left?: number; top?: number } | null } | null)
    ?.scroll
  pendingScroll =
    saved && typeof saved.top === 'number'
      ? { left: saved.left ?? 0, top: saved.top }
      : { left: 0, top: 0 }
})

function onPageEnter() {
  if (!pendingScroll) return
  const { left, top } = pendingScroll
  pendingScroll = null
  window.scrollTo(left, top)
}
</script>

<template>
  <NavHeader />
  <main class="main-content">
    <RouterView v-slot="{ Component }">
      <!-- 不用 out-in:旧页 leave 即 display:none(见 global.css),新页立刻进场不再等待 -->
      <transition name="page" @enter="onPageEnter">
        <keep-alive include="HomePage,SearchPage,ComicDetailPage,FollowingPage,HistoryPage">
          <component :is="Component" />
        </keep-alive>
      </transition>
    </RouterView>
  </main>
  <BottomTabBar />
</template>
