# niacg — 漫画浏览应用 (Vue 3 + Kotlin/Ktor)

## 项目概况

前端 Vue 3 + TypeScript + Vite + vue-router，后端 Kotlin/Ktor 多模块 Gradle 项目。
后端同时提供桌面端（单文件可执行）和 Android APK（内嵌 WebView）两种分发形式。

## 关键命令

```sh
# 前端
npm run dev              # Vite 开发服务器（:5173），/api/* 代理到 KOTLIN_BACKEND
npm run build-only       # 生产构建
npm run type-check       # vue-tsc --build 类型检查（tsconfig 有 project references）
npm run lint             # eslint . --fix
npm run format           # prettier --write src/
npm run backend          # 启动 Kotlin 桌面后端（Gradle :server-desktop:run）

# 全量构建
npm run desktop          # vite build → :server-desktop:installDist
npm run android          # vite build → :server-android:assembleRelease

# Kotlin 后端测试（核心模块）
cd kotlin-backend && ./gradlew :core:test -PexcludeIntegration
```

## 架构要点

- **Vite 代理**：开发环境 `/api/*` 由 Vite proxy 转发到 `KOTLIN_BACKEND`（默认 `http://localhost:8080`），无需单独配置 CORS
- **双后端实现**：`src/server/parser.ts` 用 Node.js（`node-html-parser`）抓取 niacg.com；`kotlin-backend/` 用 Ktor + Jsoup 实现同样功能——两者是平行实现，没有调用关系
- **tsconfig project references**：`tsconfig.json` -> `tsconfig.app.json` + `tsconfig.node.json`；`src/server/*` 仅在 node 配置中编译
- **@/ 路径别名**：映射到 `./src/*`，在 Vue SFC 和 TS 文件中通用
- **`noUncheckedIndexedAccess: true`**：tsconfig.app.json 开启，数组/对象访问需处理 undefined

## Kotlin 后端注意事项

- **JDK 17**，Kotlin 2.1.10，Ktor 3.0.2，Gradle 8.x
- **core 模块**：`allWarningsAsErrors = true`，JaCoCo 强制指令覆盖率 ≥ 95%（`./gradlew check` 会运行 `checkCoverage`）
- **测试**：集成测试标记 `@Tag("integration")`，CI 中用 `-PexcludeIntegration` 跳过
- **server-desktop**：主类 `com.niacg.backend.server.MainKt`，构建时将 `dist/` 复制到 `web/` 目录
- **server-android**：minSdk 26，targetSdk 35，构建时将 `dist/` 复制到 `src/main/assets/web/`
- **CI 触发条件**：`push-apk.yml` 监听 `kotlin-backend/**`、`src/**`、`package.json`、`vite.config.ts`、`tsconfig*.json`、`index.html` 变更

## 开发工作流

本地开发需要同时运行前端和后端：

```sh
# 方式一：一键启动（Kitty 终端，分屏）
./launch_kitty.sh

# 方式二：手动分步
npm run backend    # 终端 1：Kotlin 后端 (:8080)
npm run dev        # 终端 2：Vite 前端 (:5173)
```

## 部署

- `npm run desktop` -> `kotlin-backend/server-desktop/build/install/server-desktop/`
- `npm run android` -> `kotlin-backend/server-android/build/outputs/apk/release/`
- GitHub Release（latest tag）由 push-apk.yml 自动推送，pre-release
- `release.keystore` 和签名密码通过 CI secrets 注入
