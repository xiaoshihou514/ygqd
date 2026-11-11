// UI 夹具:用假数据渲染 MediaCollection/MediaCard 的各种形态,并把 DOM 度量写进 #result。
// 只断言结构与尺寸,不加载任何站点内容(封面地址指向不存在的本地路径,ProxyImage 失败后走占位)。
import { createApp, h, nextTick } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'
import '@/styles/global.css'
import MediaCollection from '@/components/MediaCollection.vue'
import {
  fromComic,
  fromFavorite,
  fromTimeline,
  fromVideo,
  type MediaItem,
} from '@/utils/mediaItem'

const comics: MediaItem[] = Array.from({ length: 8 }, (_, i) =>
  fromComic(
    {
      source: i % 2 ? 'bika' : 'niacg',
      id: `c${i}`,
      title: `占位漫画标题 ${i}`,
      thumbnail: '',
      category: '分类',
      categoryId: 9,
      tags: ['标签一', '标签二', '标签三', '标签四'],
      likes: i % 2 ? '1.2万' : '1,288',
      link: '',
    },
    { viewed: i < 2 },
  ),
)
const videos: MediaItem[] = [
  'abc-001-chinese-subtitle',
  'abc-002',
  'abc-003-uncensored-leak',
  'abc-004-chinese-subtitle',
  'abc-005',
  'abc-006',
].map((code, i) =>
  fromVideo(
    { code, title: `占位影片标题 ${i}`, thumbnail: '/__none__.jpg', views: '315 903', likes: '', chinese: code.includes('chinese'), duration: i % 2 ? '2:51:49' : '' },
    { watched: i === 1 },
  ),
)
const now = Date.now()
const timeline: MediaItem[] = [
  fromTimeline({ kind: 'comic', source: 'niacg', itemId: 'c1', title: '历史漫画', viewedAt: now, categoryId: 3 }),
  fromTimeline({ kind: 'video', source: 'fs1', itemId: 'abc-001', title: '历史影片', viewedAt: now - 1000, progress: '{"position":725}' }),
  fromTimeline({ kind: 'video', source: 'fs1', itemId: 'abc-009', title: '坏进度', viewedAt: now - 3000, progress: 'not-json' }),
  fromFavorite({ source: 'bika', comicId: 'f1', title: '收藏漫画', thumbnail: '', categoryId: 5, author: '', createdAt: now }),
]

const sections: Array<{ id: string; props: Record<string, unknown> }> = [
  { id: 'comic-grid', props: { items: comics, density: 'comic' } },
  { id: 'video-grid', props: { items: videos, density: 'video' } },
  { id: 'row', props: { items: timeline, layout: 'row' } },
  { id: 'skeleton', props: { items: [], loading: true, density: 'comic' } },
  { id: 'empty', props: { items: [], emptyTitle: '空态标题' } },
  { id: 'error-full', props: { items: [], error: '整块错误' } },
  { id: 'error-bar', props: { items: videos.slice(0, 2), error: '部分失败', density: 'video' } },
  { id: 'button-pager', props: { items: videos.slice(0, 2), pager: 'button', hasMore: true, moreLabel: '加载更多(第 2/3 页)' } },
  { id: 'infinite', props: { items: videos.slice(0, 2), pager: 'infinite', hasMore: true } },
]

// 无头 Chromium 窗口最小 500px:窄屏档(?w=400)改在指定宽度的 iframe 内渲染并回传度量
const wantWidth = Number(new URLSearchParams(location.search).get('w') || 0)
const framed = wantWidth > 0 && window.innerWidth !== wantWidth
if (framed) {
  const frame = document.createElement('iframe')
  frame.src = location.pathname
  frame.style.cssText = `width:${wantWidth}px;height:2400px;border:0`
  document.body.appendChild(frame)
  const poll = setInterval(() => {
    const inner = frame.contentDocument?.getElementById('result')
    if (inner) {
      clearInterval(poll)
      const pre = document.createElement('pre')
      pre.id = 'result'
      pre.textContent = inner.textContent
      document.body.appendChild(pre)
    }
  }, 200)
}

const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:p(.*)*', component: { render: () => null } }] })
const app = createApp({
  render: () =>
    sections.map((s) => h('section', { id: s.id, style: 'margin-bottom:24px' }, [h(MediaCollection, s.props)])),
})
app.use(router)
if (!framed) app.mount('#app')

function measure() {
  const out: Record<string, unknown> = { width: window.innerWidth }
  for (const s of sections) {
    const root = document.getElementById(s.id)!
    const grid = root.querySelector('.mc-items') as HTMLElement | null
    const cards = root.querySelectorAll('.media-card')
    const cover = root.querySelector('.media-cover') as HTMLElement | null
    const sentinel = root.querySelector('.mc-sentinel') as HTMLElement | null
    out[s.id] = {
      cards: cards.length,
      columns: grid ? getComputedStyle(grid).gridTemplateColumns.split(' ').filter(Boolean).length : 0,
      skeleton: root.querySelectorAll('.mc-skeleton').length,
      empty: root.querySelectorAll('.empty-state').length,
      errorBar: root.querySelectorAll('.mc-error-bar').length,
      moreBtn: root.querySelectorAll('.mc-more-btn').length,
      sentinelShown: sentinel ? getComputedStyle(sentinel).display !== 'none' : false,
      viewed: root.querySelectorAll('.media-viewed').length,
      toneCs: root.querySelectorAll('.media-badge.tone-cs').length,
      toneLeak: root.querySelectorAll('.media-badge.tone-leak').length,
      toneAccent: root.querySelectorAll('.media-badge.tone-accent').length,
      placeholders: root.querySelectorAll('.media-placeholder').length,
      progress: Array.from(root.querySelectorAll('.media-progress')).map((e) => e.textContent?.trim()),
      times: root.querySelectorAll('.media-time').length,
      hrefs: Array.from(root.querySelectorAll('a.media-hit[href]')).map((a) => a.getAttribute('href')),
      coverRatio: cover ? +(cover.getBoundingClientRect().height / cover.getBoundingClientRect().width).toFixed(2) : 0,
      overflowX: root.scrollWidth > root.clientWidth + 1,
      overlayOverlap: (() => {
        const card = Array.from(root.querySelectorAll('.media-card')).find((c) => c.querySelector('.media-overlay-duration'))
        const a = card?.querySelector('.media-overlay-duration')?.getBoundingClientRect()
        const b = card?.querySelector('.media-overlay-stat')?.getBoundingClientRect()
        if (!a || !b) return null
        return !(a.right <= b.left || b.right <= a.left || a.bottom <= b.top || b.bottom <= a.top)
      })(),
      durations: root.querySelectorAll('.media-overlay-duration').length,
      overlayStats: root.querySelectorAll('.media-overlay-stat').length,
      statGap: (() => {
        const items = root.querySelectorAll('.media-stats .stat-item')
        if (items.length < 2) return -1
        return Math.round(items[1]!.getBoundingClientRect().left - items[0]!.getBoundingClientRect().right)
      })(),
      statIcons: Array.from(root.querySelector('.media-card .media-stats')?.querySelectorAll('svg') ?? []).map((g) => g.getAttribute('data-icon')),
    }
  }
  const pre = document.createElement('pre')
  pre.id = 'result'
  pre.textContent = JSON.stringify(out)
  document.body.appendChild(pre)
}

if (!framed) void nextTick(() => setTimeout(measure, 1500))
