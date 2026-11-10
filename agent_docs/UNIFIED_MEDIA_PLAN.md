# 三板块统一卡片与容器——方案（待审定）

用户原话：「不要搜索、推荐、历史、关注那么多实现」。目标：漫画/视频/小说共用**一种卡片**（MediaCard）+
**一种容器**（MediaCollection），所有「媒体条目列表」场景都接这一套。

## 1. 现状盘点（HEAD 628ec63）

### 卡片层：7 套
| 实现 | 位置 | 用于 |
|---|---|---|
| ComicCard（含长按拉黑） | components/ComicCard.vue | 经 ComicGrid：首页、搜索（漫画）、关注（漫画作者作品）、历史（漫画） |
| VideoCardGrid 内卡片 | components/VideoCardGrid.vue | 搜索（视频）、目标作品页、详情页相关作品、历史（视频） |
| VideoBrowser 内卡片（与上者重复一份） | components/VideoBrowser.vue | 首页视频段、VideoPage |
| NovelSeriesCard | components/NovelSeriesCard.vue | 关注（小说作者作品）、历史（小说）；FollowingPage 内联拼假 NovelSeries 对象喂它 |
| TimelineRow | components/TimelineRow.vue | 历史「全部」 |
| 页面内联行 ×5 | NovelPage `.hit-row`、SearchPage 小说 `.scope-row`、FollowingPage 视频 `.work-row`、HistoryPage 收藏 `.favorite-card`、SplitPage `.item-*` | 推荐/榜单、小说搜索、关注视频作品、收藏、分组 |

### 编排层：每页手写
- 网格 3 套 CSS（ComicGrid 6→2 列断点、VideoCardGrid auto-fill 150px、NovelSeriesCard 所在页各自写）。
- 加载更多：无限滚动 7 处（各自 `document.querySelector('.scroll-sentinel')` 全局选择，keep-alive 页面切换动画期间两份 DOM 共存时存在误选风险）+ 按钮式 3 处（NovelPage、SearchPage 小说、VideoDetail 相关、关注视频作品）。
- 空态/首屏加载/加载更多/末尾提示：每页自己拼 EmptyState + LoadingSpinner + 文案，骨架只有 ComicGrid 有。
- 死代码：Pagination.vue 无引用。

## 2. 目标组件 API

### 2.1 统一条目模型 `MediaItem`（src/utils/mediaItem.ts）
卡片只认这个形态；各数据源经适配函数转换（纯函数，集中维护板块差异）。
```ts
type MediaKind = 'comic' | 'video' | 'novel'
interface MediaItem {
  key: string                    // 列表 key(source:id / code / series id)
  kind: MediaKind
  title: string
  subtitle?: string              // 视频=番号;小说=作者;漫画=作者(有则显示)
  cover?: { src: string; source: SourceId | 'video' } // 无 → 板块占位(小说书本图标+标题)
  badges?: Array<{ text: string; tone: 'accent' | 'cs' | 'leak' | 'neutral' }> // 分类/中字/无码/已看
  tags?: string[]                // 漫画标签/小说标签(卡片最多显示 3–4 个)
  stats?: { likes?: number; views?: number; chapters?: number } // 统一 formatCount 中文格式
  progress?: string              // 「读到第 3 章 第 2 页」「看到 12 分 05 秒」
  time?: number                  // 历史/收藏时间(行式显示)
  viewed?: boolean               // 已看/已读角标
  to: RouteLocationRaw           // 点击跳转
  href?: string                  // 漫画外链(保留中键/新窗口能力)
  blacklistTags?: string[]       // 漫画长按快速拉黑的候选标签
}
fromComic(c, {viewed})  fromVideo(v, {watched})  fromNovelHit(h)  fromNovelSeries(s, progress)
fromTimeline(e)  fromFavorite(f)
```
板块差异全部在适配函数里落定：视频封面走 `source:'video'` 代理、中字/无码角标、漫画分类角标、
小说无封面占位、历史进度文案（原 TimelineRow 的格式化逻辑迁入）。

### 2.2 `MediaCard.vue`
```ts
props: { item: MediaItem; layout?: 'grid' | 'row' }   // 默认 grid
emits: { open: [item]; }                               // 默认行为 router.push(item.to),父组件可拦截
slots: { actions?: { item } }                          // 行尾/卡角操作:取消收藏 ×、+书架 等
```
- grid：封面（比例按 kind：漫画/小说 3:4，视频 16:9）+ 标题两行 + subtitle + tags + stats/progress。
- row：左侧 56px 缩略（或板块图标）+ 标题/副标题/进度 + 右侧时间与 actions —— 取代 TimelineRow、
  hit-row、scope-row、work-row、favorite-card。
- 漫画长按/右键拉黑：抽成 `TagBlacklistSheet.vue`，MediaCard 在 `blacklistTags` 非空时启用（其它板块零代价）。
- 视觉按 DESIGN.md `store-utility-card`：canvas 底、1px hairline、`--radius-lg`、**无阴影**；
  角标/强调色全部走 token（新增 `--color-badge-cs` / `--color-badge-leak`，替换现有 #e8590c/#6741d9 硬编码）。

### 2.3 `MediaCollection.vue`（唯一编排容器）
```ts
props: {
  items: MediaItem[]
  layout?: 'grid' | 'row'
  loading?: boolean          // 首屏:grid 显示骨架卡,row 显示骨架行
  loadingMore?: boolean
  hasMore?: boolean
  error?: string             // 有数据时显示顶部错误条+重试;无数据时整块错误态
  pager?: 'infinite' | 'button' | 'none'   // 默认 infinite;hlib 限速场景用 button
  moreLabel?: string         // 按钮文案(如「加载更多(第 3/336 页)」)
  emptyTitle?: string; emptyMessage?: string
  endText?: string           // 末尾提示,默认「没有更多了」
}
emits: { loadMore: []; retry: []; open: [item] }
slots: { actions?: { item }; empty?: {} }
```
- 内部用 `useInfiniteScroll`，哨兵改为组件内 **模板 ref**（消灭全局 querySelector）；`loadMore` 只发事件，
  节流/限速仍由页面掌控（hlib 3 秒闸、视频去重合并逻辑不动）。
- 网格列宽统一：`grid-template-columns: repeat(auto-fill, minmax(var(--media-min, 160px), 1fr))`，
  视频 min 180px（16:9）、漫画/小说 150px；手机 2 列、平板 3–4 列、桌面 5–6 列，与现在接近。

## 3. 迁移清单（全部场景 → MediaCollection）
| 场景 | 现实现 | 迁移后 | 备注 |
|---|---|---|---|
| 首页漫画 | ComicGrid + 手写哨兵 | grid/infinite | viewedIds、星标筛选、滚动恢复保留 |
| 首页视频段 / VideoPage | VideoBrowser（自带抓取） | 抓取逻辑抽 `useVideoFeed`，展示 grid/infinite | 已看集合保留 |
| 搜索·漫画 | ComicGrid + 本地分页 visibleCount | grid/infinite | 本地分页逻辑不动，只换展示；分组页 split_items 仍存 ComicItem[] |
| 搜索·视频 | VideoCardGrid + 哨兵 | grid/infinite | |
| 搜索·小说 | 内联 scope-row + 按钮 | grid/button | hlib 限速保留按钮式 |
| 小说推荐/日榜/月榜 | 内联 hit-row + 哨兵 | grid/infinite（当前已是 infinite，保持） | 榜单序号走 badge |
| 关注·漫画作者作品 | ComicGrid | grid/infinite | |
| 关注·视频对象作品 | 内联 work-row（只有番号文字） | grid/button | 补齐封面，与视频板块一致 |
| 关注·小说作者作品 | NovelSeriesCard + 拼假对象 | grid/none | 删掉假对象拼装 |
| 目标作品页 / 详情页相关作品 | VideoCardGrid | grid/infinite、grid/button | |
| 历史·全部 | TimelineList/TimelineRow | row/none | 进度/时间格式化迁入适配器 |
| 历史·漫画/视频/小说分组 | ComicGrid / VideoCardGrid / NovelSeriesCard | grid/none | |
| 历史·收藏 | 内联 favorite-card + 取消按钮 | row/none + actions 槽 | |

**不纳入**（不是「媒体条目列表」，强行统一反而失真）：小说章节目录（系列详情、阅读器抽屉）、
关注页的「可关注对象/已关注对象」人名列表（厂商/女优/作者是人，不是作品）、SplitPage 分组工具、
标签黑名单设置。

**删除**：ComicCard、ComicGrid、VideoCardGrid、VideoBrowser、NovelSeriesCard、TimelineRow、TimelineList、
Pagination（死代码）。不留兼容层，按板块一次性切换。

## 4. 批次（每笔可独立回滚，每笔 type-check + vite build + 41 项 cargo test + 产物断言）
| 批次 | 内容 | 删除 |
|---|---|---|
| 0 基础 | mediaItem.ts 适配器、MediaCard、MediaCollection、TagBlacklistSheet、badge token；无消费者，纯新增 | — |
| 1 漫画 | 首页、搜索·漫画、关注·漫画作品、历史·漫画 | ComicGrid、ComicCard |
| 2 视频 | useVideoFeed；首页视频段、VideoPage、搜索·视频、目标作品页、详情相关作品、关注·视频作品、历史·视频 | VideoBrowser、VideoCardGrid |
| 3 小说 | 推荐/榜单、搜索·小说、关注·小说作品、历史·小说 | NovelSeriesCard |
| 4 历史行式 | 历史·全部、收藏 → row 布局 | TimelineRow、TimelineList、Pagination |
| 5 收尾 | 新组件 DESIGN token 自查、ARCHITECTURE.md 组件章节更新 | — |

**产物断言**：构建产物中不再出现被删组件的类名/文件；新增一个 UI 夹具页（同 B5 验证法：真实组件 +
假数据，无头 Chromium 400px/1200px 两档），断言各 kind 卡片数、角标、骨架、哨兵、row/grid 两种布局的
元素存在与尺寸——不加载任何站点内容。

## 5. 需 frontend-dev 过目的隐藏依赖（我已发现的）
1. App.vue keep-alive 缓存 HomePage/SearchPage/NovelPage/FollowingPage/HistoryPage：容器内无限滚动在
   deactivated 时要 teardown、activated 时 re-setup（现各页手写了这一对，迁移后由容器内部负责）。
2. 首页/搜索的 sessionStorage 状态与滚动恢复：依赖 items 数组形态（ComicItem[]），适配只在渲染层做，
   存储层不变。
3. SearchPage 漫画「星标降序」会 fetchAllPages 全量拉取后本地分页：容器只负责展示 visibleItems。
4. ComicCard 是 `<a href target=_blank>`：桌面中键/新窗口依赖此行为，MediaCard 用 `href` 字段保留。
5. 小说 hlib 3 秒限速与「未解析到系列链接=到底」的终止判定在页面层，容器不吞错误文案。
6. 视频同番号去重（中字>有码>无码）在页面层/后端，容器不做去重；页面层直接用已有的
   `utils/videoCode.ts` 的 `dedupeByVideoBase`（需求 12 口径单点），不再另写。

frontend-dev 过目补充（已确认有效，纳入实施约束）：
7. VideoBrowser 的哨兵是**常驻 DOM、v-show 显隐**（「搜索后翻页失效」的结构性修复）：MediaCollection 的
   哨兵同样常驻、只用 v-show 控制，不随条件分支销毁重建；VideoBrowser 的哨兵调试日志迁入容器（`[media-collection]`）。
8. 漫画网格 6→5→4→3→2 列的断点是用户习惯密度：MediaCollection 网格按 kind 取断点，漫画沿用这套断点，
   不用 auto-fill（避免平板密度回归）；视频/小说保持现有 auto-fill 密度。
9. 漫画路由 `/comic/:source/:categoryId/:id` 的 categoryId 必须由适配器带上（历史/收藏取存量字段），缺失即 404。

## 2.4 MediaItem 设计补充（frontend-dev 意见，采纳）
- 跳转字段类型收窄为联合：`{ to: RouteLocationRaw; href?: string } | { href: string; to?: RouteLocationRaw }`，
  杜绝「可点击但无去处」的死卡。
- `viewed` 保持独立字段（右上角已看/已读标记），不并入 badges。
- 进度/时间格式化复用 `utils/formatTime.ts`、`utils/formatCount.ts`；进度 JSON 解析保留 try/catch 容错。

## 6. 视觉统一化的取舍（请审定）
- 卡片一律无阴影、hairline 边框、18px 圆角（DESIGN.md）；三套现卡片的圆角/边框/底色不一致，统一后会有轻微观感变化。
- 关注·视频作品从「番号文字行」升级为封面卡片（与视频板块一致）。
- 小说卡片仍无真实封面（站点不提供），统一用书本图标占位。
