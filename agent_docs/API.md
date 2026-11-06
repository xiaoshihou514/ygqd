# Tauri command 接口

前端统一通过 `src/services/api.ts` 调用 Rust commands，不存在 HTTP Base URL 或 REST
响应包装。成功值直接返回，Rust 的 `Err(String)` 会使 `invoke` Promise reject。
命令以 `src-tauri/src/lib.rs` 的实际签名为准，共享数据模型见文末说明。

## 漫画源（niacg / 哔咔通用）

| Command | 参数 | 返回值 |
|---|---|---|
| `fetch_category` | `source`, `category`, `page` | `{ items, pagination }`（300 秒进程内缓存） |
| `search_comics` | `source`, `keyword`, `classid`, `show`, `page`, `cacheBuster?` | `SearchResult`（300 秒缓存；niacg 源全局限速 5.2 秒，过密搜索会拿到空结果页） |
| `fetch_comic` | `source`, `categoryId`, `id` | `ComicDetail`（300 秒缓存；哔咔源附带给全章节列表） |
| `fetch_comic_images` | `source`, `categoryId`, `id`, `epOrder`, `page` | `ComicImagesResult`（哔咔源按话分页；niacg 源整话一次返回，图片地址缓存 300 秒） |
| `fetch_image` | `url`, `source` | `ArrayBuffer`（沿用对应源客户端的请求头/cookie） |
| `fetch_comic_viewed_ids` | `source`, `ids` | `string[]`（返回入参中已浏览过的 id 子集，判定依据是浏览历史） |

## 哔咔源专用

| Command | 参数 | 返回值 |
|---|---|---|
| `fetch_bika_categories` | — | `string[]`（300 秒缓存） |
| `fetch_bika_leaderboard` | `tt`（`H24`/`D7`/`D30`） | `{ items, pagination }`（无分页，一次返回整个榜；300 秒缓存） |
| `bika_switch_like` | `id` | `void`（toggle） |
| `bika_switch_favourite` | `id` | `void`（toggle） |
| `bika_login` | `email`, `password` | `BikaAccountStatus`（凭据入库，之后 401 自动重登） |
| `bika_logout` | — | `void`（清空本地凭据并失效全部缓存） |
| `bika_status` | — | `BikaAccountStatus` |
| `fetch_bika_addresses` | — | `string[]`（从官方 init 端点同步分流 IP） |
| `get_bika_address` | — | `string`（空串表示不分流） |
| `set_bika_address` | `address` | `void`（重建哔咔客户端并清缓存） |
| `sync_bika_favorites` | — | `number`（拉取服务器收藏合并进本地表，返回本次新增条数） |

## 本地数据（收藏 / 历史 / 关注 / 黑名单）

| Command | 参数 | 返回值 |
|---|---|---|
| `fetch_favorites` | — | `FavoriteEntry[]` |
| `is_favorite` | `source`, `comicId` | `bool` |
| `add_favorite` | `entry` | `void`（bika 源同时同步服务器） |
| `remove_favorite` | `source`, `comicId` | `void`（bika 源同时同步服务器） |
| `fetch_follows` | — | `FollowedAuthor[]` |
| `follow_author` | `author` | `FollowedAuthor` |
| `unfollow_author` | `author` | `void` |
| `fetch_history` | `limit` | `ViewHistoryEntry[]` |
| `record_history` | `entry` | `void` |
| `fetch_blacklist` | — | `BlacklistEntry[]` |
| `add_blacklist` | `tag`, `mode` | `BlacklistEntry` |
| `remove_blacklist` | `tag` | `void` |
| `update_blacklist` | `tag`, `mode` | `void` |

## 视频（fs1 优先，missav 直连兜底）

| Command | 参数 | 返回值 |
|---|---|---|
| `fetch_video_list` | `page` | `VideoListResult`（fs1 最新上市；`page` 从 1 开始；300 秒缓存） |
| `search_videos` | `keyword`, `page` | `VideoListResult`（fs1 与 missav 并发搜索合并，同番号以 fs1 为准；`page` 从 1 开始；300 秒缓存） |
| `fetch_video_detail` | `code` | `VideoDetail`（见下述缓存与代理说明） |
| `fetch_video_image` | `url` | `ArrayBuffer`（视频封面必须沿用 fs1 客户端请求头，尤其 Referer） |
| `record_video_watched` | `code` | `void`（UPSERT：重复观看仅刷新观看时间戳，同番号只留一条记录） |
| `fetch_video_watched_ids` | `codes` | `string[]`（批量查询：返回入参中已看过的 code 子集） |

`fetch_video_detail` 行为细节：

- 结果缓存 300 秒。缓存存的是**原始 m3u8 地址**；本地流代理端口每次启动随机，
  读取缓存时重新包装，因此缓存跨重启依然可用。
- 返回的 `m3u8` 是本地流代理地址（`http://127.0.0.1:{port}/…`），不是上游直链：
  上游 CDN 校验 Referer 并拒绝浏览器 TLS，播放器直连必 403，必须经 Rust 侧代理转发。
- 元数据与播放优先走 fs1；fs1 失效时由 missav 直连（TLS 指纹模拟）兜底，
  `provider` 字段标明实际来源（`fs1` / `missav`）。

## 小说（hlib 源，经 WebView 网关）

| Command | 参数 | 返回值 |
|---|---|---|
| `novel_add` | `input` | `NovelSeries`（输入 hlib 系列链接或纯 ID；入库书架；首次会弹网关窗口，需用户完成一次登录/安全验证） |
| `novel_search` | `keyword` | `NovelHit[]`（站内搜索，端点自适应；首次需网关验证/登录） |
| `novel_latest` | — | `NovelHit[]`（首页最新系列，发现页用） |
| `novel_shelf` | — | `NovelSeries[]`（本地书架） |
| `novel_remove` | `id` | `void`（移出书架） |
| `novel_series_info` | `id` | `NovelSeries`（按 ID 抓取系列信息含章节列表/简介，**不入库**——浏览漏斗用；首次弹网关验证/登录） |
| `video_follow` | `source, kind, targetId, displayName` | `void`（关注视频目标；source: fs1/missav，kind: maker/actress） |
| `video_unfollow` | `source, kind, targetId` | `void`（取消关注） |
| `video_follow_list` | `source, kind` | `FollowTargetEntry[]`（关注列表，按关注时间倒序） |
| `video_follow_targets` | `source, kind` | `FollowTargetItem[]`（可关注目标列表：fs1 女优/missav 厂商/missav 女优排行） |
| `video_follow_works` | `source, kind, targetId, page?` | `VideoListResult`（关注对象的作品流，复用视频列表解析） |
| `record_item_history` | `source, kind, itemId, title, progress?` | `void`（记录全局历史；视频/小说写入 item_history，漫画走 record_history 存量） |
| `fetch_item_history` | `source?, kind?, limit?` | `ItemHistoryEntry[]`（全局历史查询；source/kind 可选过滤，None=全板块时间线；ItemHistoryEntry 含 progress JSON） |
| `novel_search_page` | `keyword`, `page?` | `NovelSearchPage`（分页搜索：hits + page + totalPages；page 不传视为第 1 页。真端点 /s/search?query=，需登录） |
| `novel_latest` | `page?` | `NovelLatestPage`（发现流最新系列分页：hits + page + totalPages；page 不传视为第 1 页，深页走全站 ?p= 机制） |
| `novel_read` | `chapterId`, `page` | `NovelPage`（读取章节某页正文；未登录/未验证时返回引导错误） |
| `novel_progress` | `seriesId` | `[chapterOrder, page] | null`（阅读进度，无记录返回 `null`；`chapterOrder` 为源站目录「#N」的原样章节序号、非数组下标，续读按 order 精确匹配章节；`page` 为章节正文页码，从 1 开始） |
| `novel_set_progress` | `seriesId`, `chapterOrder`, `page` | `void`（写入阅读进度） |

## 网关（WebView 网关窗口，当前仅服务 hlib）

| Command | 参数 | 返回值 |
|---|---|---|
| `gateway_fetch` | `label`, `url` | `string`（经网关抓取页面文本，自动处理 Cloudflare 挑战；主窗口前端未直接使用，hlib 抓取在 Rust 侧调用） |
| `gateway_open` | `label` | `void`（打开/创建网关窗口，供用户登录站点或完成人机验证） |
| `gateway_finish` | `label` | `void`（用户点网关内「完成返回」后隐藏网关、主窗口回前台） |
| `gateway_set_visible` | `label`, `visible` | `void`（显示/隐藏网关窗口；主窗口前端未直接使用） |
| `gateway_result` | `result` | `void`（网关 webview 内 JS 的回传通道，经本地回环代理转交；**不暴露给主窗口前端**） |

## 诊断

| Command | 参数 | 返回值 |
|---|---|---|
| `diag_read` | — | `string`（读取应用数据目录下的 diag.log 全文） |
| `diag_clear` | — | `void`（删除 diag.log） |

## 共享数据模型

共享的数据模型定义在 `src/types/index.ts` 和 `src-tauri/src/models.rs`（视频模型在
`src-tauri/src/sources/video.rs`，小说搜索结果在 `src-tauri/src/sources/novel.rs`）。
Rust 模型使用 `#[serde(rename_all = "camelCase")]` 保持与 TypeScript 字段一致。

| Rust 类型 | TypeScript 类型 | 说明 |
|---|---|---|
| `PaginationInfo` | `PaginationInfo` | `current` 各源语义不一（niacg 恒 0，bika/视频为 page−1），**前端不应依赖此字段**（当前前端零消费）；翻页判断一律以 `hasNext`/`hasPrev` 为准，`total` 多数源不提供（填 0/1） |
| `VideoCard` | `VideoCard` | `code` 为番号（小写），是视频详情的 id；中字变体带 `-chinese-subtitle` 后缀 |
| `VideoListResult` | `VideoListResult` | `{ items: VideoCard[], pagination }` |
| `VideoDetail` | `VideoDetail` | `m3u8` 为本地流代理地址（见 `fetch_video_detail` 说明）；`provider` 区分 fs1/missav |
| `NovelHit` | `NovelHit` | `{ id, title, author?, tags? }`，搜索/发现页条目（author/tags 为 2024-11 扩展，hlib 列表页 DOM 含作者与标签元素） |
| `NovelSearchPage` | `NovelSearchPage` | `{ hits, page, totalPages }`——搜索结果的一页（hlib 每页 30 条） |
| `NovelLatestPage` | `NovelLatestPage` | `{ hits, page, totalPages }`——发现流最新系列的一页 |
| `NovelSeries` | `NovelSeries` | `{ id, title, author, chapters: NovelChapter[], addedAt, cover?, introduction? }`——cover 恒空(hlib 不提供系列封面)、introduction 来源页 meta description |
| `NovelPage` | `NovelPage` | `{ title, author, text, totalPages }` |

翻页基准提醒：漫画系命令（`fetch_category` / `search_comics` / `fetch_comic_images` /
`fetch_bika_leaderboard`）`page` 从 0 开始，Rust 侧内部 +1 转成源站页码；视频系命令
（`fetch_video_list` / `search_videos`）`page` 从 1 开始，直接透传。
