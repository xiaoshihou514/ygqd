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

    // 发起一次网关 fetch:注册回传 id -> eval JS -> JS 经 invoke 回传结果
    let dispatch = |window: &tauri::WebviewWindow,
                    manager: &GatewayManager|
     -> Result<tokio::sync::oneshot::Receiver<GatewayReply>, String> {
        let id = format!(
            "g{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let rx = manager.register(id.clone());
        let js_id = serde_json::to_string(&id).map_err(|e| e.to_string())?;
        let js = format!(
            r#"(async () => {{
  let status = 0;
  let body = "";
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
      }}
      if (!found) found = findSearchForm(doc);
      if (!found) throw new Error("未找到搜索表单");
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
      status = r.status;
      body = await r.text();
      window.__TAURI_INTERNALS__.invoke("gateway_result", {{
        id: {js_id}, status: status, body: body
      }});
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
          }} catch (e) {{}}
        }}
        body = found;
      }}
      status = r.status;
      window.__TAURI_INTERNALS__.invoke("gateway_result", {{
        id: {js_id}, status: status, body: body
      }});
    }}
  }} catch (e) {{
    window.__TAURI_INTERNALS__.invoke("gateway_result", {{
      id: {js_id}, status: 0, body: String(e)
    }});
  }}
}})();"#
        );
        window.eval(&js).map_err(|e| format!("网关执行失败: {e}"))?;
        Ok(rx)
    };

    // 主循环:发请求 -> 判定挑战/失败 -> 弹网关等用户 -> 重试(最多 4 次)
    let mut rx_opt = Some(dispatch(&window, manager)?);
    let mut last: Option<(u16, String)> = None;
    for attempt in 0..4 {
        let rxi = rx_opt.take().expect("rx present");
        let reply = match tokio::time::timeout(Duration::from_secs(60), rxi).await {
            Ok(Ok(r)) => r,
            Ok(Err(_)) => return Err("网关回传中断".into()),
            Err(_) => return Err("网关请求超时(网关可能未完成验证)".into()),
        };
        let status = reply.status;
        // 挑战判定从紧:仅认 403 状态或短小的挑战标题页。
        // 不做正文包含判断——正常站点页面也会引用 challenge-platform 脚本,
        // 误报会导致网关反复抢前台把用户按在站点页面
        let body_for_check = reply.body.clone();
        let is_challenge = status == 403
            || (body_for_check.len() < 6000
                && (body_for_check.contains("Just a moment") || body_for_check.contains("请稍候")));
        last = Some((status, reply.body));
        if !is_challenge {
            break;
        }
        // 静默重试:不抢前台(用户可能正在主界面看结果);
        // 全部重试失败后在最终错误里引导用户打开网关
        wait_until_ready(&window, 45).await;
        rx_opt = Some(dispatch(&window, manager)?);
        let _ = attempt;
    }
    let Some((status, body)) = last else {
        return Err("网关无回传".into());
    };
    if status == 0 {
        return Err(format!("网关 fetch 失败: {body}"));
    }
    if status >= 400 {
        return Err(format!("网关源返回 HTTP {status}"));
    }
    // 成功:隐藏网关转后台(会话保留)
    let _ = window.hide();
    Ok(body)
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
        let id = format!(
            "nav{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let rx = manager.register(id.clone());
        let id_json = serde_json::to_string(&id).map_err(|e| e.to_string())?;
        window
            .eval(&format!(
                r#"window.__TAURI_INTERNALS__.invoke('gateway_result', {{
                  id: {id_json},
                  status: 200,
                  body: document.documentElement.outerHTML
                }});"#
            ))
            .map_err(|e| format!("取页面失败: {e}"))?;
        let reply = tokio::time::timeout(Duration::from_secs(30), rx)
            .await
            .map_err(|_| "取页面超时".to_string())
            .and_then(|r| r.map_err(|_| "回传中断".to_string()))?;
        let html = reply.body;
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
