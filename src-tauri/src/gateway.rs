//! WebView 网关:借应用内嵌 webview 的浏览器环境(真 Chrome 网络栈 +
//! 持久 cookie)绕过 Cloudflare 指纹/交互挑战,供 hlib(小说)与
//! missav(视频播放)抓取使用。
//!
//! 工作方式:每个站点一个懒创建的 webview("gateway_{label}"),
//! 只在创建时导航一次到站点首页;之后 gateway_fetch 通过 eval 在
//! 该页面上下文执行同源 fetch,JS 把结果经共享 invoke_handler 的
//! gateway_result 命令回传,Rust 侧按请求 id 用 oneshot 匹配。
//! 首次创建时窗口可见(用户点一次 CF 挑战),通过后前端可调用
//! gateway_set_visible 隐藏。

use std::{collections::HashMap, sync::Mutex, time::Duration};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

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

#[derive(Default)]
pub struct GatewayManager {
    pending: Mutex<HashMap<String, oneshot::Sender<GatewayReply>>>,
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

    pub(crate) fn resolve(&self, reply: GatewayReply) {
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

/// 确保指定站点的网关 webview 存在。返回是否为本次新建。
pub fn ensure_gateway(app: &AppHandle, label: &str) -> Result<bool, String> {
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
    let label_json = serde_json::to_string(label).unwrap_or_else(|_| "\"hlib\"".into());
    // gateway_result 命令注册在 app 级 invoke_handler,所有 webview 共享。
    // initialization_script 注入悬浮「完成返回」栏:Android 上网关窗口会
    // 占据前台,用户完成验证/登录后点它回到主应用,否则流程被打断。
    let window = WebviewWindowBuilder::new(app, &window_label, WebviewUrl::External(url))
        .title(title)
        .inner_size(400.0, 720.0)
        // 首次创建时可见:若遇 CF 交互挑战,用户点一次即可,cookie 持久化
        .visible(true)
        .on_page_load({
            let label_json = label_json.clone();
            move |webview, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                let _ = webview.eval(&format!(
                    r#"(function() {{
                      var label = {label_json};
                      function ensureBar() {{
                        if (document.getElementById('ygwd-gw-bar')) return;
                        var bar = document.createElement('div');
                        bar.id = 'ygwd-gw-bar';
                        bar.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:2147483647;background:#0066cc;color:#fff;padding:14px;text-align:center;font-size:16px;font-weight:600;cursor:pointer';
                        bar.textContent = '\u2713 完成,返回应用';
                        bar.addEventListener('click', function() {{
                          window.__TAURI_INTERNALS__.invoke('gateway_finish', {{ label: label }});
                        }});
                        var host = document.body || document.documentElement;
                        host.appendChild(bar);
                      }}
                      ensureBar();
                      setInterval(ensureBar, 1500);
                    }})();"#
                ));
            }
            }
        })
        .initialization_script(&format!(
            r#"(function() {{
              var label = {label_json};
              function ensureBar() {{
                if (document.getElementById('ygwd-gw-bar')) return;
                var bar = document.createElement('div');
                bar.id = 'ygwd-gw-bar';
                bar.style.cssText = 'position:fixed;top:0;left:0;right:0;z-index:2147483647;background:#0066cc;color:#fff;padding:14px;text-align:center;font-size:16px;font-weight:600;cursor:pointer';
                bar.textContent = '\u2713 完成,返回应用';
                bar.addEventListener('click', function() {{
                  window.__TAURI_INTERNALS__.invoke('gateway_finish', {{ label: label }});
                }});
                var host = document.body || document.documentElement;
                host.appendChild(bar);
              }}
              ensureBar();
              setInterval(ensureBar, 1500);
            }})();"#
        ))
        .build()
        .map_err(|e| format!("创建网关失败: {e}"))?;
    let _ = &window;
    Ok(true)
}

/// 在网关页面上下文执行 fetch 并等待桥回传
pub async fn gateway_fetch(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    url: &str,
) -> Result<String, String> {
    gateway_fetch_inner(app, manager, label, url, false, None).await
}

/// 在 hlib 页面上下文中提交站内搜索表单,沿用用户的登录状态。
pub async fn gateway_search(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    keyword: &str,
) -> Result<String, String> {
    gateway_fetch_inner(
        app,
        manager,
        label,
        "https://hlib.cc/",
        false,
        Some(keyword),
    )
    .await
}

/// 同 gateway_fetch;extract_m3u8 为真时在页面上下文内执行
/// missav 的 eval(packer) 混淆并从中提取首个 m3u8 地址
pub async fn gateway_fetch_extract_m3u8(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    url: &str,
) -> Result<String, String> {
    gateway_fetch_inner(app, manager, label, url, true, None).await
}

async fn gateway_fetch_inner(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    url: &str,
    extract_m3u8: bool,
    search_keyword: Option<&str>,
) -> Result<String, String> {
    use tauri::Manager as _;
    let created = ensure_gateway(app, label)?;
    let window = app
        .get_webview_window(&format!("gateway_{label}"))
        .ok_or("网关窗口不存在")?;
    if created {
        // 新建网关:站点可能停在 CF 挑战/登录页,等用户完成(最长 150 秒)
        wait_until_ready(&window, 150).await;
    }

    let js_url = serde_json::to_string(url).map_err(|e| e.to_string())?;
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

    // 发起网关抓取:结果经 history.replaceState 写入同源 URL 路径,
    // Rust 侧轮询 window.url() 读取。
    // (远程页面没有 Tauri IPC 桥,invoke 回传方案不可行)
    let dispatch = |window: &tauri::WebviewWindow| -> Result<(), String> {
        let js = format!(
            r#"(async () => {{
  const put = (s) => {{
    try {{
      const b = btoa(unescape(encodeURIComponent(s))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
      history.replaceState(null, '', '/__ygwd_' + b);
    }} catch (e2) {{
      history.replaceState(null, '', '/__ygwd_E_' + encodeURIComponent(s).slice(0, 1500));
    }}
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
      if (!found) {{ put("E:未找到搜索表单"); return; }}
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
      put(r.status + "|" + body);
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
      put(r.status + "|" + body);
    }}
  }} catch (e) {{
    put("0|E:" + String(e));
  }}
}})();"#
        );
        window.eval(&js).map_err(|e| format!("网关执行失败: {e}"))
    };

    // 轮询 replaceState 标记读取结果
    let read_marker = |window: &tauri::WebviewWindow| -> Result<Option<(u16, String)>, String> {
        let u = window.url().map_err(|e| e.to_string())?;
        let Some(b64) = u.path().strip_prefix("/__ygwd_") else {
            return Ok(None);
        };
        if b64.is_empty() {
            return Ok(None);
        }
        use base64::Engine as _;
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(b64)
            .or_else(|_| base64::engine::general_purpose::STANDARD.decode(b64))
            .map_err(|e| format!("结果解码失败: {e}"))?;
        let body = String::from_utf8(bytes).unwrap_or_default();
        Ok(Some((200, body)))
    };

    // 主循环:发请求 -> 轮询标记 -> 挑战则弹网关等用户 -> 重试(最多 4 轮)
    let mut last: Option<(u16, String)> = None;
    let mut first = true;
    for _attempt in 0..4 {
        dispatch(&window)?;
        let deadline =
            tokio::time::Instant::now() + Duration::from_secs(if first { 100 } else { 70 });
        loop {
            if let Some((status, body)) = read_marker(&window)? {
                // 清理标记
                let _ = window.eval("history.replaceState(null, '', '/');");
                let is_challenge = status == 403
                    || (body.len() < 6000
                        && (body.contains("Just a moment") || body.contains("请稍候")));
                last = Some((status, body.clone()));
                if !is_challenge {
                    let _ = window.hide();
                    if let Some(main) = app.get_webview_window("main") {
                        let _ = main.show();
                        let _ = main.set_focus();
                    }
                    return Ok(body);
                }
                break; // 挑战:走重试
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        first = false;
        // 未拿到有效结果:把网关带回前台等用户(可能需要登录/验证),再重试
        let _ = window.show();
        let _ = window.set_focus();
        wait_until_ready(&window, 60).await;
    }
    if let Some((status, body)) = last {
        if status == 200 && !body.is_empty() && body.len() > 6000 {
            // 有真实内容但流程未走完:视为成功返回
            let _ = window.hide();
            return Ok(body);
        }
        return Err(format!(
            "网关未完成(状态 {status});请点开应用内网关完成登录/验证后重试"
        ));
    }
    Err("网关无响应:请在弹出的网关窗口完成验证/登录".into())
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

/// 判断网关当前页面是否仍在 CF 挑战/未就绪
async fn page_not_ready(window: &tauri::WebviewWindow) -> bool {
    window
        .eval(
            r#"(function() {
              var t = document.title || '';
              var b = (document.body && document.body.innerText) || '';
              var cf = !!(document.querySelector('iframe[src*="challenges.cloudflare"]') || document.getElementById('challenge-form'));
              window.__YGWD_READY__ = !( /请稍候|Just a moment/i.test(t) || /正在进行安全验证|security verification|Verifying you are human/i.test(b) || cf );
            })();"#,
        )
        .is_err()
}

/// 轮询等待网关站点就绪(挑战通过),timeout_secs 总上限
async fn wait_until_ready(window: &tauri::WebviewWindow, timeout_secs: u64) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        if tokio::time::Instant::now() >= deadline {
            return;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
        if !page_not_ready(window).await {
            return;
        }
    }
}

/// 导航式抓取:让网关窗口直接导航到目标页(浏览器完整处理 CF,
/// 包括 JS 渲染),等待渲染稳定后取 outerHTML。
/// 若落在挑战页:保持网关在前台等用户点验证,期间轮询直到出现
/// 真实内容或超时。
pub async fn navigate_and_dump(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    url: &str,
    settle_ms: u64,
) -> Result<String, String> {
    use tauri::Manager as _;
    ensure_gateway(app, label)?;
    let window = app
        .get_webview_window(&format!("gateway_{label}"))
        .ok_or("网关窗口不存在")?;
    // 前台显示(用户可能需要过验证)
    let _ = window.show();
    let _ = window.set_focus();

    let url_json = serde_json::to_string(url).map_err(|e| e.to_string())?;
    window
        .eval(&format!("window.location.href = {url_json};"))
        .map_err(|e| format!("导航失败: {e}"))?;

    // 等待导航+渲染稳定(settle 毫秒),期间检测挑战页;挑战时等用户操作
    let deadline = tokio::time::Instant::now() + Duration::from_secs(150);
    loop {
        tokio::time::sleep(Duration::from_millis(settle_ms)).await;
        // 取 DOM:经 replaceState 写入同源路径(Remote 页无 IPC 桥),
        // 再轮询 window.url() 读取
        window
            .eval(
                r#"(function() {
                  try {
                    const b = btoa(unescape(encodeURIComponent(document.documentElement.outerHTML))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
                    history.replaceState(null, '', '/__ygwdnav_' + b);
                  } catch (e) {}
                })();"#,
            )
            .map_err(|e| format!("取页面失败: {e}"))?;
        // 轮询标记(outerHTML 较大,给 8 秒)
        let mut html = String::new();
        let dl2 = tokio::time::Instant::now() + Duration::from_secs(8);
        loop {
            let u = window.url().map_err(|e| e.to_string())?;
            if let Some(b64) = u.path().strip_prefix("/__ygwdnav_") {
                use base64::Engine as _;
                if let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(b64) {
                    html = String::from_utf8(bytes).unwrap_or_default();
                }
                let _ = window.eval(&format!("history.replaceState(null, '', {url_json});"));
                break;
            }
            if tokio::time::Instant::now() >= dl2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(400)).await;
        }
        let challenged = html.len() < 20000
            && (html.contains("Just a moment")
                || html.contains("请稍候")
                || html.contains("challenge-platform"));
        if !challenged {
            // 拿到真实内容:隐藏网关回主界面
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
        // 继续等用户点验证(网关保持在最前台)
        let _ = window.show();
    }
}
