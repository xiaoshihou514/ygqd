# niacg

漫画浏览应用，Vue 3 前端 + Kotlin/Ktor 后端，支持 Web / Android / 桌面端。

## 项目结构

```
niacg/
├── src/                          # Vue 3 前端源码
│   ├── pages/                    # 页面组件
│   ├── components/               # 通用组件
│   ├── composables/              # 组合式函数（useTheme 等）
│   ├── services/                 # API 调用层
│   ├── router/                   # Vue Router
│   ├── styles/                   # CSS 变量与全局样式
│   └── types/                    # TypeScript 类型
├── kotlin-backend/               # Kotlin 后端
│   ├── core/                     # 核心业务逻辑、模型、解析器
│   ├── server-desktop/           # 桌面端 Ktor 服务 + 打包
│   └── server-android/           # Android 端 Ktor 服务 + APK
├── .github/workflows/            # GitHub Actions
│   ├── push-apk.yml              # 构建 APK + 桌面端并推送 Release
│   └── kotlin-backend-ci.yml     # CI：测试、桌面端构建、Android 构建
└── dist/                         # Vue 构建产物
```

## 特性

- **自适应暗黑模式**：跟随系统 `prefers-color-scheme` + 手动切换（☀/☾/☯）
- **单文件可执行**（桌面端）：双击即用，无需分别启动前后端
- **APK 一键安装**（Android）：WebView 内嵌前端，后台自动运行 Ktor 服务

## 前端开发

```sh
npm install
npm run dev
# 自定义后端地址
KOTLIN_BACKEND=http://192.168.1.100:8080 npm run dev
npm run build-only      # 生产构建
npm run type-check      # 类型检查
```

## 桌面端

```sh
# 一键构建桌面可执行文件
npm run desktop

# 或分步执行
npm run build-only
cd kotlin-backend
./gradlew :server-desktop:installDist

# 产物路径：
# kotlin-backend/server-desktop/build/install/server-desktop/
#  ├── bin/server-desktop      ← 启动脚本
#  └── lib/                    ← JAR 包
#  └── web/                    ← Vue 前端
```

启动桌面应用：

```sh
kotlin-backend/server-desktop/build/install/server-desktop/bin/server-desktop
# 访问 http://localhost:8080
```

### 自定义 web 目录

```sh
# 命令行参数
-Dniacg.webDir=/path/to/custom/web

# 或设置环境变量
export NIACG_WEB_DIR=/path/to/custom/web
```

## Android APK

```sh
# 一键构建
npm run android

# 或分步执行
npm run build-only
cd kotlin-backend
echo "sdk.dir=$ANDROID_HOME" > local.properties
./gradlew :server-android:assembleDebug

# APK 产出路径：
# kotlin-backend/server-android/build/outputs/apk/debug/
```

## CI 自动构建

推送代码到 main 分支自动触发：
- **Push Release** → 构建 APK + 桌面端 → 推送到 GitHub Release（`latest` tag）
- **Build & Test** → 测试、桌面端构建、Android 构建

## IDE

- [VS Code](https://code.visualstudio.com/) + [Volar](https://marketplace.visualstudio.com/items?itemName=Vue.volar)
- 禁用 [Vetur](https://marketplace.visualstudio.com/items?itemName=octref.vetur)
