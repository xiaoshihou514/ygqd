//! WebView 网关:借应用内嵌 webview 的浏览器环境(真 Chrome 网络栈 +
//! 持久 cookie)处理 Cloudflare 验证与登录,供 hlib(小说)抓取使用。
//! (视频源 missav 已改为 wreq 指纹直连,不再走网关)
//!
//! 宿主通道(关键):网关加载的是远程页面,回环 fetch 与 IPC fetch 均被
//! CF 挑战页 CSP 拦截。抓取回传、完成栏、就绪报到统一走主框架导航
//! `ygwd-gw://{r|finish|ready}` + on_navigation 拦截(不真跳转,零 CSP 交互)。
//!
//! 交互时序:首次创建网关时窗口可见并导航到站点首页;CF 托管挑战通常
//! 自动通过,就绪探测报到后才发抓取;命中 CF 挑战静默重试(最多约 4 轮),
//! 命中站点反爬闸(伊蕾娜 /antibot)则网关主框架亲自走 Turnstile 校验;
//! 成功后网关自动隐藏(会话保留),主窗口回到前台。

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

/// diag.log 所在目录:setup 时解析一次缓存。不能每次现解析——Android 上
/// `app.path()` 走移动插件同步调用(投递主线程再阻塞等回音),而网关导航
/// 回调(shouldOverrideUrlLoading)本身就在主线程,现解析即主线程自锁死:
/// 实机表现为点开书架黑屏(MIUI 判 APP_SCOUT_HANG,栈顶 Rust.shouldOverride)。
static DIAG_DIR: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();

/// setup 阶段登记 diag 目录(先于任何网关导航)
pub fn init_diag_dir(dir: std::path::PathBuf) {
    let _ = DIAG_DIR.set(dir);
}

/// 诊断日志:写应用内部数据目录 diag.log;查看走应用内入口
/// (release 无法 run-as/logcat,应用内读取是唯一通道)。
/// 只用 setup 缓存的目录,任何线程(含 Android 主线程回调)调用都安全;
/// 未登记时静默丢弃。
pub fn diag(_app: &AppHandle, msg: &str) {
    use std::io::Write;
    let Some(dir) = DIAG_DIR.get() else {
        return;
    };
    let _ = std::fs::create_dir_all(dir);
    let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("diag.log"))
    else {
        return;
    };
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let _ = writeln!(f, "{ts} {msg}");
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct GatewayReply {
    pub id: String,
    /// HTTP 状态;0 表示 fetch 抛异常(body 为错误文本)
    pub status: u16,
    pub body: String,
}

/// 待回传的请求注册表:gateway_result 桥按 id 匹配 oneshot
pub struct GatewayManager {
    pending: Mutex<HashMap<String, oneshot::Sender<GatewayReply>>>,
    /// 就绪探测序号:已发出的最大值 / 页面报到过的最大值
    ready_issued: AtomicU64,
    ready_seen: AtomicU64,
    /// Android:系统回收 GatewayActivity 后网关窗口对象失效的标记
    /// (失效时 close 旧窗口重建,恢复抓取能力)
    #[cfg(target_os = "android")]
    gateway_stale: AtomicBool,
}

impl Default for GatewayManager {
    fn default() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            ready_issued: AtomicU64::new(0),
            ready_seen: AtomicU64::new(0),
            #[cfg(target_os = "android")]
            gateway_stale: AtomicBool::new(false),
        }
    }
}

impl GatewayManager {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_ready_nonce(&self) -> u64 {
        self.ready_issued.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn mark_ready(&self, nonce: u64) {
        self.ready_seen.fetch_max(nonce, Ordering::SeqCst);
    }

    /// Android:标记网关窗口失效(Activity 被系统回收)
    #[cfg(target_os = "android")]
    fn mark_gateway_stale(&self) {
        self.gateway_stale.store(true, Ordering::Release);
    }

    /// Android:取走并清零失效标记
    #[cfg(target_os = "android")]
    fn take_gateway_stale(&self) -> bool {
        self.gateway_stale.swap(false, Ordering::AcqRel)
    }

    fn ready_seen(&self) -> u64 {
        self.ready_seen.load(Ordering::SeqCst)
    }

    fn register(&self, id: String) -> oneshot::Receiver<GatewayReply> {
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .expect("gateway pending lock")
            .insert(id, tx);
        rx
    }

    pub fn resolve_reply(&self, reply: GatewayReply) {
        if let Some(tx) = self
            .pending
            .lock()
            .expect("gateway pending lock")
            .remove(&reply.id)
        {
            let _ = tx.send(reply);
        }
    }
}

/// 网关站点注册表:label -> (起始 URL,窗口标题)
fn site(label: &str) -> Option<(&'static str, &'static str)> {
    match label {
        "hlib" => Some(("https://hlib.cc/", "小说源连接")),
        _ => None,
    }
}

// 网关页面与宿主间的回传桥:主框架导航 `ygwd-gw://` + on_navigation
// 拦截——主框架导航不受页面 CSP connect-src/frame-src 约束(navigate-to
// 已废弃),hlib 挑战页 CSP 仅放行 'self' 与 challenges.cloudflare.com,
// 旧版回环 fetch 与 IPC fetch 传输均被拦导致网关死路;导航式零 CSP 交互。
// Android 的 IPC 走 JavaBridge(非 fetch)不受页面 CSP——实机与源码
// 核查确认后,Android IPC 优先分支已移除(见 git 历史),导航式为唯一
// 通道;JavaBridge 通道的实机行为待 Android 实机验证后再评估接线

/// 从 query 串取单参数(不做 percent 解码,解码在取值后单独处理)
fn parse_query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|kv| {
        let (k, v) = kv.split_once('=')?;
        (k == key).then_some(v.to_string())
    })
}

fn label_from_registry(_host: &str) -> String {
    // 当前唯一网关站点;site() 注册表扩展后此处按 host 反查
    "hlib".to_string()
}

/// percent 解码(encodeURIComponent 形态:%XX;'+' 不转空格)
fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let hex = std::str::from_utf8(&b[i + 1..=i + 2]).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// 解析导航式回传:ygwd-gw://r?id=I&len=L&d=urlencoded("status|body")
/// → GatewayReply(len 为原文长度,解码后校验完整性)
fn parse_nav_reply(query: &str) -> Option<GatewayReply> {
    let mut id = None;
    let mut len = None;
    let mut d = None;
    for kv in query.split('&') {
        let (k, v) = kv.split_once('=')?;
        match k {
            "id" => id = Some(v.to_string()),
            "len" => len = Some(v.parse::<usize>().ok()?),
            "d" => d = Some(v.to_string()),
            _ => {}
        }
    }
    let payload = percent_decode(&d?)?;
    if payload.len() != len? {
        return None;
    }
    let (status_str, body) = payload.split_once('|')?;
    Some(GatewayReply {
        id: id?,
        status: status_str.parse::<u16>().ok()?,
        body: body.to_string(),
    })
}

/// 回调内(Android 为主线程)不直接操作窗口:收回动作投到异步运行时执行,
/// 回调本身只做纯内存操作(解析/oneshot 投递/原子计数),杜绝主线程重入
fn finish_off_main_thread(app: &AppHandle, label: String) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = finish(&app, &label);
    });
}

/// 网关导航分流:处理 ygwd-gw 协议的 finish/r/ready 三类报到,
/// 返回是否放行该导航(ygwd-gw 一律 false 阻止;其他 scheme 放行)
fn handle_gateway_navigation(app: &AppHandle, url: &tauri::Url) -> bool {
    use tauri::Manager as _;
    if url.scheme() != "ygwd-gw" {
        return true; // 正常站点导航放行
    }
    let host = url.host_str().unwrap_or("");
    let query = url.query().unwrap_or("");
    match host {
        // 完成栏:ygwd-gw://finish?label=...
        "finish" => {
            let label =
                parse_query_param(query, "label").unwrap_or_else(|| label_from_registry(host));
            diag(app, "[通道观测] gwfinish 经导航通道到达");
            finish_off_main_thread(app, label);
        }
        // 抓取回传:ygwd-gw://r?id=..&len=..&d=..
        "r" => {
            diag(app, "[通道观测] gwresult 经导航通道到达");
            match parse_nav_reply(query) {
                Some(reply) => {
                    // 临时取证:hlib 页面(样本特征 title「H图书馆」)落盘供
                    // 离线 DOM 结构分析(不渲染查看);验收后移除。目录用
                    // DIAG_DIR 缓存(任何线程安全)
                    if let Some(dir) = DIAG_DIR.get() {
                        if reply.body.contains("图书馆") || reply.body.contains("hlib.cc/s/") {
                            let ts = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis();
                            let _ = std::fs::write(
                                dir.join(format!("hlib_sample_{ts}.html")),
                                &reply.body,
                            );
                        }
                    }
                    app.state::<GatewayManager>().resolve_reply(reply.clone());
                    // 收回交给 gateway_fetch_inner 的成功路径(非挑战非闸才收):
                    // 此处无差别收回会在挑战态把网关藏起,用户无法交互验证
                    diag(
                        app,
                        &format!("[网关链路] 回传到达 bytes={}", reply.body.len()),
                    );
                }
                None => diag(app, "[通道观测] gwresult 解析失败"),
            }
        }
        // 就绪报到:ygwd-gw://ready?n=序号(非挑战态才会发出)
        "ready" => {
            if let Some(n) = parse_query_param(query, "n").and_then(|v| v.parse::<u64>().ok()) {
                app.state::<GatewayManager>().mark_ready(n);
            }
        }
        _ => diag(app, "[通道观测] 未知 ygwd-gw 导航已拦截"),
    }
    false // 阻止真实导航,页面留在原地
}

/// 完成栏注入 JS:底部悬浮「完成,返回应用」(放底部:顶部会盖住站点导航栏
/// 的登录入口,Android 全面屏下还与状态栏重叠、易误触通知),点击经主框架导航
/// `ygwd-gw://finish` 通知宿主(on_navigation 拦截处理,页面不跳转)
fn finish_bar_js(label: &str) -> String {
    let label_json = serde_json::to_string(label).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(function() {{
          function ensureBar() {{
            if (document.getElementById('ygwd-gw-bar')) return;
            var bar = document.createElement('div');
            bar.id = 'ygwd-gw-bar';
            bar.style.cssText = 'position:fixed;bottom:0;left:0;right:0;z-index:2147483647;background:#0066cc;color:#fff;padding:14px 14px max(env(safe-area-inset-bottom), 16px);text-align:center;font-size:16px;font-weight:600;cursor:pointer';
            bar.textContent = '\\u2713 完成,返回应用';
            bar.addEventListener('click', function() {{
              location.href = 'ygwd-gw://finish?label=' + encodeURIComponent({label_json});
            }});
            var host = document.body || document.documentElement;
            host.appendChild(bar);
          }}
          ensureBar();
          setInterval(ensureBar, 1500);
        }})();"#
    )
}

/// 确保指定站点的网关 webview 存在。返回是否为本次新建。
pub fn ensure_gateway(app: &AppHandle, label: &str) -> Result<bool, String> {
    use tauri::Manager as _;
    let window_label = format!("gateway_{label}");
    // Android:系统回收 GatewayActivity 后 Rust 窗口对象失效(exec 报
    // Activity 不存在),close 清注册后走下方建窗流程完整重建
    // (activity_name 重新 startActivity 重建 Activity 与 webview)
    #[cfg(target_os = "android")]
    if app.state::<GatewayManager>().take_gateway_stale() {
        diag(app, "[网关链路] 检测到失效标记,close 旧网关窗口重建");
        if let Some(w) = app.get_webview_window(&window_label) {
            let _ = w.close();
        }
    }
    if app.get_webview_window(&window_label).is_some() {
        return Ok(false);
    }
    let Some((start_url, title)) = site(label) else {
        return Err(format!("未知网关站点 {label:?}"));
    };
    let url: tauri::Url = start_url
        .parse()
        .map_err(|e| format!("无效网关地址: {e}"))?;
    // 完成栏注入用 on_page_load(Android 上 initialization_script 不可靠);
    // 每次页面加载完成都会重新注入
    let bar_js = finish_bar_js(label);
    let builder = WebviewWindowBuilder::new(app, &window_label, WebviewUrl::External(url));
    // Android:网关放进独立的 GatewayActivity(仓库 overlay 提供,见
    // src-tauri/android-overlay)。wry 在 Android 上一个 Activity 只挂一个
    // webview,eval 一律投给「最后创建者」;与主界面同处 MainActivity 时,
    // 主界面的 IPC 回包(Android 经 eval 回调)全被投进网关,invoke 永不返回
    #[cfg(target_os = "android")]
    let builder = builder.activity_name("GatewayActivity");
    let window = builder
        .title(title)
        .inner_size(400.0, 720.0)
        // 静默化:后台创建不抢焦点——JS 挑战自动求解在隐藏 webview 照常
        // 运行,就绪后抓取成功自动收回,用户全程无感;交互式挑战在重试
        // 轮次足够后前置转人工(见 gateway_fetch_inner)。Android 的
        // Activity 启动由系统管理,visible 属性大概率无效(如实)
        .visible(false)
        .on_navigation({
            let app = app.clone();
            move |url| {
                // 入口诊断:记录每条导航(截断防撑爆日志)——Android 退出
                // 瞬间的最后一条入口 diag 即触发源定位证据
                // 参数键名入日志,不含 d 值(d 携带页面全文,脱敏防日志膨胀)
                let keys = url
                    .query()
                    .map(|q| {
                        q.split('&')
                            .filter_map(|kv| kv.split('=').next())
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .unwrap_or_default();
                diag(
                    &app,
                    &format!(
                        "[导航入口] {}://{} keys={keys}",
                        url.scheme(),
                        url.host_str().unwrap_or(""),
                    ),
                );
                // panic 屏障:回调经 JNI 在 Android 主线程执行,Rust panic
                // 穿过 JNI 边界无法 unwind 会直接 abort 进程(「应用退出」
                // 候选①);拦截后记录 panic 摘要并阻止导航,桌面无 panic
                // 时零差异
                let allow = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handle_gateway_navigation(&app, url)
                }))
                .unwrap_or_else(|payload| {
                    let msg = payload
                        .downcast_ref::<&str>()
                        .map(|s| (*s).to_string())
                        .or_else(|| payload.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| format!("{payload:?}"));
                    diag(&app, &format!("[PANIC 屏障] 导航回调 panic 已拦截: {msg}"));
                    false // panic 态一律阻止真实导航
                });
                allow
            }
        })
        .on_page_load({
            move |webview, payload| {
                if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                    let _ = webview.eval(&bar_js);
                }
            }
        })
        .build()
        .map_err(|e| format!("创建网关失败: {e}"))?;
    // 用户点关闭键时改为隐藏:保住登录/验证会话
    let close_label = window_label.clone();
    let app_owned = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            if let Some(w) = app_owned.get_webview_window(&close_label) {
                let _ = w.hide();
            }
        }
    });
    Ok(true)
}

/// 就绪探测 JS:页面非挑战态时经导航通道 `ygwd-gw://ready?n=序号` 报到;
/// 挑战态(含站点反爬闸「人机校验」)保持静默——不在挑战页上发起导航,免得顶替挑战自身的表单提交。
/// (旧实现改写 document.title 再读 window.title(),但原生窗口标题不随
/// document.title 同步,恒为构建时的「小说源连接」,就绪永远判不出,
/// 每次首建网关都要白等满 150 秒超时)
fn ready_probe_js(nonce: u64) -> String {
    format!(
        r#"(function() {{
          if (window.__ygwdStale) return;
          var t = document.title || '';
          var cf = !!(window._cf_chl_opt
            || document.querySelector('iframe[src*="challenges.cloudflare"], #challenge-form, #challenge-stage'));
          if (/请稍候|Just a moment|人机校验/i.test(t) || cf) return;
          location.href = 'ygwd-gw://ready?n={nonce}';
        }})();"#
    )
}

/// 等待网关站点就绪(挑战通过):每轮注入就绪探测并发新序号,收到不小于
/// 本轮序号的报到即视为就绪。返回是否就绪(超时返回 false,由调用方照常尝试)。
async fn wait_until_ready(window: &tauri::WebviewWindow, timeout_secs: u64) -> bool {
    use tauri::Manager as _;
    let app = window.app_handle();
    let manager = app.state::<GatewayManager>();
    let started = tokio::time::Instant::now();
    let deadline = started + Duration::from_secs(timeout_secs);
    loop {
        let nonce = manager.next_ready_nonce();
        let _ = window.eval(ready_probe_js(nonce));
        // 报到经导航通道异步到达,短轮询等本轮回音
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(300)).await;
            if manager.ready_seen() >= nonce {
                diag(
                    app,
                    &format!("[网关链路] 站点就绪 耗时={}s", started.elapsed().as_secs()),
                );
                return true;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            diag(
                app,
                &format!("[网关链路] 就绪等待超时 {timeout_secs}s(挑战未通过或页面无响应)"),
            );
            return false;
        }
    }
}

/// 确保网关存在且站点就绪(挑战通过);期间用户可能在网关窗口操作
async fn ensure_ready(app: &AppHandle, label: &str) -> Result<tauri::WebviewWindow, String> {
    use tauri::Manager as _;
    let created = ensure_gateway(app, label)?;
    let window = app
        .get_webview_window(&format!("gateway_{label}"))
        .ok_or("网关窗口不存在")?;
    if created {
        wait_until_ready(&window, 150).await;
    }
    Ok(window)
}

/// 在网关页面上下文执行抓取并把结果 POST 回本地代理
fn dispatch_fetch(
    window: &tauri::WebviewWindow,
    manager: &GatewayManager,
    url: &str,
) -> Result<tokio::sync::oneshot::Receiver<GatewayReply>, String> {
    let id = format!(
        "g{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let rx = manager.register(id.clone());
    let js_url = serde_json::to_string(url).map_err(|e| e.to_string())?;
    // 双形态:JSON 字符串字面量(带引号,invoke 参数用)与裸值(导航 URL query 用)
    // ——URL 里若用带引号形态,reply.id 与 pending 注册的裸 id 不匹配,回传全空转
    let js_id = serde_json::to_string(&id).map_err(|e| e.to_string())?;
    let url_id = &id;

    let js = format!(
        r#"(async () => {{
  const post = (status, body) => {{
    // len 语义:UTF-8 字节数(与 Rust 侧 parse_nav_reply 的 payload.len()
    // 字节校验一致;JS 的 String.length 是 UTF-16 单元数,含中文时必不匹配)
    const payload = status + '|' + body;
    const payloadLen = new TextEncoder().encode(payload).length;
    // 主框架导航回传(on_navigation 拦截,不真跳转,零 CSP 交互)。
    // Android 的 JavaBridge IPC 优先分支已移除:实机与源码核查确认该
    // 传输在 Linux 被页面 CSP 拦(死路),导航式是唯一可靠通道(on_navigation 拦截,不真跳转,零 CSP 交互)
    location.href = 'ygwd-gw://r?id={url_id}&len=' + payloadLen + '&d=' + encodeURIComponent(payload);
  }};
  try {{
    const r = await fetch({js_url}, {{ credentials: "include" }});
    const body = await r.text();
    post(r.status, body);
  }} catch (e) {{
    post(0, "E:" + String(e));
  }}
}})();"#
    );
    window.eval(&js).map_err(|e| format!("网关执行失败: {e}"))?;
    Ok(rx)
}

/// hlib 站点自有反爬闸「伊蕾娜」(/antibot,title「人机校验」):按会话访问
/// 频率触发,页面自动 render Turnstile,token 交 /api/n_bot_v2 后跳回目标页。
/// 页内 fetch 拿到的只是闸页文本,必须让网关主框架亲自走一遍校验。
fn is_antibot_gate(body: &str) -> bool {
    body.contains("/api/n_bot_v2") || body.contains("<title>人机校验</title>")
}

/// 统一网关抓取入口:发请求、判定挑战、静默重试(网关已在前台供
/// 用户操作)、成功后隐藏网关并回主界面。返回成功状态的 body。
async fn gateway_fetch_inner(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    url: &str,
) -> Result<String, String> {
    let window = ensure_ready(app, label).await?;
    let mut rx = dispatch_fetch(&window, manager, url)?;
    diag(
        app,
        &format!("[网关链路] dispatch 发起 label={label} url={url}"),
    );

    let mut last_status: Option<u16> = None;
    for i in 0..4u32 {
        let reply = match tokio::time::timeout(Duration::from_secs(60), rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => {
                diag(app, "[网关链路] 回传通道中断(oneshot 关闭)");
                return Err("网关回传中断".into());
            }
            Err(_) => {
                diag(
                    app,
                    "[网关链路] dispatch 60 秒无回传(挑战页 CSP 拦回传或页面未完成)",
                );
                return Err("网关请求超时:请点开应用内网关窗口完成验证/登录后重试".into());
            }
        };
        let status = reply.status;
        // 挑战判定从紧:仅认 403 状态或短小的挑战标题页。
        // 不做正文包含判断——正常站点页面也会引用 challenge-platform 脚本
        let is_challenge = status == 403
            || (reply.body.len() < 6000
                && (reply.body.contains("Just a moment") || reply.body.contains("请稍候")));
        let is_gate = is_antibot_gate(&reply.body);
        diag(
            app,
            &format!(
                "[网关链路] 回传到达 attempt status={status} bytes={} is_challenge={is_challenge} is_gate={is_gate}",
                reply.body.len()
            ),
        );
        if !is_challenge && !is_gate {
            // 成功:收回网关(会话保留),主界面回前台
            let _ = finish(app, label);
            return Ok(reply.body);
        }
        last_status = Some(status);
        if is_gate {
            // 反爬闸:主框架导航到目标页(服务端会转 /antibot),Turnstile 在
            // 网关里亲自跑;通过后页面自动跳回目标页,就绪探测随之报到。
            // 未就绪不重发请求——重发只会再撞闸并累加访问频率
            diag(app, "[网关链路] 命中伊蕾娜反爬闸,网关导航至校验页");
            // 先给当前文档打陈旧标记:导航提交前旧文档仍存活,不打标会被
            // 就绪探测误报为「已就绪」(实测 0 秒假就绪);新文档 window 全新无标记
            let _ = window.eval("window.__ygwdStale = true;");
            if let Ok(target) = url.parse::<tauri::Url>() {
                let _ = window.navigate(target);
            }
            present(&window);
            if !wait_until_ready(&window, 120).await {
                return Err(
                    "hlib 人机校验未通过:已在弹出窗口打开校验页,请等待勾选完成(或手动点一下)后重试"
                        .into(),
                );
            }
        } else {
            // CF 挑战:先静默等待自动求解(JS 挑战在隐藏 webview 照常跑);
            // 第 2 轮起前置转人工(交互式 Turnstile 需要用户点击,隐藏态
            // 永远过不去)——前置即打扰,仅在自动求解无望时发生
            if i >= 1 {
                diag(
                    app,
                    &format!("[网关链路] 挑战第 {} 轮未就绪,前置转人工", i + 1),
                );
                window.show().ok();
                window.set_focus().ok();
            }
            diag(app, "[网关链路] 挑战态,等待就绪后重试");
            wait_until_ready(&window, 60).await;
        }
        rx = dispatch_fetch(&window, manager, url)?;
        diag(app, "[网关链路] 重试 dispatch 已发起");
    }
    // 全部重试失败:网关带回前台让用户看到站点状态
    present(&window);
    match last_status {
        Some(status) => Err(format!(
            "网关未完成(状态 {status});已在网关窗口打开站点,请完成验证/登录后重试"
        )),
        None => Err("网关无响应".into()),
    }
}

/// 公开入口:经网关抓取页面文本
pub async fn gateway_fetch(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    url: &str,
) -> Result<String, String> {
    gateway_fetch_inner(app, manager, label, url).await
}

/// 完成网关操作:隐藏网关并把主窗口带回前台
pub fn finish(app: &AppHandle, label: &str) -> Result<(), String> {
    use tauri::Manager as _;
    #[cfg(target_os = "android")]
    if let Some(window) = app.get_webview_window(&format!("gateway_{label}")) {
        android_gateway_to_back(&window);
    }
    #[cfg(not(target_os = "android"))]
    {
        if let Some(window) = app.get_webview_window(&format!("gateway_{label}")) {
            let _ = window.hide();
        }
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.show();
            let _ = main.set_focus();
        }
    }
    Ok(())
}

/// 把网关导航到指定站内地址并带到前台(登录墙:直接落在登录表单,
/// 免得用户在站点折叠菜单里找登录入口)
pub fn navigate_and_present(app: &AppHandle, label: &str, url: &str) {
    use tauri::Manager as _;
    let Some(window) = app.get_webview_window(&format!("gateway_{label}")) else {
        return;
    };
    if let Ok(target) = url.parse::<tauri::Url>() {
        let _ = window.navigate(target);
    }
    present(&window);
}

/// 把网关窗口带到前台供用户操作(验证/登录/人机校验)
pub fn present(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "android")]
    {
        android_gateway_to_front(window);
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Android 收回网关:把 GatewayActivity 所在任务栈退到后台,主界面回前台。
/// 只退不 finish——Activity 与 webview 保活,Rust 侧网关窗口对象持续有效,
/// 后续抓取照常在后台网关里执行(tao 的 set_visible 在 Android 是空实现,
/// show/hide 无效,只能走 Activity 任务栈)。
#[cfg(target_os = "android")]
fn android_gateway_to_back(window: &tauri::WebviewWindow) {
    android_exec_on_gateway(window, "退后台", |env, activity, _webview| {
        env.call_method(activity, "moveTaskToBack", "(Z)Z", &[true.into()])
            .map(|_| ())
    });
}

/// Android 前置网关:以 GatewayActivity 自身类重新 startActivity。清单里
/// 是 singleTask + 独立 taskAffinity,已存在的实例走 onNewIntent 被整栈
/// 拉回前台,不会重建(也就不经 tao 的建窗流程)。调用时主界面在前台,
/// 不受后台启动 Activity 的限制。
#[cfg(target_os = "android")]
fn android_gateway_to_front(window: &tauri::WebviewWindow) {
    android_exec_on_gateway(window, "前置", |env, activity, _webview| {
        use tauri::wry::prelude::jni::objects::JValue;
        // FLAG_ACTIVITY_NEW_TASK | FLAG_ACTIVITY_REORDER_TO_FRONT
        const FLAGS: i32 = 0x1000_0000 | 0x0002_0000;
        let class = env.get_object_class(activity)?;
        let intent = env.new_object(
            "android/content/Intent",
            "(Landroid/content/Context;Ljava/lang/Class;)V",
            &[JValue::Object(activity), JValue::Object(&class)],
        )?;
        env.call_method(
            &intent,
            "addFlags",
            "(I)Landroid/content/Intent;",
            &[JValue::Int(FLAGS)],
        )?;
        env.call_method(
            activity,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[JValue::Object(&intent)],
        )
        .map(|_| ())
    });
}

/// 在网关所属 Activity 的主线程上执行一段 JNI 操作(exec 递来的 activity 即
/// GatewayActivity——每个 Activity 有自己的消息管道)。结果入 diag;挂起的
/// Java 异常一律清掉,避免污染后续 JNI 调用。
#[cfg(target_os = "android")]
fn android_exec_on_gateway<F>(window: &tauri::WebviewWindow, action: &'static str, op: F)
where
    F: FnOnce(
            &mut tauri::wry::prelude::JNIEnv,
            &tauri::wry::prelude::JObject,
            &tauri::wry::prelude::JObject,
        ) -> tauri::wry::prelude::jni::errors::Result<()>
        + Send
        + 'static,
{
    use tauri::Manager as _;
    let app = window.app_handle().clone();
    let result = window.with_webview(move |platform| {
        platform.jni_handle().exec(move |env, activity, webview| {
            let outcome = if activity.is_null() {
                app.state::<GatewayManager>().mark_gateway_stale();
                "Activity 不存在(系统回收,标记失效待重建)".to_string()
            } else {
                match op(env, activity, webview) {
                    Ok(()) => "完成".to_string(),
                    Err(e) => format!("失败: {e}"),
                }
            };
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
            diag(&app, &format!("[网关显隐] Android 网关{action}:{outcome}"));
        });
    });
    if let Err(e) = result {
        use tauri::Manager as _;
        diag(
            window.app_handle(),
            &format!("[网关显隐] with_webview 失败: {e}"),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enc(s: &str) -> String {
        // encodeURIComponent 等价(测试辅助):非保留字符逐字节 %XX
        let mut out = String::new();
        for b in s.bytes() {
            let c = b as char;
            if c.is_ascii_alphanumeric() || "-_.!~*'()".contains(c) {
                out.push(c);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    }

    #[test]
    fn parses_nav_reply_ascii() {
        let body = "403|Just a moment";
        let payload = format!("403|{body}");
        let query = format!("id=g123&len={}&d={}", payload.len(), enc(&payload));
        let reply = parse_nav_reply(&query).expect("应解析成功");
        assert_eq!(reply.id, "g123");
        assert_eq!(reply.status, 403);
        assert_eq!(reply.body, body);
    }

    #[test]
    fn parses_nav_reply_utf8_body() {
        // 文章正文为 UTF-8 中文:percent 解码后须还原原文且 len 校验通过
        let payload = "200|第三章 雪夜(试)——作者：某人";
        let query = format!("id=g9&len={}&d={}", payload.len(), enc(payload));
        let reply = parse_nav_reply(&query).expect("UTF-8 应解析成功");
        assert_eq!(reply.status, 200);
        assert_eq!(reply.body, payload.split_once('|').unwrap().1);
    }

    #[test]
    fn detects_antibot_gate() {
        // 闸页结构取证(2026-11 样本,文案以占位代替):title「人机校验」+
        // Turnstile 回调提交 /api/n_bot_v2
        let gate = r#"<html><head><title>人机校验</title></head><body>
          <div id="recaptcha"></div><script>turnstile.render('#recaptcha', {callback: function(t) {
          $.ajax({url: '/api/n_bot_v2', type: 'post'}); }});</script></body></html>"#;
        assert!(is_antibot_gate(gate));
        // 正常系列/搜索页(含 Turnstile 无关的同名词)不误判
        let normal = r#"<html><head><title>系列搜索</title></head><body>
          <a href="/me/email">验证邮箱</a><li class="list-group-item"></li></body></html>"#;
        assert!(!is_antibot_gate(normal));
        // 挑战页上就绪探测保持静默:探测 JS 含闸标题判据
        assert!(ready_probe_js(1).contains("人机校验"));
        // 导航提交前的旧文档(陈旧标记)不得报到
        assert!(ready_probe_js(1).contains("__ygwdStale"));
    }

    #[test]
    fn ready_nonce_tracks_max_seen() {
        let m = GatewayManager::new();
        let a = m.next_ready_nonce();
        let b = m.next_ready_nonce();
        assert!(b > a);
        assert!(ready_probe_js(b).contains(&format!("ygwd-gw://ready?n={b}")));
        // 迟到的旧报到不回退已见序号
        m.mark_ready(b);
        m.mark_ready(a);
        assert_eq!(m.ready_seen(), b);
    }

    #[test]
    fn rejects_len_mismatch_and_bad_input() {
        let payload = "200|ok";
        let query = format!("id=g1&len=999&d={}", enc(payload));
        assert!(parse_nav_reply(&query).is_none(), "len 不符应拒绝");
        assert!(
            parse_nav_reply("id=g1&len=6&d=%E6%88%90").is_none(),
            "截断的 UTF-8 应拒绝"
        );
        assert!(
            parse_nav_reply("id=g1&d=200%7Cok").is_none(),
            "缺 len 应拒绝"
        );
        assert!(parse_query_param("label=hlib", "label") == Some("hlib".to_string()));
        assert!(percent_decode("a%20b%2Fc").as_deref() == Some("a b/c"));
    }
}
