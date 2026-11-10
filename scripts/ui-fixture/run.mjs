// UI 夹具断言:构建夹具页 → 本地静态服务 → 无头 Chromium 按 400px / 1200px 两档渲染 →
// 读取 #result 的 DOM 度量做断言。只断言结构/计数/尺寸,不加载任何站点内容。
// 用法:node scripts/ui-fixture/run.mjs   (CHROMIUM=可执行文件路径 可覆盖浏览器)
import { build } from 'vite'
import { createServer } from 'node:http'
import { readFile } from 'node:fs/promises'
import { extname, join } from 'node:path'
import { execFile } from 'node:child_process'
import { fileURLToPath } from 'node:url'
import { mkdtemp, rm } from 'node:fs/promises'
import { tmpdir } from 'node:os'

const here = fileURLToPath(new URL('.', import.meta.url))
const dist = fileURLToPath(new URL('../../.scratch/ui-fixture-dist', import.meta.url))
const chromium = process.env.CHROMIUM || 'chromium-browser'

await build({ configFile: join(here, 'vite.config.mjs') })

const types = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css' }
const server = createServer(async (req, res) => {
  const path = join(dist, decodeURIComponent((req.url || '/').split('?')[0]).replace(/^\/$/, '/index.html'))
  try {
    const body = await readFile(path)
    res.writeHead(200, { 'content-type': types[extname(path)] || 'application/octet-stream' })
    res.end(body)
  } catch {
    res.writeHead(404)
    res.end()
  }
})
await new Promise((r) => server.listen(0, '127.0.0.1', r))
const port = server.address().port

async function render(width, path = 'index.html') {
  const profile = await mkdtemp(join(tmpdir(), 'ui-fixture-'))
  try {
    const dom = await new Promise((resolve, reject) =>
      execFile(
        chromium,
        [
          '--headless=new',
          '--no-sandbox',
          '--disable-gpu',
          `--window-size=${width},2400`,
          `--user-data-dir=${profile}`,
          '--virtual-time-budget=10000',
          '--dump-dom',
          `http://127.0.0.1:${port}/${path}${path.includes('?') ? '&' : '?'}w=${width}`,
        ],
        { maxBuffer: 64 * 1024 * 1024, timeout: 90000 },
        (err, stdout) => (err ? reject(err) : resolve(stdout)),
      ),
    )
    const m = /<pre id="result">([^<]*)<\/pre>/.exec(dom)
    if (!m) throw new Error(`宽度 ${width}:未取到 #result`)
    return JSON.parse(m[1].replace(/&quot;/g, '"').replace(/&amp;/g, '&'))
  } finally {
    await rm(profile, { recursive: true, force: true })
  }
}

const failures = []
function expect(width, label, actual, expected) {
  const ok = typeof expected === 'function' ? expected(actual) : actual === expected
  if (!ok) failures.push(`[${width}px] ${label}: 实际 ${JSON.stringify(actual)}`)
}

for (const width of [400, 1200]) {
  const r = await render(width)
  const narrow = width === 400
  console.log(`[${width}px] 实际视口宽度 ${r.width}`)
  // 漫画网格:8 卡、6→2 列断点、已读 2、外链保留(中键/新窗口)
  expect(width, '漫画卡片数', r['comic-grid'].cards, 8)
  expect(width, '漫画列数', r['comic-grid'].columns, narrow ? 2 : 5)
  expect(width, '漫画已读角标', r['comic-grid'].viewed, 2)
  expect(width, '漫画外链 href(含 categoryId)', r['comic-grid'].hrefs[0], '/comic/niacg/9/c0')
  expect(width, '漫画封面 5:7', r['comic-grid'].coverRatio, (v) => Math.abs(v - 1.4) < 0.05)
  // 视频网格:中字 2、无码 1、已看 1、16:9
  expect(width, '视频卡片数', r['video-grid'].cards, 6)
  expect(width, '视频中字角标', r['video-grid'].toneCs, 2)
  expect(width, '视频无码角标', r['video-grid'].toneLeak, 1)
  expect(width, '视频已看', r['video-grid'].viewed, 1)
  expect(width, '视频时长徽标(空时长隐藏)', r['video-grid'].durations, 3)
  expect(width, '视频播放量遮罩(每卡一个)', r['video-grid'].overlayStats, 6)
  expect(width, '时长与播放量遮罩不重叠', r['video-grid'].overlayOverlap, false)
  expect(width, '视频封面 16:9', r['video-grid'].coverRatio, (v) => Math.abs(v - 0.56) < 0.03)
  expect(width, '视频列数', r['video-grid'].columns, (v) => (narrow ? v === 2 : v >= 5))
  // 小说:无封面占位 5、榜单序号 5
  expect(width, '小说占位', r['novel-grid'].placeholders, 5)
  expect(width, '小说榜单序号', r['novel-grid'].toneAccent, 5)
  expect(width, '小说统计项间距', r['novel-grid'].statGap, (v) => v >= 4)
  expect(width, '小说统计图标(阅读/收藏)', (r['novel-grid'].statIcons || []).join(','), 'eye,heart')
  expect(width, '漫画点赞图标', (r['comic-grid'].statIcons || []).join(','), 'heart')
  // 行式:6 行单列、时间、进度文案(坏 JSON 容错不出文案)
  expect(width, '行式条数', r.row.cards, 6)
  expect(width, '行式单列', r.row.columns, 1)
  expect(width, '行式时间(书架条目无时间)', r.row.times, 5)
  expect(width, '行式进度文案', r.row.progress.join('|'), '看到 12 分 5 秒|读到第 3 章 第 2 页|读到第 3 章')
  // 状态
  expect(width, '骨架', r.skeleton.skeleton, 12)
  expect(width, '骨架时无卡片', r.skeleton.cards, 0)
  expect(width, '空态', r.empty.empty, 1)
  expect(width, '整块错误', r['error-full'].empty, 1)
  expect(width, '错误条', r['error-bar'].errorBar, 1)
  expect(width, '按钮翻页', r['button-pager'].moreBtn, 1)
  expect(width, '按钮翻页不挂哨兵', r['button-pager'].sentinelShown, false)
  expect(width, '无限滚动哨兵显示', r.infinite.sentinelShown, true)
  // 任何区块不得横向溢出
  for (const id of ['comic-grid', 'video-grid', 'novel-grid', 'row']) {
    expect(width, `${id} 无横向溢出`, r[id].overflowX, false)
  }
}

// —— 页面级:设置页(黑名单)/关注页,模拟 IPC 假数据(超长标签/作者名) ——
for (const width of [400, 1200]) {
  const b = await render(width, 'pages.html?page=blacklist')
  expect(width, '设置页无横向溢出(超长黑名单词)', b.docOverflow, false)
  expect(width, '设置页溢出元素', b.offenders.join(','), '')
  expect(width, '黑名单网格不嵌套(两个独立区块)', b.chains.length, 2)
  const f = await render(width, 'pages.html?page=following')
  expect(width, '关注页无横向溢出(超长作者名)', f.docOverflow, false)
  const groups = Object.values(f.groups || {})
  expect(width, '关注页三组均有对象芯片', groups.length === 3 && groups.every((g) => g.chips > 0), true)
  expect(width, '关注页三组芯片样式一致', new Set(groups.flatMap((g) => g.sig)).size, 1)
  expect(width, '关注页三组都能 × 取消关注', groups.every((g) => g.removable > 0), true)
  expect(width, '关注页三组默认选中一项', groups.every((g) => g.active === 1), true)
  expect(width, '关注页旧行式/旧芯片残留', groups.reduce((n, g) => n + g.legacyRows, 0), 0)
}

server.close()
if (failures.length) {
  console.error(`UI 夹具断言失败 ${failures.length} 项:\n` + failures.join('\n'))
  process.exit(1)
}
console.log('UI 夹具断言全部通过(400px / 1200px)')
