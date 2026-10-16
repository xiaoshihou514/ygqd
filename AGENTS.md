# niacg — Android 漫画浏览应用（Vue 3 + Tauri 2）

## 项目概况

前端使用 Vue 3、TypeScript、Vite 和 vue-router。Android 容器及全部后端逻辑使用
Tauri 2/Rust；同时提供 Linux 桌面构建（deb/rpm/AppImage）。应用内部不启动
HTTP 服务，也不提供 macOS/Windows 分发。

应用聚合两个漫画源，所有功能（首页、搜索、关注、历史、黑名单）跨源混合：
- `niacg`：niacg.com，HTML 抓取，无需登录。
- `bika`：哔咔（PicaAPI JSON 接口），需要账号登录（设置页登录，凭据和 token
  存 `ygqd.db`，401 自动重登）。

## 关键命令

```sh
npm run type-check       # Vue/TypeScript 类型检查
npm run build-only       # Vite 生产构建
npm run android:init     # 首次生成 Tauri Android 工程
npm run android:dev      # 在 Android 设备或模拟器上开发
npm run android          # 构建 APK/AAB
npm run desktop:dev      # Linux 桌面开发调试
npm run desktop:build    # 构建 Linux 桌面包（deb/rpm/AppImage）
cd src-tauri && cargo check
cd src-tauri && cargo fmt --check
```

## 架构要点

- Vue 通过 `@tauri-apps/api/core` 的 `invoke` 调用 Rust commands。
- commands、网络、缓存和数据访问入口位于 `src-tauri/src/lib.rs`；命令按
  `source` 参数分发（`"niacg"`/`"bika"`）。
- 漫画源实现位于 `src-tauri/src/sources/`：`niacg.rs` 用 `scraper` 解析
  HTML；`bika.rs` 是 PicaAPI 客户端（HMAC-SHA256 请求签名）。新增源时在该
  目录加模块，并在 `lib.rs` 命令里加分发分支、前端 `src/config/sources.ts`
  注册。
- 关注、历史、黑名单和哔咔账号位于应用数据目录的 `ygqd.db`，使用 bundled
  SQLite；`view_history` 主键为 (source, comic_id)。
- 图片通过 `fetch_image` command 返回二进制数据（按源选 HTTP client），
  `ProxyImage.vue` 将其转换成 Blob URL。
- 哔咔漫画按章节分页，阅读走 `fetch_comic_images`（ep_order/page）；
  niacg 一次性返回全部图片。
- `src/services/api.ts` 是前端唯一的 Tauri IPC 封装层；详情页路由为
  `/comic/:source/:categoryId/:id`，链接统一由 `src/utils/comicPath.ts`
  生成。
- `noUncheckedIndexedAccess: true`，索引访问必须处理 `undefined`。

## 开发注意事项

- Android 优先，同时支持 Linux 桌面；不要增加 macOS/Windows 特定行为或
  localhost API 服务。
- 新增后端功能时同时注册 `#[tauri::command]` 和 `generate_handler!`，再在
  `src/services/api.ts` 添加类型化封装。
- 网络请求应继续复用共享的 `reqwest::Client`，以保留 cookie 和统一请求头。
- SQLite 操作必须经过共享连接的互斥锁，并保持 camelCase IPC 序列化格式。
- 修改解析逻辑后应优先增加基于固定 HTML fixture 的 Rust 单元测试。
