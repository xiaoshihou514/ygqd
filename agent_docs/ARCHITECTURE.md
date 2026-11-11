# 架构说明

面向后续智能体与新人的项目架构与开发约定。接口明细见 [API.md](API.md)，UI 设计规范见
[DESIGN.md](DESIGN.md)，板块规划与实施状态见 [CONTENT_PLAN.md](CONTENT_PLAN.md)。

## 项目概况

Android 优先的漫画/视频/小说聚合浏览应用：前端 Vue 3 + TypeScript + Vite +
vue-router，应用容器与全部抓取逻辑为 Tauri 2 (Rust)，同时提供 Linux AppImage
桌面构建。应用内不启动对外 HTTP 服务（本地流代理仅绑定 127.0.0.1），不提供
macOS/Windows 分发。

内容源与板块：

| 板块 | niacg (ACG) | bika (哔咔) | fs1 / missav |
|---|---|---|---|---|
| 漫画 | HTML 抓取 | PicaAPI JSON | — | — |
| 视频 | — | — | fs1 直连 + missav 指纹直连 | — |

- niacg：HTML 直出，无需登录；两次搜索强制间隔 5.2 秒，否则拿到空结果提示页。
- bika：需账号登录（设置页登录，凭据与 token 存 `ygqd.db`，401 自动重登）。
- 视频：元数据与播放优先 fs1（reqwest 直连）；missav 按 TLS 指纹拦截通用客户端，
  用 wreq 模拟 Chrome 指纹直连兜底（构建需 `ANDROID_NDK_HOME`，见「开发命令」）。

## 架构分层

```
Vue 页面/组件 ──invoke──▶ Rust commands ──▶ sources/* 抓取 ──▶ 上游站点
     ▲                        │                    │
     └── 本地流代理(127.0.0.1) ◀── stream_proxy ◀──┘ (图片/HLS 经代理回环)
```

### 前端（src/）

- `services/api.ts`：**唯一的 Tauri IPC 封装层**，页面与组件不直接调 `invoke`。
- `pages/` + `router/index.ts`：路由为
  `/`（首页，内容分段：漫画/视频/小说）、`/search`、`/comic/:source/:categoryId/:id`、
  `/videos` 与 `/videos/:code`（视频列表/详情）、`/videos` 系列
  （小说书架/阅读器）、`/split`、`/following`、`/history`、`/settings`。
- 首页顶部为内容分段控件：漫画段沿用源分类 tab 组；视频段为最新流（`useVideoFeed`）；
  小说段为书架入口卡片。底部导航五 tab：推荐/搜索/关注/历史/设置。
- `composables/`：本地偏好持久化（浏览已读、搜索历史、标签黑名单、主题等）。
- **媒体列表统一体系（唯一实现，勿另起卡片/列表）**：漫画/视频/小说在推荐、榜单、
  搜索、关注、历史、收藏、目标作品等全部场景共用
  - `utils/mediaItem.ts`：统一条目模型 `MediaItem` 与适配函数（`fromComic`/
    `fromComicHistory`/`fromFavorite`/`fromVideo`/
    `fromTimeline`）——板块差异（封面代理来源、中字/无码角标、小说占位、进度文案、
    漫画路由 categoryId、小说点击去向）只在这里处理；
  - `components/MediaCard.vue`：唯一卡片（grid/row 两种布局，actions 插槽；漫画长按
    拉黑弹层 `TagBlacklistSheet.vue`）；
  - `components/MediaCollection.vue`：唯一编排容器（骨架/空态/错误与重试/无限滚动或
    按钮翻页）。无限滚动哨兵常驻 DOM、v-show 显隐、模板 ref，keep-alive 激活/失活
    自动重绑/解绑；限速、去重、本地分页留在页面层。
  - 回归护栏：`npm run ui:fixture`（真实组件 + 假数据，无头 Chromium 400/1200px 断言）。
  方案与迁移记录见 `UNIFIED_MEDIA_PLAN.md`。
- `components/ProxyImage.vue`：封面统一经 `fetch_image`/`fetch_video_image`
  取回二进制转 Blob URL，解决防盗链。
- `config/sources.ts`：`SourceId`（`niacg`/`bika`）与路由参数收敛
  （`parseSourceId`，未知值回退 niacg）、首页 tab 配置。
- `types/index.ts`：IPC 共享数据模型（与 Rust 侧一一对应）。

### IPC 边界

- 成功值直接返回；Rust 的 `Err(String)` 使 `invoke` Promise reject，前端按字符串提示。
- 共享模型定义在 `src/types/index.ts` 与 `src-tauri/src/models.rs`，Rust 模型用
  `#[serde(rename_all = "camelCase")]` 保持与 TypeScript 字段一致。
- 新增 command 三处同步：`#[tauri::command]` + `generate_handler!`（`lib.rs` 尾部）
  + `api.ts` 类型化封装。command 全表见 [API.md](API.md)。

### Rust 后端（src-tauri/src/）

- `lib.rs`：全部 command 入口与 `AppState`（niacg 共享 reqwest client、
  video 抓取器、哔咔客户端 `RwLock`（切换分流时整体重建）、哔咔 token、
  SQLite 连接 `Mutex`、300 秒 TTL 进程内缓存、niacg 搜索限速闸）。
- `sources/` 按源分模块：`niacg.rs`（scraper 解析 HTML）、`bika.rs`
  （PicaAPI 客户端，HMAC-SHA256 请求签名）、`video.rs`（fs1 + missav，
  详情页 Dean Edwards packer 解码提取 m3u8）。
  新增源在此目录加模块，并在 `lib.rs` 命令里加分发分支、前端 `api.ts` 加封装。
- `stream_proxy.rs`：本地回环 HTTP 代理（仅 127.0.0.1）。三个职责：
  ① HLS 转发（m3u8 文本重写子地址为代理地址；分片带站点 Referer 转发；
  Range 透传；已取分片缓存为插入序，超限约 96MB 时批量淘汰最旧一半，
  频繁快进零重复下载）；
  ② 图片防盗链转发（`/img/` 端点保留，当前无调用方——封面统一走
  `fetch_image` IPC 取回二进制）；③ 网关回传端点。播放器只接触本地代理
  地址，永不直连上游 CDN（其校验 Referer 且拒绝浏览器 TLS 栈）；封面组件
  经 IPC 取回二进制，不走代理 URL。
- `models.rs` / `db.rs`：IPC 数据模型；建表与迁移。

### 数据库（应用数据目录 `ygqd.db`，bundled SQLite）

| 表 | 主键/说明 |
|---|---|
| `favorites` | (source, comic_id)；bika 源收藏同时同步服务器 |
| `view_history` | (source, comic_id)；漫画与通用浏览记录 |
| `video_watched` | code；视频已看记录 |
| `novel_series` / `novel_progress` | **已废弃保留**（小说板块删除后不再读写；用户设备存量数据不动，留恢复余地） |
| `follow_target` / `tag_blacklist` / `item_history` | 关注目标（漫画作者+视频厂商/女优）/ 标签黑名单（mode 区分）/ 全局历史时间线 |
| `bika_account` / `bika_address` | 哔咔凭据 / 分流地址（单行） |

## 关键约定

1. `src/services/api.ts` 是唯一 IPC 封装层；不在页面/组件里直接 `invoke`。
2. IPC 序列化保持 camelCase，与 TS 类型字段一致。
3. 新增后端功能三处同步：`#[tauri::command]`、`generate_handler!`、`api.ts` 封装。
4. 网络请求复用共享 `reqwest::Client`（保留 cookie 与统一请求头）；
   wreq 仅用于 missav 的 TLS 指纹直连，不要混用。
5. SQLite 操作必须经过共享连接的互斥锁。
6. 修改解析逻辑后优先增加基于固定 HTML fixture 的单元测试
   （`video.rs` 的 packer 解码、meta-refresh 均有真实样本单测）。
7. Android 优先，同时支持 Linux 桌面；不要引入 macOS/Windows 特定行为或
   localhost 对外 API 服务。
8. TypeScript 开启 `noUncheckedIndexedAccess`，索引访问必须处理 `undefined`。
9. UI 样式遵循 [DESIGN.md](DESIGN.md) 的 token 体系，不内联 hex。
10. 构建 Android 时 wreq 依赖 BoringSSL 交叉编译，需要 `ANDROID_NDK_HOME`
    指向 NDK 根目录（CI 已在 android-nightly workflow 中设置）。

## 开发命令

```sh
just dev            # Android 开发(npm run android:dev)
just build          # 构建 APK/AAB
just type-check     # vue-tsc 类型检查
just check          # type-check + build-only + cargo check
just format         # prettier + cargo fmt
npm run desktop:dev # Linux 桌面调试
```

本机开发构建 Android 前需自行设置（CI 由 android-nightly workflow 设置）：

```sh
export ANDROID_NDK_HOME="$ANDROID_HOME/ndk/<NDK 版本目录>"
```

CI 为 GitHub Actions 双流水线（`.github/workflows/`）：Android Nightly（APK）
与 Linux Nightly（AppImage），推送 main 触发并发布滚动 nightly release。

## 已登记技术债

### T-1 video.rs 单文件过大（1736 行，重构搁置）

**现状**：`src/sources/video.rs` 单文件承载 fs1/missav 双源的全部解析、
抓取与 VideoApi（68 函数）。两次拆分尝试（脚本批量/手工批量迁移）
均在 impl 方法与自由函数的交织处失控（错误连锁 97 条），已回滚止损。

**建议拆分路径**（未来有 IDE 级重构工具或人力时执行）：
- `video/mod.rs`：VideoApi 结构、new、HTTP 层（get/missav_get）、
  入口（list/search/detail）、共享类型
- `video/fs1.rs`：parse_cards、scan_codes_fallback、search_fs1、
  detail_fs1 系列、to_base、fs1_models 系列
- `video/missav.rs`：search_missav、missav_m3u8、play_paths、
  parse_missav 系列、MissavMeta、apply_missav_meta、meta 链接提取
- `video/follow.rs`：dedup_key、base_code、variant_priority、
  dedupe_by_base、merged_has_next、关注作品流

**实施要求**：rust-analyzer 的 move item 重构（文本剪切在 27 方法
与 20 自由函数的交织处已被证明不可靠）；每移动 3-5 函数一次
编译核对；expert 复审的 c879957 版本（MissavMeta 结构化）为准。

**记录日期**：2026-11-09（重构批次收官，lead 裁决方案 1 搁置）

### T-2 GatewayActivity overlay 保留（无消费方，随小说板块删除登记）

**现状**：`android-overlay/GatewayActivity.kt` 及其注册链
（`apply-android-overlay.mjs` 注入 AndroidManifest、nightly workflow
的 overlay 校验步骤）为 WebView 网关的 Android 宿主通道。小说板块
删除（`gateway.rs` 移除）后网关无消费方——视频 fs1/missav 已 wreq
指纹直连、哔咔/niacg 亦 reqwest 直连。

**保留理由**：删除涉及 Android 构建链（overlay 注入脚本 + workflow
校验步骤 + AndroidManifest 注册）且不影响功能；未来网关复用（其他
CF 严防站点接入）时可恢复。

**恢复路径**：参见 git 历史 `2a98ac3` 之前的 `gateway.rs`（WebView
网关实现）与本文件早前版本的网关章节描述。

**记录日期**：2026-11-11（小说板块删除收官，lead 批准保留登记）
