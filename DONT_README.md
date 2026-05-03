# niacg

漫画浏览应用，Vue 3 前端 + Kotlin/Ktor 后端，支持 Web 和 Android 双平台。

## 项目结构

```
niacg/
├── src/                          # Vue 3 前端源码
│   ├── pages/                    # 页面组件
│   ├── services/                 # API 调用层
│   ├── router/                   # Vue Router
│   └── types/                    # TypeScript 类型
├── kotlin-backend/               # Kotlin 后端
│   ├── core/                     # 核心业务逻辑、模型、解析器
│   ├── server-desktop/           # 桌面端 Ktor 服务
│   └── server-android/           # Android 端 Ktor 服务 + APK
├── .github/workflows/            # GitHub Actions
│   ├── push-apk.yml              # 构建 APK 并推送到 Release
│   └── kotlin-backend-ci.yml     # CI：测试、桌面端构建、Android 构建
└── dist/                         # Vue 构建产物（npm run build-only）
```

## 架构

- **前端**：Vue 3 + TypeScript + Vite，API 调用使用相对路径 `/api/*`
- **后端**：Kotlin/Ktor，提供 RESTful API，代理/解析 niacg 网站内容
- **Android**：WebView 内嵌 Vue 前端，Ktor 后端以前台服务运行在同一 APK 中

## 前端开发

```sh
# 安装依赖
npm install

# 启动开发服务器（默认代理到 localhost:8080）
npm run dev

# 自定义后端地址
KOTLIN_BACKEND=http://192.168.1.100:8080 npm run dev

# 构建生产版本
npm run build

# 类型检查
npm run type-check
```

## 后端开发

```sh
# 桌面端运行后端
cd kotlin-backend && ./gradlew :server-desktop:run

# 或通过 npm 脚本
npm run backend
```

## Android APK 构建

APK 包含 Kotlin 后端和 Vue 前端，打包在同一个 APK 中，无需独立服务器。

### 本地构建

```sh
# 1. 先构建 Vue 前端
npm run build-only

# 2. 构建 Android APK（Gradle 会自动将 dist/ 复制到 assets）
cd kotlin-backend
echo "sdk.dir=$ANDROID_HOME" > local.properties
./gradlew :server-android:assembleDebug

# APK 产出路径：
# kotlin-backend/server-android/build/outputs/apk/debug/
```

### CI 自动构建

推送代码到 main 分支时，`.github/workflows/push-apk.yml` 会自动：
1. 构建 Vue 前端
2. 构建 Android APK
3. 将 APK 推送到 GitHub Release（`latest` tag）
