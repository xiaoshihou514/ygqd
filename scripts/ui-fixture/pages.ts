// 页面级夹具:模拟 Tauri IPC(假数据),渲染真实页面,测量横向溢出与交互控件形态。
// ?page=blacklist|following;只断言结构与尺寸,不加载任何站点内容。
import { createApp, h } from 'vue'
import { createMemoryHistory, createRouter } from 'vue-router'
import '@/styles/global.css'

const LONG = '超长标签词超长标签词超长标签词超长标签词超长标签词超长标签词'
const mock: Record<string, unknown> = {
  fetch_blacklist: [
    { tag: '短', mode: 'hide' },
    { tag: LONG, mode: 'blur' },
    { tag: 'averyveryverylongasciitagwithoutanyspacesatall_xxxxxxxxxxxxxxxx', mode: 'hide' },
    ...Array.from({ length: 6 }, (_, i) => ({ tag: `标签${i}`, mode: 'hide' })),
  ],
  bika_status: { loggedIn: false, email: '' },
  bika_address: '',
  bika_addresses: [],
  diag_read: '',
  fetch_followed_authors: [{ author: LONG, followedAt: 0, lastCheckedAt: 0 }, { author: '作者乙', followedAt: 0, lastCheckedAt: 0 }],
  fetch_follows: [{ author: LONG, followedAt: 0, lastCheckedAt: 0 }, { author: '作者乙', followedAt: 0, lastCheckedAt: 0 }],
  video_follow_works: { items: [], pagination: { hasNext: false, page: 1 } },
  video_follow_list: [{ source: 'missav', kind: 'actress', targetId: LONG, displayName: LONG, followedAt: 0 }],
  video_follow_targets: [],
}
;(window as any).__TAURI_INTERNALS__ = {
  invoke: async (cmd: string) => (cmd in mock ? structuredClone(mock[cmd]) : Array.isArray(mock[cmd]) ? [] : null),
  transformCallback: () => 0,
  metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
}
;(window as any).__TAURI_INTERNALS__.invoke = new Proxy((window as any).__TAURI_INTERNALS__.invoke, {
  apply: async (t, _this, args) => {
    const cmd = args[0] as string
    if (!(cmd in mock)) (window as any).__unknownCmds = [...((window as any).__unknownCmds || []), cmd]
    return t(...args)
  },
})

const which = new URLSearchParams(location.search).get('page') || 'blacklist'
const wantWidth = Number(new URLSearchParams(location.search).get('w') || 0)
const framed = wantWidth > 0 && window.innerWidth !== wantWidth

async function start() {
  if (framed) {
    const frame = document.createElement('iframe')
    frame.src = `${location.pathname}?page=${which}`
    frame.style.cssText = `width:${wantWidth}px;height:3000px;border:0`
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
    return
  }
  const Page =
    which === 'following'
      ? (await import('@/pages/FollowingPage.vue')).default
      : (await import('@/pages/TagBlacklistPage.vue')).default
  const router = createRouter({ history: createMemoryHistory(), routes: [{ path: '/:p(.*)*', component: { render: () => null } }] })
  const app = createApp({ render: () => h(Page) })
  app.use(router)
  app.mount('#app')
  setTimeout(() => void (which === 'following' ? measureTabs() : measure()), 2500)
}

/** 关注页:逐个切 tab,记录每组的对象芯片数量与样式签名(统一性断言) */
async function measureTabs() {
  const groups: Record<string, unknown> = {}
  const tabs = Array.from(document.querySelectorAll('#app [role="tab"].content-tab')) as HTMLElement[]
  for (const t of tabs) {
    t.click()
    await new Promise((r) => setTimeout(r, 1200))
    const chips = Array.from(document.querySelectorAll('#app .follow-chip')) as HTMLElement[]
    const sig = [...new Set(chips.map((c) => { const cs = getComputedStyle(c); return `${Math.round(c.getBoundingClientRect().height)}|${cs.borderRadius}|${cs.fontSize}` }))]
    groups[t.textContent!.trim()] = {
      chips: chips.length,
      removable: document.querySelectorAll('#app .follow-chip .follow-chip-remove').length,
      active: document.querySelectorAll('#app .follow-chip.active').length,
      sig,
      legacyRows: document.querySelectorAll('#app .follow-row, #app .author-chip').length,
    }
  }
  ;(window as any).__groups = groups
  // 先测量再挂 groups-result:不折行的 <pre> 长 JSON 会把文档撑宽,污染 docOverflow
  measure()
  const gpre = document.createElement('pre')
  gpre.id = 'groups-result'
  gpre.textContent = JSON.stringify(Object.values(groups))
  document.body.appendChild(gpre)
}

/** 页面内任何元素超出视口右缘(含被 overflow 隐藏前的内容宽度)即记为溢出 */
function measure() {
  const vw = document.documentElement.clientWidth
  const offenders: string[] = []
  for (const el of Array.from(document.querySelectorAll('#app *'))) {
    const r = (el as HTMLElement).getBoundingClientRect()
    if (r.width === 0) continue
    if (r.right > vw + 1 || r.left < -1) {
      const cls = (el as HTMLElement).className
      offenders.push(`${el.tagName.toLowerCase()}.${typeof cls === 'string' ? cls.split(' ')[0] : ''}@${Math.round(r.right)}`)
    }
  }
  // 交互控件形态:关注页各组的切换/选择控件类名与尺寸(统一性断言用)
  const controls = Array.from(document.querySelectorAll('#app [role="tab"], #app [class*="chip"], #app [class*="tab"]'))
    .filter((e) => (e as HTMLElement).offsetWidth > 0 && e.tagName === 'BUTTON')
    .map((e) => {
      const cs = getComputedStyle(e as HTMLElement)
      return { cls: (e as HTMLElement).className.split(' ')[0], h: Math.round((e as HTMLElement).getBoundingClientRect().height), radius: cs.borderRadius, font: cs.fontSize }
    })
  const pre = document.createElement('pre')
  pre.id = 'result'
  pre.textContent = JSON.stringify({
    width: vw,
    docOverflow: document.documentElement.scrollWidth > vw + 1,
    offenders: [...new Set(offenders)].slice(0, 30),
    controls,
    chains: Array.from(document.querySelectorAll('.tag-grid')).map((g) => { const out: string[] = []; let e: HTMLElement | null = g as HTMLElement; while (e && e.id !== 'app') { out.push(`${e.className.split(' ')[0] || e.tagName}:${Math.round(e.getBoundingClientRect().width)}/${getComputedStyle(e).display}`); e = e.parentElement } return out.join(' < ') }),
    groups: (window as any).__groups,
    unknownCmds: [...new Set((window as any).__unknownCmds || [])],
  })
  document.body.appendChild(pre)
}

void start()
