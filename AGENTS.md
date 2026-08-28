# niacg — Android 漫画浏览应用（Vue 3 + Tauri 2）

## 项目概况

前端使用 Vue 3、TypeScript、Vite 和 vue-router。Android 容器及全部后端逻辑使用
Tauri 2/Rust；应用内部不启动 HTTP 服务，也不提供桌面分发。

## 关键命令

```sh
npm run type-check       # Vue/TypeScript 类型检查
npm run build-only       # Vite 生产构建
npm run android:init     # 首次生成 Tauri Android 工程
npm run android:dev      # 在 Android 设备或模拟器上开发
npm run android          # 构建 APK/AAB
cd src-tauri && cargo check
cd src-tauri && cargo fmt --check
```

## 架构要点

- Vue 通过 `@tauri-apps/api/core` 的 `invoke` 调用 Rust commands。
- commands、网络、缓存和数据访问入口位于 `src-tauri/src/lib.rs`。
- HTML 解析位于 `src-tauri/src/parser.rs`，使用 `scraper`。
- 关注、历史和黑名单位于应用数据目录的 `ygqd.db`，使用 bundled SQLite。
- 漫画发布日期元数据也持久化在 `ygqd.db`，供关注作者的新作检查复用。
- 图片通过 `fetch_image` command 返回二进制数据，`ProxyImage.vue` 将其转换成 Blob URL。
- `src/services/api.ts` 是前端唯一的 Tauri IPC 封装层。
- `noUncheckedIndexedAccess: true`，索引访问必须处理 `undefined`。

## 开发注意事项

- 这是 Android-only 项目；不要增加桌面窗口行为或 localhost API 服务。
- 新增后端功能时同时注册 `#[tauri::command]` 和 `generate_handler!`，再在
  `src/services/api.ts` 添加类型化封装。
- 网络请求应继续复用共享的 `reqwest::Client`，以保留 cookie 和统一请求头。
- SQLite 操作必须经过共享连接的互斥锁，并保持 camelCase IPC 序列化格式。
- 修改解析逻辑后应优先增加基于固定 HTML fixture 的 Rust 单元测试。
