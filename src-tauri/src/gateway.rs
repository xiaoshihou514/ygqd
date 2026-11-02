//! WebView 网关:借应用内嵌 webview 的浏览器环境(真 Chrome 网络栈 +
//! 持久 cookie)处理 Cloudflare 验证与登录,供 hlib(小说)与
//! missav(视频播放地址解析)抓取使用。
//!
//! 回传通道(关键):网关加载的是远程页面,远程页没有 Tauri IPC 桥,
//! 一切 invoke 方案都不可行。回传统一走**本地回环 HTTP 代理**
//! (stream_proxy):网关 JS `fetch('http://127.0.0.1:{port}/gwresult',
//! {method:'POST', body})` ——回环 POST 是所有 WebView 100% 支持的
//! 标准行为,三端一致,不依赖任何 webkit 特性。
//!
//! 交互时序:首次创建网关时窗口可见并导航到站点首页;用户完成
//! CF 验证/登录(cookie 持久化,验证通常只需要一次);Rust 侧对
//! 挑战响应静默重试(最多约 4 轮),期间用户在网关窗口操作;
//! 成功后网关自动隐藏(会话保留),主窗口回到前台。

use std::{collections::HashMap, sync::Mutex, time::Duration};
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

use crate::stream_proxy;

/// 诊断日志:写应用内部数据目录 diag.log;查看走应用内入口
/// (release 无法 run-as/logcat,应用内读取是唯一通道)
pub fn diag(app: &AppHandle, msg: &str) {
    use std::io::Write;
    use tauri::Manager as _;
    let Ok(dir) = app.path().app_data_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
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
}

impl Default for GatewayManager {
    fn default() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
        }
    }
}

impl GatewayManager {
    pub fn new() -> Self {
        Self::default()
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
        "missav" => Some(("https://missav.ai/", "视频源连接")),
        _ => None,
    }
}

/// 完成栏注入 JS:顶部悬浮「完成,返回应用」,点击经本地回环代理通知
fn finish_bar_js(label: &str) -> String {
    let port = stream_proxy::port();
    let label_json = serde_json::to_string(label).unwrap_or_else(|_| "\"\"".into());
    format!(
        r#"(function() {{
          function ensureBar() {{
            if (document.getElementById('ygwd-gw-bar')) return;
            var bar = document.createElement('div');
            bar.id = 'ygwd-gw-bar';
            bar.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:2147483647;background:#0066cc;color:#fff;padding:14px;text-align:center;font-size:16px;font-weight:600;cursor:pointer';
            bar.textContent = '\\u2713 完成,返回应用';
            bar.addEventListener('click', function() {{
              fetch('http://127.0.0.1:{port}/gwfinish?label=' + encodeURIComponent({label_json}), {{ mode: 'no-cors' }});
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
    let window = WebviewWindowBuilder::new(app, &window_label, WebviewUrl::External(url))
        .title(title)
        .inner_size(400.0, 720.0)
        .visible(true)
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

/// 等待网关站点就绪(挑战通过):检测方式为 eval 改写 document.title,
/// 再读回 title()(同步 API,三端一致)
async fn wait_until_ready(window: &tauri::WebviewWindow, timeout_secs: u64) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        tokio::time::sleep(Duration::from_secs(3)).await;
        let _ = window.eval(
            r#"(function() {
              var t = document.title || '';
              var cf = !!(document.querySelector('iframe[src*="challenges.cloudflare"]') || document.getElementById('challenge-form'));
              document.title = (/请稍候|Just a moment/i.test(t) || cf) ? 'YGWD_CHALLENGE' : 'YGWD_READY';
            })();"#,
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
        match window.title() {
            Ok(t) if t.starts_with("YGWD_READY") => return,
            _ => {}
        }
        if tokio::time::Instant::now() >= deadline {
            return;
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
    extract_m3u8: bool,
    search_keyword: Option<&str>,
) -> Result<tokio::sync::oneshot::Receiver<GatewayReply>, String> {
    let port = stream_proxy::port();
    let id = format!(
        "g{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    let rx = manager.register(id.clone());
    let js_url = serde_json::to_string(url).map_err(|e| e.to_string())?;
    let js_id = serde_json::to_string(&id).map_err(|e| e.to_string())?;
    let extract = if extract_m3u8 { "true" } else { "false" };
    let search = if search_keyword.is_some() {
        "true"
    } else {
        "false"
    };
    let js_keyword = search_keyword
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| "null".into());

    let js = format!(
        r#"(async () => {{
  const post = (status, body) => {{
    fetch('http://127.0.0.1:{port}/gwresult?id={js_id}', {{
      method: 'POST',
      mode: 'no-cors',
      headers: {{ 'Content-Type': 'text/plain' }},
      body: status + '|' + body
    }});
  }};
  try {{
    if ({search}) {{
      const keyword = {js_keyword};
      const findSearchForm = (doc) => {{
        const fieldPattern = /search|keyword|query|(^|[^a-z])q([^a-z]|$)|(^|[^a-z])kw([^a-z]|$)|key|word|title|author|搜索|关键字|关键词|书名|作者/i;
        const forms = Array.from(doc.forms).map((form) => {{
          const fields = Array.from(form.querySelectorAll("input"))
            .filter((input) => input.name && !["hidden", "password", "submit", "button", "checkbox", "radio"].includes(input.type));
          const action = form.getAttribute("action") || "";
          const explicit = fields.find((input) => input.type === "search" || fieldPattern.test(
            [input.name, input.id, input.placeholder, input.getAttribute("aria-label")].join(" ")
          ));
          const fallback = /search|find|query|搜索/i.test(action + " " + form.id + " " + form.className)
            ? fields[0]
            : null;
          return {{ form, input: explicit || fallback, explicit: Boolean(explicit) }};
        }}).filter((item) => item.input);
        forms.sort((a, b) => Number(b.explicit) - Number(a.explicit));
        return forms[0] || null;
      }};
      let doc = document;
      let baseUrl = location.href;
      let found = findSearchForm(doc);
      if (!found) {{
        const home = await fetch("https://hlib.cc/", {{ credentials: "include" }});
        const html = await home.text();
        doc = new DOMParser().parseFromString(html, "text/html");
        baseUrl = home.url;
        found = findSearchForm(doc);
      }}
      if (!found) {{ post(0, "E:未找到搜索表单"); return; }}
      const target = new URL(found.form.getAttribute("action") || baseUrl, baseUrl);
      found.input.value = keyword;
      const hidden = Array.from(found.form.querySelectorAll('input[type="hidden"]'))
        .map((h) => h.name + "=" + encodeURIComponent(h.value))
        .join("&");
      const method = (found.form.getAttribute("method") || "GET").toUpperCase();
      const fetchInit = {{ method: method, credentials: "include" }};
      if (method === "POST") {{
        fetchInit.headers = {{ "Content-Type": "application/x-www-form-urlencoded; charset=UTF-8" }};
        fetchInit.body = hidden
          ? hidden + "&" + found.input.name + "=" + encodeURIComponent(keyword)
          : found.input.name + "=" + encodeURIComponent(keyword);
      }}
      const r = await fetch(target.href, fetchInit);
      const body = await r.text();
      post(r.status, body);
    }} else {{
      const r = await fetch({js_url}, {{ credentials: "include" }});
      let body = await r.text();
      if ({extract}) {{
        let found = "";
        const blocks = body.match(/eval\\(function\\(p,a,c,k,e,d\\)\\{{[\\s\\S]*?\\}}\\)\\)/g) || [];
        for (const block of blocks) {{
          try {{
            const out = new Function("return " + block.replace(/^eval/, ""))();
            const urls = String(out).match(/https?:\\/\\/[^"'\\s]+\\.m3u8[^"'\\s]*/g);
            if (urls && urls.length) {{ found = urls[0]; break; }}
          }} catch (e2) {{}}
        }}
        body = found;
      }}
      post(r.status, body);
    }}
  }} catch (e) {{
    post(0, "E:" + String(e));
  }}
}})();"#
    );
    window.eval(&js).map_err(|e| format!("网关执行失败: {e}"))?;
    Ok(rx)
}

/// 统一网关抓取入口:发请求、判定挑战、静默重试(网关已在前台供
/// 用户操作)、成功后隐藏网关并回主界面。返回成功状态的 body。
async fn gateway_fetch_inner(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    url: &str,
    extract_m3u8: bool,
    search_keyword: Option<&str>,
) -> Result<String, String> {
    use tauri::Manager as _;
    let window = ensure_ready(app, label).await?;
    let mut rx = dispatch_fetch(&window, manager, url, extract_m3u8, search_keyword)?;

    let mut last: Option<(u16, String)> = None;
    for _attempt in 0..4 {
        let reply = match tokio::time::timeout(Duration::from_secs(60), rx).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => return Err("网关回传中断".into()),
            Err(_) => return Err("网关请求超时:请点开应用内网关窗口完成验证/登录后重试".into()),
        };
        let status = reply.status;
        let body_for_check = reply.body.clone();
        // 挑战判定从紧:仅认 403 状态或短小的挑战标题页。
        // 不做正文包含判断——正常站点页面也会引用 challenge-platform 脚本
        let is_challenge = status == 403
            || (body_for_check.len() < 6000
                && (body_for_check.contains("Just a moment") || body_for_check.contains("请稍候")));
        if !is_challenge {
            // 成功:隐藏网关转后台(会话保留),主窗口回前台
            let _ = window.hide();
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.show();
                let _ = main.set_focus();
            }
            return Ok(reply.body);
        }
        last = Some((status, reply.body));
        // 挑战:静默重试(网关已在前台供用户操作,不重复抢焦点)
        wait_until_ready(&window, 60).await;
        rx = dispatch_fetch(&window, manager, url, extract_m3u8, search_keyword)?;
    }
    // 全部重试失败:网关带回前台让用户看到站点状态
    let _ = window.show();
    let _ = window.set_focus();
    match last {
        Some((status, body)) => Err(format!(
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
    gateway_fetch_inner(app, manager, label, url, false, None).await
}

/// 公开入口:经网关抓取并从页面提取首个 m3u8(missav 播放地址)
pub async fn gateway_fetch_extract_m3u8(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    url: &str,
) -> Result<String, String> {
    gateway_fetch_inner(app, manager, label, url, true, None).await
}

/// 公开入口:经网关提交站点搜索表单并返回结果页文本(hlib)
pub async fn gateway_search(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    keyword: &str,
) -> Result<String, String> {
    gateway_fetch_inner(app, manager, label, "", false, Some(keyword)).await
}

/// 完成网关操作:隐藏网关并把主窗口带回前台
pub fn finish(app: &AppHandle, label: &str) -> Result<(), String> {
    use tauri::Manager as _;
    if let Some(window) = app.get_webview_window(&format!("gateway_{label}")) {
        let _ = window.hide();
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
    Ok(())
}

/// 导航式抓取:网关窗口直接导航到目标页(浏览器完整处理 CF 与 JS
/// 渲染),轮询取 outerHTML(经本地代理 POST 回传)。
/// 适用于页面由 JS 渲染、fetch 拿不到内容的站点(missav 搜索页)。
pub async fn navigate_and_dump(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    url: &str,
    settle_ms: u64,
) -> Result<String, String> {
    use tauri::Manager as _;
    let window = ensure_ready(app, label).await?;
    let port = stream_proxy::port();
    let url_json = serde_json::to_string(url).map_err(|e| e.to_string())?;
    let _ = window.show();
    let _ = window.set_focus();
    window
        .eval(&format!("window.location.href = {url_json};"))
        .map_err(|e| format!("导航失败: {e}"))?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(150);
    loop {
        tokio::time::sleep(Duration::from_millis(settle_ms)).await;
        let id = format!(
            "nav{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let rx = manager.register(id.clone());
        let js_id = serde_json::to_string(&id).map_err(|e| e.to_string())?;
        window
            .eval(&format!(
                r#"(function() {{
                  try {{
                    const b = btoa(unescape(encodeURIComponent(document.documentElement.outerHTML))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
                    fetch('http://127.0.0.1:{port}/gwresult?id={js_id}', {{
                      method: 'POST', mode: 'no-cors',
                      headers: {{ 'Content-Type': 'text/plain' }},
                      body: '200|' + b
                    }});
                  }} catch (e) {{}}
                }})();"#
            ))
            .map_err(|e| format!("取页面失败: {e}"))?;
        let reply = tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| "取页面超时".to_string())
            .and_then(|r| r.map_err(|_| "回传中断".to_string()))?;
        let html = reply.body;
        let challenged =
            html.len() < 20000 && (html.contains("Just a moment") || html.contains("请稍候"));
        if !challenged {
            let _ = window.hide();
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.show();
                let _ = main.set_focus();
            }
            return Ok(html);
        }
        if tokio::time::Instant::now() >= deadline {
            return Err("网关页面仍在安全验证中,请稍后重试".into());
        }
        let _ = window.show();
    }
}
