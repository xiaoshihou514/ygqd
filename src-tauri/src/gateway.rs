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
    let created = ensure_gateway(app, label)?;
    if created {
        // 新建窗口需要等首页加载完成(或挑战通过)后才具备 fetch 条件,
        // 这里给出基础等待;调用方对首访失败可重试
        tokio::time::sleep(Duration::from_secs(3)).await;
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
    let js = format!(
        r#"(async () => {{
  try {{
    const r = await fetch({js_url}, {{ credentials: "include" }});
    const body = await r.text();
    window.__TAURI_INTERNALS__.invoke("gateway_result", {{
      id: {js_id}, status: r.status, body
    }});
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
    Ok(reply.body)
}
