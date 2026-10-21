# 内容板块扩展规划：小说 + 视频

## 零、源能力侦察结论

| 源 | 小说 | 视频 |
|---|---|---|
| niacg | **未发现**（首页导航无小说入口） | **有**：「视频在线资源平台」，搜索模板 `tempid=7`（漫画为 `tempid=1`），另有 AI 图生视频工具页 |
| bika | 无 API（pikapika/PicaComic 均无 novel 端点） | **有**：games 区（`GET /games`），GameInfo 含 `videoLink`、`androidLinks`（APK 下载）、截图——实为"视频+应用资源"混合区 |

> 小说板块目前两个源都没有现成内容，需要确认接入哪个新源（见「待决策」）。

## 一、核心概念：ContentType（内容类型）

与 `Source`（源）正交的第二个维度：

```
ContentType = 'comic' | 'video' | 'novel'
```

**能力矩阵**（前端按此渲染入口，矩阵声明在 `config/sources.ts`）：

```ts
// 源 × 内容类型 能力
export const CAPABILITIES: Record<SourceId, ContentType[]> = {
  niacg: ['comic', 'video'],
  bika: ['comic', 'video'],   // video = games 区
}
```

- `HomeTab` 增加 `contentType` 字段；tab 状态机从「源分类」升级为「内容类型 × 源分类」
- 路由参数带上类型：`/video/:source/:categoryId/:id`、`/novel/:source/:categoryId/:id`（漫画保持 `/comic/...` 不动，向后兼容）

## 二、导航与信息架构（本规划最关键的决策）

底部导航已满 5 个（首页/搜索/关注/历史/设置），**不动**。在首页顶部加**内容类型分段控件**：

```
┌─────────────────────────────┐
│  [漫画] [视频] [小说]        ← 新增：内容分段(二级导航)
│  ─────────────────────────  │
│  最新 漫画 COS 日榜 ...      ← 现有 tab 组(随分段整体切换)
│                             │
│  (内容网格)                  │
└─────────────────────────────┘
```

选择理由（对比过的备选）：
- ✅ 分段控件：入口集中在首页、拇指可达、`HomeTab` 状态机自然扩展（每段独立的 tab 组与滚动位置）、底部导航零改动
- ❌ tab 平铺追加：日榜+分类已经 7+ 个 tab，再加会溢出滚动
- ❌ 底部新 tab：需要合并现有页面（如历史并入关注），破坏用户习惯，改动面大

分段切换时各段独立记忆 tab 选择与滚动位置（复用现有 `stateFor(key)` 模式，key 加前缀 `v:`/`n:`）。

## 三、页面与路由

| 路由 | 页面 | 说明 |
|---|---|---|
| `/` + 分段 | 首页 | 三段内容各自 tab 组 |
| `/video/:source/:categoryId/:id` | **视频详情页（新）** | 见下 |
| `/novel/:source/:categoryId/:id` | **小说详情页（新）** | 章节列表 + 简介 |
| `/novel/:source/:categoryId/:id?ch=N` | **阅读器（新）** | 正文阅读，进度记忆 |
| `/search` | 搜索（扩展） | `SearchParams` 加 `contentType`，niacg 视频搜索走 `tempid=7` |

**视频详情页**（分两种源形态，同一个页面组件内分支渲染）：
- niacg 视频：封面 + 标题 + 标签 + 内嵌 `<video>` 播放器（若为嵌入页则 WebView/外跳）
- bika game 资源：截图轮播 + 介绍 + `videoLink` 播放 + 下载链接列表（androidLinks 跳转系统浏览器）
- 复用：详情页骨架、ProxyImage、标签行、收藏/点赞按钮（收藏表已支持跨内容）

**阅读器**（工作量最大的新组件）：
- 章节目录抽屉（右滑/按钮呼出）+ 正文分页渲染（CSS 多栏或按段落虚拟滚动）
- 设置面板：字号/行距/背景（纸白/暗黑/护眼绿），设置持久化到 SQLite properties
- 阅读进度：`view_history` 扩展记录章节号，详情页「继续阅读」入口

## 四、组件规划

**复用（零改动或小改）**：
- `ComicGrid`/`ComicCard`：视频封面卡直接适用（加时长/类型角标 props）
- `SearchForm`、`ProxyImage`、`EmptyState`、`Pagination`、黑名单过滤

**新增**：
| 组件 | 职责 | 预估 |
|---|---|---|
| `ContentTypeSegmented.vue` | 首页顶部分段控件 | 小 |
| `VideoPlayer.vue` | HTML5 video 封装（直链播放、HLS 按需引入 hls.js） | 中 |
| `ScreenshotCarousel.vue` | bika game 截图轮播 | 小 |
| `NovelReader.vue` | 章节阅读器（分页/设置/进度） | **大** |

## 五、数据层（前端视角）

- TS 类型：`ContentType`、`VideoItem`、`NovelDetail`（章节列表）、`NovelChapter`（正文）
- `api.ts` 新增封装：`fetchVideoList`/`fetchVideoDetail`/`fetchGames`(bika)/`fetchNovel...`
- 搜索类型 `SearchParams.contentType`，后端 niacg 分支按类型选 `tempid`（1/7）
- 收藏/历史加 `contentType` 维度（列表渲染按类型路由到对应详情页）

## 六、分期实施

| 期 | 内容 | 依赖 |
|---|---|---|
| P1 | ContentType 概念 + 首页分段 + bika games 区接入（列表/详情/播放） | 无，可立即开始 |
| P2 | niacg 视频接入 + VideoPlayer 打磨 | 需应用内探测 niacg 视频页结构（CF 拦截裸 curl，要用带 cookie 的共享 client 抓页面分析 selector） |
| P3 | 小说全链路（详情/阅读器/进度）+ 搜索/收藏打通 | **阻塞：小说源未定** |

## 八、实施侦察结论（已定案）

### 视频板块（已完成一期,提交 1bb862d）
- **fs1.app（Jable 系）**：服务端渲染,reqwest 直连可达。列表 `/new-release/{p}/`、搜索 `/search/{kw}/{p}/`、详情 `/videos/{番号}/` 均 HTML 直出；m3u8 内嵌详情页（hlsUrl）,CDN 无挑战可直接拉流。已实现后端 + 前端全链路。
- **missav.ai**：CF 按 TLS 指纹拦截（reqwest 403,curl_cffi chrome 指纹 200）；列表页 JS 渲染拿不到,但番号直达 `/videos/{番号}` → 302 `/dm*/{番号}`,eval packer 里有三档清晰度 m3u8（surrit.com CDN）。**missav 播放增强走 WebView 网关**（见下）。

### 小说板块（hlib.cc）
- 全站 CF managed challenge + Turnstile 交互验证：curl_cffi(chrome 指纹)、playwright headless、playwright 有头+自动点击均被拒。**必须真浏览器人工通过一次**。
- **定案：WebView 网关架构**——应用内嵌隐藏 WebView 加载 hlib,首次由用户点一次挑战,Android WebView cookie 持久化；此后所有抓取在该 webview 内执行 fetch(JS 网络栈 = 真 Chrome,永不再挑战),结果经 invoke 桥回传 Rust。missav 的番号直达播放同享此网关。
- 实现要点:Tauri 2 WebviewWindow(隐藏) + initialization_script 注入桥 + 共享 invoke_handler + on_page_load 事件。

## 七、待决策（需要你确认）

1. **小说源**：两个现有源都没有小说内容。接哪个站？（若是某个新站点，给我地址，我评估 HTML/API 可行性）
2. **bika games 区的定位**：它是"视频 + App 下载"混合区。只搬视频部分，还是整个 games 区都进视频板块（含下载链接）？
3. **niacg 视频**：需要一次应用内探测（手机上打开视频板块确认 URL 形态），或者你直接告诉我板块入口路径


## 九、实施完成状态(截至 714bc9b)

小说(hlib.cc)与视频(missav 优先/fs1 回退)已全部实现并推送,流水线双绿。

| 模块 | 实现 | 验证 |
|---|---|---|
| 视频列表/搜索 | fs1 HTML 解析(标题 h6/中字去重) | 联调实测 |
| 视频播放 | stream:// 流代理(CDN Referer/TLS 双拦的解法),hls.js 统一 | 单测+联调+wry 源码确认 secure scheme |
| missav 优先 | 网关 eval packer 解码(中字变体优先),失败回退 fs1 | headless chromium 端到端 |
| 小说网关 | WebView 常驻会话(关闭改隐藏),挑战自动弹回 | 编译+单测 |
| 小说搜索/发现 | 端点自适应探测 + 首页通用提取 | 编译(待真实会话验证) |
| 书架/阅读器 | SQLite 存储+进度记忆,章节抽屉/翻页/续读 | 编译+单测 |

待办:用户端到端实测确认(桌面端/Android nightly)。
