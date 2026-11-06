# 平板 / 真机 WebView 调试指南

面向 backend-dev、frontend-dev。内容来自小说网关(hlib)在 Android 真机上的攻坚记录,
其中的坑都实际踩过。动手前先读第 0 节和第 4 节。

---

## 0. 三条铁律(先背下来)

1. **以窗口焦点为准再操作**。发任何点击、返回键之前,先确认当前焦点窗口是我们的应用
   (见 3.1)。攻坚期间两次误开用户微信,都是因为没有确认前台就操作。
2. **不截图、不渲染站点内容**。只做元数据层断言:节点存在、计数、坐标、包名、日志关键字
   (见第 4 节)。`adb shell screencap` / `screenrecord` 一律禁止。
3. **adb server 不是你的就别杀**。沙箱内起的 adb server 看不到 USB 设备,
   杀掉别人起的 server 后,沙箱里再也起不回来(见 1.3)。

---

## 1. 环境与连接

### 1.1 常用命令速查

```sh
adb devices -l                                   # 枚举设备,看状态与型号
adb -s <串号> install -r app.apk                 # 覆盖安装(保留数据与登录 cookie)
adb -s <串号> shell monkey -p com.niacg.ygqd \
    -c android.intent.category.LAUNCHER 1        # 拉起应用(比 am start 省去写 Activity 名)
adb -s <串号> shell am force-stop com.niacg.ygqd # 结束应用
adb -s <串号> shell pidof com.niacg.ygqd         # 进程是否存活(进程号变化 = 曾经重启/崩溃)
adb -s <串号> shell input tap X Y                # 点击(坐标见 3.2)
adb -s <串号> shell input keyevent 4             # 返回键(先看 3.1!)
```

- **始终带 `-s <串号>`**。多设备或换设备时,不带串号会操作到错误的设备。
- **覆盖安装只用 `install -r`**。卸载重装会清空应用数据,网关里的 hlib 登录 cookie 随之丢失。
  CI 包和本机包签名不同时 `install -r` 会失败(INSTALL_FAILED_UPDATE_INCOMPATIBLE),
  这时**不要**卸载,改用同一流水线出的包。
- **装之前校验 CI 产物哈希**:`gh run download <run-id>`,再比对包里自带的 `.sha256`。
  注意 `.sha256` 文件里记录的是 CI 侧路径,`sha256sum -c` 会报找不到文件,
  手工比对两边的哈希值即可。

### 1.2 设备识别:先核对再动手

```sh
adb devices -l
# 22e616e3  device usb:3-2 product:xun model:23073RPBFC device:xun
adb -s <串号> shell getprop ro.product.marketname   # 例:Redmi Pad SE
adb -s <串号> shell wm size                        # 分辨率,影响点击坐标
adb -s <串号> shell pm list packages | grep niacg   # 是否已安装
```

攻坚期间出现过一次换设备:手机 `2af6d937`(24069RA21C/peridot)换成了平板 `22e616e3`
(23073RPBFC/xun)。新设备上没有应用,也没有 hlib 登录态,屏幕坐标也完全不同。
**串号一变,先停下来向 lead 确认用哪台设备验收**,不要直接往别人设备上装东西。

### 1.3 unauthorized 与 adb server 的坑(必读)

现象:`adb devices` 显示 `unauthorized`。

处理顺序:
1. 让用户看手机上有没有「允许 USB 调试」弹框,点允许并勾选「始终允许」。
2. 没有弹框时,请用户在**真实终端**执行 `adb kill-server && adb start-server && adb devices -l`,
   或重新插拔数据线。

**沙箱内外的坑(这次亲身踩过)**:
- 我们的 bash 运行在沙箱里,沙箱中**没有 `/dev/bus/usb`**。用 `lsusb` 能看到设备,
  但沙箱内起的 adb server 枚举不到任何 USB 设备,而且 shell 一结束 server 就退出。
- 之前能用 adb,是因为 server 是用户在沙箱外起的,我们只是作为客户端连上去
  (默认 tcp:5037)。
- 我按指示执行了 `adb kill-server`,结果**杀掉了这个沙箱外的 server**,沙箱里也起不回能访问
  USB 的 server,只能请用户重新起。
- 规范:**在沙箱里永远不要执行 `adb kill-server`**。遇到 unauthorized,把操作写清楚交给用户在
  真实终端完成。判断方法:`ls /dev/bus/usb` 不存在,就说明你在沙箱里。

---

## 2. WebView 调试核心

### 2.1 chrome://inspect 远程调试(Android WebView DevTools)

开启条件(wry `src/android/main_pipe.rs`):只有在 `debug_assertions`(debug 构建)
或启用 tauri `devtools` 特性时,才会调用 `WebView.setWebContentsDebuggingEnabled(true)`。

- **CI nightly 是 release 构建**,未启用 devtools,chrome://inspect **看不到**我们的 WebView;
  而且 APK 不可调试(`run-as` 不可用)。
- 开启方式(任选其一,**都只用于本地调试,不要提交**):
  1. 本地出 debug 包:`npm run android:init` 之后执行 `npx tauri android build --debug`,
     或 `npm run android:dev`。debug buildType 自带 `isDebuggable = true`(gradle 生成)。
     注意签名与 CI 包不同,不能覆盖安装 CI 包,需另装一个实例或接受清数据。
  2. 临时给 `src-tauri/Cargo.toml` 的 tauri 加 `features = ["devtools"]` 再出 release 包。
- 连接方法:电脑上用 Chromium 打开 `chrome://inspect/#devices`,勾选 Discover USB devices,
  就能在设备下列出每个 WebView(主界面、网关各一个)。点 inspect 后可用 Console 执行 JS、
  看 Network。
- **安全纪律提醒**:inspect 窗口会**渲染页面画面**(屏幕投射)。网关页属于站点内容,
  **不要打开网关 WebView 的 inspect 画面**;只对主界面 WebView 用,或只在 Console 里做计数类断言。

### 2.2 logcat 过滤

```sh
adb -s <串号> logcat -c                         # 操作前清空,便于对齐
adb -s <串号> logcat -d -v time > .scratch/x.log
grep -E 'FATAL|AndroidRuntime: |am_anr|ANR in|APP_SCOUT' .scratch/x.log     # 崩溃 / 无响应
grep " $(adb -s <串号> shell pidof com.niacg.ygqd))" .scratch/x.log          # 只看本进程
grep -E 'START u0.*niacg|ActivityTaskManager' .scratch/x.log                 # Activity 启动记录
```

各关键字的含义:
- `FATAL EXCEPTION` + `AndroidRuntime`:Java/Kotlin 层崩溃,紧接着的几行就是异常栈。
  例:`NoSuchMethodError ... GatewayActivity.getPluginManager()`。
- `APP_SCOUT_WARNING` / `APP_SCOUT_HANG`(MIUI 特有):主线程卡顿超过 2.5 秒 / 5 秒。
  **后面会附主线程栈**,栈顶就是卡住的位置。
  例:`Rust.shouldOverride ← RustWebView.loadUrl` 说明卡在导航回调里。
- 没有 FATAL 但进程号变了:进程被系统杀掉(ANR 或内存回收)。
- `ContentCatcher ... notifyWebView view: com.niacg.ygqd.RustWebView{8835074 ...}`:
  每个 WebView 实例的标识。按出现顺序,第一个是主界面,后面的是网关。

注意:我们自己的 `diag()` **不进 logcat**,它写在应用数据目录的 diag.log(见 5.2)。

### 2.3 dumpsys:任务栈、焦点与视图层级

```sh
# 焦点窗口(判断前台的唯一可靠依据,见 3.1)
adb -s <串号> shell dumpsys window | grep -m1 mCurrentFocus

# 任务栈:网关是否在独立任务里
adb -s <串号> shell dumpsys activity activities | grep -E 'Task\{|Hist  #|GatewayActivity'
#   Task{#1611 ... A=10192:com.niacg.ygqd ...}           ← 主界面任务
#   Task{#1612 ... A=10192:com.niacg.ygqd.gateway ...}   ← 网关独立任务(taskAffinity)

# 某个 Activity 的视图层级(只看类名和实例标识)
adb -s <串号> shell dumpsys activity com.niacg.ygqd/.MainActivity | sed -n '/View Hierarchy/,/Looper/p'
```

**定位手法实例**:当年就是用这一招确认了「网关顶替主界面」——
`android:id/content` 下只剩一个 `RustWebView{a7c91f4}`,而这个标识是网关的(见 2.2),
说明主界面 WebView 被摘掉了。点「完成」后标识没变,说明收回无效。
修复后,同一位置的标识变回主界面的 `2410861`,即验证通过。

坑:
- `topResumedActivity` 在多显示区或过渡动画期间可能滞后,**以 `mCurrentFocus` 为准**。
- `dumpsys activity top` 会列出所有可见 Activity(其它应用的也在里面),要按包名过滤。

---

## 3. 自动化操作

### 3.1 焦点规范(必须遵守)

```sh
foc() { adb -s <串号> shell dumpsys window | grep -m1 mCurrentFocus \
        | grep -o 'com\.[A-Za-z0-9_.]*/[A-Za-z0-9_.]*' | head -1; }
# 输出形如 com.niacg.ygqd/com.niacg.ygqd.GatewayActivity
```

- **发返回键(keyevent 4)之前,必须确认 `foc` 是我们的目标窗口**。只在网关
  (`...GatewayActivity`)在前台时,才用返回键收回网关。主界面在前台时按返回键会让应用退到后台,
  露出用户之前打开的应用(这次就露出了微信聊天)。
- 比较 `foc` 的输出时**写全格式**,例如 `case "$F" in *GatewayActivity) ...`。
  不要拿 `niacg.ygqd.MainActivity` 这类自己臆造的短格式去做相等比较:这次就是这里写错,
  判断永远为真,导致误发了返回键。
- **不要点屏幕最顶端约 100px 的区域**(状态栏、通知横幅)。全面屏下网页元素会延伸到状态栏下,
  点上去可能打开通知;这次就因此点开了微信横幅。网关的「完成」栏已移到底部。
- 每次批量操作后都检查 `foc`,发现前台不是我们的应用,立刻用 monkey 拉回(见 1.1),
  并如实报告。

### 3.2 坐标与界面导出

```sh
adb -s <串号> shell uiautomator dump /sdcard/u.xml && adb -s <串号> pull /sdcard/u.xml .scratch/u.xml
adb -s <串号> shell rm -f /sdcard/u.xml
```

- 用 `text="..."` **精确匹配**加 `class` 后缀定位节点,取 bounds 的中心点击。
  不同设备、页面滚动、过渡动画都会让坐标变化,**每次点击前重新导出**,不要复用旧坐标。
- `could not get idle state`:页面还在动(动画、广告),等几秒重试。
- 只有 1 个节点、且根节点不是全屏:**有对话框挡在前面**(例如站点的 JS `alert`)。
  这时只查看对话框里的按钮(`OK`)并点掉。
- 页面切换有过渡动画,点击后要 `sleep`;点得太急会落在旧位置。
- WebView 内的元素在无障碍树里会被拍平,父子关系不可靠。做结构断言时用**计数**
  (如结果行数、文本节点总数),不要依赖父子层级。

### 3.3 文字输入:特殊字符与输入法

- `input text 'abc'`:`%s` 会被转换成空格,其它 `%x` 原样输入;整段加单引号,
  防止设备端 shell 展开。
- **输入法(fcitx5 拼音)会改写输入**:
  - 字母先进入拼音组字状态,要按回车(`keyevent 66`)或空格才会上屏;
  - 大写字母、`%` 等字符混在一起时会被组字改写:密码 14 个字符,实际只上屏 5~6 个;
  - 纯小写加 `@` `.` 的邮箱可以正常输入。
- **输入密码等精确字符串的做法**:临时停用输入法,输入完再**按原值恢复**:

  ```sh
  adb -s <串号> shell settings get secure default_input_method      # 先记下原值
  adb -s <串号> shell settings get secure enabled_input_methods     # 含子类型列表,也要记下
  adb -s <串号> shell ime disable <输入法ID>                         # 停用后按键直达 WebView
  # ……输入、核对长度……
  adb -s <串号> shell ime enable <输入法ID>
  adb -s <串号> shell ime set <输入法ID>
  adb -s <串号> shell settings put secure enabled_input_methods '<原值,分号要转义 \;>'
  ```
  恢复后必须重新 get 一次,与原值逐字比对。`ime enable` 会把子类型列表清掉,不手工写回就恢复不完整。
- **输入中文**(如「汉服」):在输入法启用状态下先切中文(`keycombination 113 62`,即 Ctrl+空格),
  再输入 `hanfu` 并按空格上屏,输入完切回英文。输入后从界面导出里核对文本框内容等于目标值。
- 清空输入框:点进去后按 `keyevent 123`(移到末尾),再按若干次 `keyevent 67`(删除)。
- 文本框里的值属于我们自己的输入,可以读出来核对;**密码框只核对长度**。

---

## 4. 调试纪律

### 4.1 安全纪律下的断言方式

本机有强制策略:**严禁打开、查看、渲染任何站点内容**(图片、视频、页面画面)。允许的只有:

| 允许 | 禁止 |
|---|---|
| 节点存在性(如登录表单有 `password="true"` 的输入框) | `screencap` / `screenrecord` 截屏录屏 |
| 计数(结果行数、文本节点数、条目数) | 打开网关 WebView 的 inspect 画面 |
| 包名、Activity 名、实例标识、bounds 坐标 | 在浏览器里打开站点样本 HTML |
| HTTP 状态码、字节数、日志关键字 | **宽泛地打印**界面导出里的全部文本 |
| 我们自己的界面文案(按钮、提示) | |

教训:有一次我用「所有短文本」的宽泛条件筛选界面导出,结果把网关列表页的标题和标签打印了出来,
违背了纪律。**导出筛选一律精确匹配**:要么匹配我们已知的文案(`text="登录"`、`返回应用`),
要么只输出计数和布尔值。站点弹窗的提示文案可以读一行,用来判断流程走到哪一步。

离线结构分析用代码完成:对抓下来的 HTML 统计 `li.list-group-item` 的数量,
或用页码选择器的最大值确定总页数。最好直接复用应用自己的解析器
(写一个临时 `#[ignore]` 测试调用 `extract_series`,用完删掉)。

### 4.2 截屏禁止

不截屏,也不让任何工具替你截屏。需要「看见」界面时,用界面导出(3.2)和 dumpsys(2.3)代替。

### 4.3 敏感凭据

- 凭据**只在执行操作的那条命令里出现**,严禁写入文件、脚本、diag、提交信息、报告和团队消息。
  需要引用时写「用户提供的凭据」。
- 核对输入时只输出「是否一致」或长度,**不回显凭据本身**。
- 辅助脚本(如 `.scratch/tab.sh`)里不要放凭据。
- 用完的 cookie 副本、抓取样本要及时删掉(`.scratch/` 虽然被 git 忽略,也要清理)。

### 4.4 共享工作区提交纪律

团队成员共用同一个工作目录,别人未提交的改动也在里面。**提交前必须执行
`git status` 加 `git diff --cached`,`git add` 时只列自己的文件**。这次有一笔提交就把
别人的临时取证代码一起带进了 release。

---

## 5. 本项目专项:小说网关

### 5.1 架构要点(调试前先在脑子里过一遍)

- 网关 = 独立的 `GatewayActivity`(独立任务栈,taskAffinity 为 `${applicationId}.gateway`),
  源码在 `src-tauri/android-overlay/`,由 `npm run android:init` 拷进生成的工程。
  **不要改 `src-tauri/gen/android/` 下的副本**,重新 init 会被覆盖。
  - 原因:wry 在 Android 上一个 Activity 只能挂一个 WebView,eval 一律投给最后创建的那个。
    网关如果和主界面同处一个 Activity,主界面的 IPC 回包(Android 上走 eval)就全被投进网关,
    导致主界面所有 invoke 永不返回。
- 收回 = `moveTaskToBack(true)`(网关保活、退到后台);前置 = 用 `GatewayActivity` 重新
  `startActivity`(singleTask 走 onNewIntent)。网关里按返回键 = 收回。
  对应代码在 `gateway.rs` 的 `finish` / `present` / `android_exec_on_gateway`。
- 网关与宿主之间的回传走主框架导航 `ygwd-gw://{r|finish|ready}`,在 on_navigation 里拦截。
  **这个回调在 Android 主线程上执行**:回调里不能做任何「投递到主线程再阻塞等结果」的事,
  比如 `app.path()`、同步的移动插件调用、`window.navigate`,否则主线程自锁、界面黑屏。
  需要做的副作用一律 `tauri::async_runtime::spawn` 出去。
- `JniHandle::exec` 递来的 webview 是该 Activity **最后创建的** WebView,与你从哪个窗口调用无关。
  要操作特定 WebView,必须通过它自己的 Activity 去做(网关就有自己的 Activity)。
- 站点在 WebView 里会弹出「本站没有软件……」的 `alert`,不点 OK 网关会一直卡着(见 3.2 对话框判断)。

### 5.2 日志位置

| 日志 | 位置 / 读取方式 |
|---|---|
| diag.log(Android) | 应用数据目录。release 包没法 `run-as`,只能在**应用内读取**:底栏「设置」→ 滚到「诊断日志」→「查看」。界面导出里是一整段文本,按 `\d{10} ` 时间戳拆分 |
| diag.log(桌面) | `.scratch/xdgdata/com.niacg.ygqd/diag.log`(XDG_DATA_HOME 指向 `.scratch/xdgdata`,见 start_dev.sh) |
| 系统日志 | `adb logcat`(见 2.2);我们的 diag 不在其中 |

diag 关键标签:`[网关链路]`(dispatch / 回传 / 就绪 / 闸)、`[网关显隐]`(Android 前置 / 退后台的结果)、
`[通道观测]`、`[导航入口]`、`[PANIC 屏障]`、`[自检]`。
桌面可以在 cage 无头后端起应用,并设置 `YGQD_SELFTEST_NOVEL=关键词` 跑小说链路自检(仅 debug 构建),
不会占用真实屏幕。

### 5.3 常见故障快速定位表

| 现象 | 先查什么 | 历史根因 |
|---|---|---|
| 黑屏 / 界面不动 | logcat 的 `APP_SCOUT_HANG` 及其主线程栈 | 导航回调里调用 `app.path()`,主线程等自己,自锁 |
| 应用直接退出 | logcat 有没有 `FATAL`:有 → 看异常栈;没有 → 进程号是否变了(ANR 被杀),或是不是返回键退到底 | `getPluginManager` 缺失导致闪退;卡死被判 ANR;同 Activity 下返回键作用于主界面 WebView |
| 网关弹出后回不来 | `mCurrentFocus`,以及网关任务栈是否独立;diag 里 `[网关显隐]` 的结果 | Android 上 show/hide 是空操作,必须走任务栈 |
| 主界面一直转圈(搜索中…/加载中…) | 网关是不是和主界面同处一个 Activity(dumpsys 看 WebView 个数和所属 Activity) | IPC 回包被投进了网关 WebView |
| 网关一直停在前台不收回 | 用界面导出看有没有对话框(只有 1 个节点、根节点不是全屏) | 站点 `alert` 没有点掉 |
| 搜索报错「需要登录」 | 网关是否打开了 `/login`;登录后搜索是否还撞墙 | 新设备没有登录态 |
| 等了很久才返回或超时 | diag 里 `is_gate=true` 和 `站点就绪 耗时` | 触发站点反爬闸「伊蕾娜」,访问太频繁 |
| 搜索结果太少 | 站点总页数(页码选择器的最大值) | 只取了第 1 页;已有分页命令 `novel_search_page` |

### 5.4 一轮真机验证的标准流程

1. `gh run download` 下载产物 → 比对哈希 → `adb -s <串号> install -r`
2. `logcat -c` → 用 monkey 拉起应用 → 每一步都先 `foc`,再导出界面,再点击
3. 每一步只做元数据断言:焦点所在 Activity、任务栈、计数、存在性
4. 结束时:`logcat` 里查 FATAL/ANR/APP_SCOUT,确认进程号没变,确认焦点回到我们的应用
5. 清理 `.scratch` 下的导出文件和安装包,如实报告所有误操作
