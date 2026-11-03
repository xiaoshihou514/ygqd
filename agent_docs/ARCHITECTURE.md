# 架构说明

面向后续智能体与新人的项目架构与开发约定。接口明细见 [API.md](API.md)，UI 设计规范见
[DESIGN.md](DESIGN.md)，板块规划与实施状态见 [CONTENT_PLAN.md](CONTENT_PLAN.md)。

## 项目概况

Android 优先的漫画/视频/小说聚合浏览应用：前端 Vue 3 + TypeScript + Vite +
vue-router，应用容器与全部抓取逻辑为 Tauri 2 (Rust)，同时提供 Linux AppImage
桌面构建。应用内不启动对外 HTTP 服务（本地流代理仅绑定 127.0.0.1），不提供
macOS/Windows 分发。

内容源与板块：

| 板块 | niacg (ACG) | bika (哔咔) | fs1 / missav | hlib.cc |
|---|---|---|---|---|
| 漫画 | HTML 抓取 | PicaAPI JSON | — | — |
| 视频 | — | — | fs1 直连 + missav 指纹直连 | — |
| 小说 | — | — | — | WebView 网关抓取 |

- niacg：HTML 直出，无需登录；两次搜索强制间隔 5.2 秒，否则拿到空结果提示页。
- bika：需账号登录（设置页登录，凭据与 token 存 `ygqd.db`，401 自动重登）。
- 视频：元数据与播放优先 fs1（reqwest 直连）；missav 按 TLS 指纹拦截通用客户端，
  用 wreq 模拟 Chrome 指纹直连兜底（构建需 `ANDROID_NDK_HOME`，见「开发命令」）。
- 小说：hlib.cc 全站 CF 挑战 + 登录墙，必须经 WebView 网关（用户首次点一次验证，
  cookie 持久化），站点对连续抓取敏感，章节间隔 ≥ 10 秒。

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
  `/videos` 与 `/videos/:code`（视频列表/详情）、`/novels` 与 `/novels/:id`
  （小说书架/阅读器）、`/split`、`/following`、`/history`、`/settings`。
- 首页顶部为内容分段控件：漫画段沿用源分类 tab 组；视频段内嵌 `VideoBrowser`；
  小说段为书架入口卡片。底部导航五 tab：推荐/搜索/关注/历史/设置。
- `composables/`：本地偏好持久化（浏览已读、搜索历史、标签黑名单、主题等）。
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
  详情页 Dean Edwards packer 解码提取 m3u8）、`novel.rs`（hlib，经网关抓取）。
  新增源在此目录加模块，并在 `lib.rs` 命令里加分发分支、前端 `api.ts` 加封装。
- `gateway.rs`：WebView 网关（当前仅 hlib 使用）。内嵌 webview 用真浏览器网络栈
  完成 CF 挑战与登录；抓取结果经本地回环 HTTP（`POST /gwresult`）回传——远程页
  没有 Tauri IPC 桥，invoke 方案不可行。
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
| `novel_series` / `novel_progress` | 书架与阅读进度（章节序 + 页码） |
| `followed_authors` / `tag_blacklist` | 关注作者 / 标签黑名单（mode 区分） |
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
