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
    let builder = WebviewWindowBuilder::new(app, &window_label, WebviewUrl::External(url))
        .title(title)
        .inner_size(400.0, 720.0)
        // 首次创建时可见:若遇 CF 交互挑战,用户点一次即可,cookie 持久化
        .visible(true);
    // gateway_result 命令注册在 app 级 invoke_handler,所有 webview 共享
    builder.build().map_err(|e| format!("创建网关失败: {e}"))?;
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
    let created = ensure_gateway(app, label)?;
    if created {
        // 新建窗口需要等首页加载完成(或挑战通过)后才具备 fetch 条件,
        // 这里给出基础等待;调用方对首访失败可重试
        tokio::time::sleep(Duration::from_secs(6)).await;
    }
    let window = app
        .get_webview_window(&format!("gateway_{label}"))
        .ok_or("网关窗口不存在")?;
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
        status = home.status;
        body = html;
        if (home.ok) {{
          doc = new DOMParser().parseFromString(html, "text/html");
          baseUrl = home.url;
          found = findSearchForm(doc);
        }}
      }}
      if (found) {{
        const {{ form, input }} = found;
        const formData = new FormData(form);
        formData.set(input.name, keyword);
        const submit = form.querySelector('button[type="submit"], input[type="submit"]');
        if (submit && submit.name && !formData.has(submit.name)) formData.set(submit.name, submit.value || "");
        const action = new URL(form.getAttribute("action") || baseUrl, baseUrl);
        if (action.origin !== location.origin) throw new Error("hlib 搜索表单地址不在本站");
        const method = (form.method || "GET").toUpperCase();
        const options = {{ method, credentials: "include" }};
        if (method === "GET" || method === "HEAD") {{
          for (const [key, value] of formData.entries()) {{
            if (typeof value === "string") action.searchParams.append(key, value);
          }}
        }} else if (form.enctype.includes("multipart/form-data")) {{
          options.body = formData;
        }} else {{
          const encoded = new URLSearchParams();
          for (const [key, value] of formData.entries()) {{
            if (typeof value === "string") encoded.append(key, value);
          }}
          options.headers = {{ "Content-Type": "application/x-www-form-urlencoded;charset=UTF-8" }};
          options.body = encoded.toString();
        }}
        const result = await fetch(action.toString(), options);
        status = result.status;
        body = await result.text();
      }} else if (!/请稍候|Just a moment|正在进行安全验证|security verification/i.test(body)) {{
        // 未找到表单时交给 Rust 侧继续尝试已知路径。
        body = "";
      }}
    }} else {{
      const r = await fetch({js_url}, {{ credentials: "include" }});
      status = r.status;
      body = await r.text();
      if ({extract}) {{
        // missav 的播放地址藏在 eval(p,a,c,k,e,d) packer 里;
        // 页面上下文即 JS 环境,new Function 执行后抽取 m3u8
        let found = "";
        const blocks = body.match(/eval\(function\(p,a,c,k,e,d\)\{{[\s\S]*?\}}\)\)/g) || [];
        for (const block of blocks) {{
          try {{
            const out = new Function("return " + block.replace(/^eval/, ""))();
            const urls = String(out).match(/https?:\/\/[^"'\s]+\.m3u8[^"'\s]*/g);
            if (urls && urls.length) {{ found = urls[0]; break; }}
          }} catch (e) {{}}
        }}
        body = found;
      }}
    }}
    window.__TAURI_INTERNALS__.invoke("gateway_result", {{ id: {js_id}, status, body }});
  }} catch (e) {{
    window.__TAURI_INTERNALS__.invoke("gateway_result", {{
      id: {js_id}, status: 0, body: String(e)
    }});
  }}
}})();"#
    );
    window.eval(&js).map_err(|e| format!("网关执行失败: {e}"))?;
    let reply = tokio::time::timeout(Duration::from_secs(30), rx)
        .await
        .map_err(|_| "网关请求超时".to_string())
        .and_then(|r| r.map_err(|_| "网关回传中断".to_string()))?;
    if reply.status == 0 {
        return Err(format!("网关 fetch 失败: {}", reply.body));
    }
    if reply.status >= 400 {
        return Err(format!("网关源返回 HTTP {}", reply.status));
    }
    // 首次成功说明挑战/登录已就绪,隐藏窗口转为后台网关
    if let Some(window) = app.get_webview_window(&format!("gateway_{label}")) {
        let _ = window.hide();
    }
    Ok(reply.body)
}
