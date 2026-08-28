# 摇杆驱动

Android 漫画浏览应用，Vue 3 前端 + Tauri 2/Rust 后端。

后端通过 Tauri commands 在进程内提供抓取、HTML 解析、图片下载、缓存和 SQLite
持久化，不启动本地 HTTP 服务。

```sh
npm install
npm run android:init # 首次生成 Android 工程
npm run android:dev  # 连接设备进行开发
npm run android      # 构建 APK/AAB
```

前端检查使用 `npm run type-check` 和 `npm run build-only`，Rust 后端检查使用
`cd src-tauri && cargo check`。
