//! WebView 网关:借应用内嵌 webview 的浏览器环境(真 Chrome 网络栈 +
//! 持久 cookie)处理 Cloudflare 验证与登录,供 hlib(小说)抓取使用。
//! (视频源 missav 已改为 wreq 指纹直连,不再走网关)
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
        _ => None,
    }
}

/// 网关页面与宿主间的回传桥:主框架导航 `ygwd-gw://` + on_navigation
/// 拦截——主框架导航不受页面 CSP connect-src/frame-src 约束(navigate-to
/// 已废弃),hlib 挑战页 CSP 仅放行 'self' 与 challenges.cloudflare.com,
/// 旧版回环 fetch 与 IPC fetch 传输均被拦导致网关死路;导航式零 CSP 交互。
/// Android 的 IPC 走 JavaBridge(非 fetch)可能不受 CSP,故 Android 优先
/// 试 IPC,失败回退导航式(YGWD_IPC_ERR 写 title 留判据)。
/// IPC 失败原因写入 document.title(YGWD_IPC_ERR 前缀)供 Rust 诊断读取。
const IPC_BRIDGE_ANDROID: &str = r#"async function ygwdInvoke(cmd, payload) {
  const t = window.__TAURI_INTERNALS__;
  if (!(t && typeof t.invoke === 'function')) {
    document.title = 'YGWD_IPC_ERR|IPC_UNAVAILABLE';
    throw new Error('IPC_UNAVAILABLE');
  }
  return t.invoke(cmd, payload).catch(function(e) {
    document.title = 'YGWD_IPC_ERR|INVOKE_FAIL|' + String(e).slice(0, 180);
    throw e;
  });
}"#;

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

/// 完成栏注入 JS:顶部悬浮「完成,返回应用」,点击经主框架导航
/// `ygwd-gw://finish` 通知宿主(on_navigation 拦截处理,页面不跳转)
fn finish_bar_js(label: &str) -> String {
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
        .on_navigation({
            let app = app.clone();
            move |url| {
                if url.scheme() != "ygwd-gw" {
                    return true; // 正常站点导航放行
                }
                use tauri::Manager as _;
                let host = url.host_str().unwrap_or("");
                let query = url.query().unwrap_or("");
                match host {
                    // 完成栏:ygwd-gw://finish?label=...
                    "finish" => {
                        let label = parse_query_param(query, "label")
                            .unwrap_or_else(|| label_from_registry(host));
                        diag(&app, "[通道观测] gwfinish 经导航通道到达");
                        let _ = finish(&app, &label);
                    }
                    // 抓取回传:ygwd-gw://r?id=..&len=..&d=..
                    "r" => {
                        diag(&app, "[通道观测] gwresult 经导航通道到达");
                        match parse_nav_reply(query) {
                            Some(reply) => {
                                app.state::<GatewayManager>().resolve_reply(reply.clone());
                                // P3 自动收回:结果回传即收网关回主界面
                                let _ = finish(&app, "hlib");
                                diag(
                                    &app,
                                    &format!(
                                        "[网关链路] 回传到达 bytes={} 自动收回网关",
                                        reply.body.len()
                                    ),
                                );
                            }
                            None => diag(&app, "[通道观测] gwresult 解析失败"),
                        }
                    }
                    _ => diag(&app, "[通道观测] 未知 ygwd-gw 导航已拦截"),
                }
                false // 阻止真实导航,页面留在原地
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
    // 双形态:JSON 字符串字面量(带引号,invoke 参数用)与裸值(导航 URL query 用)
    // ——URL 里若用带引号形态,reply.id 与 pending 注册的裸 id 不匹配,回传全空转
    let js_id = serde_json::to_string(&id).map_err(|e| e.to_string())?;
    let url_id = &id;
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
    // len 语义:UTF-8 字节数(与 Rust 侧 parse_nav_reply 的 payload.len()
    // 字节校验一致;JS 的 String.length 是 UTF-16 单元数,含中文时必不匹配)
    const payload = status + '|' + body;
    const payloadLen = new TextEncoder().encode(payload).length;
    // Android: IPC 走 JavaBridge(非 fetch)可能不受页面 CSP——优先试,
    // 失败原因写 title(YGWD_IPC_ERR 判据)后回退导航式
    const android = /android/i.test(navigator.userAgent);
    if (android && window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {{
      window.__TAURI_INTERNALS__.invoke('gateway_result', {{ result: {{ id: {js_id}, status: status, body: body }} }})
        .catch(function(e) {{
          document.title = 'YGWD_IPC_ERR|INVOKE_FAIL|' + String(e).slice(0, 180);
          location.href = 'ygwd-gw://r?id={url_id}&len=' + payloadLen + '&d=' + encodeURIComponent(payload);
        }});
      return;
    }}
    // 桌面/通用:主框架导航回传(on_navigation 拦截,不真跳转,零 CSP 交互)
    location.href = 'ygwd-gw://r?id={url_id}&len=' + payloadLen + '&d=' + encodeURIComponent(payload);
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
      const body = await r.text();
      post(r.status, body);
    }}
  }} catch (e) {{
    post(0, "E:" + String(e));
  }}
}})();"#
    );
    window.eval(&js).map_err(|e| format!("网关执行失败: {e}"))?;
    // IPC 失败观测:页面 ygwdInvoke 失败时会把 YGWD_IPC_ERR|原因 写入
    // document.title(写 title 不受页面 CSP 约束),3 秒后读取入 diag
    {
        use tauri::Manager as _;
        let diag_window = window.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(3)).await;
            for _ in 0..2 {
                if let Ok(t) = diag_window.title() {
                    if let Some(reason) = t.strip_prefix("YGWD_IPC_ERR|") {
                        diag(
                            diag_window.app_handle(),
                            &format!("[IPC 诊断] invoke 失败: {reason}"),
                        );
                        return;
                    }
                }
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        });
    }
    Ok(rx)
}

/// 统一网关抓取入口:发请求、判定挑战、静默重试(网关已在前台供
/// 用户操作)、成功后隐藏网关并回主界面。返回成功状态的 body。
async fn gateway_fetch_inner(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    url: &str,
    search_keyword: Option<&str>,
) -> Result<String, String> {
    use tauri::Manager as _;
    let window = ensure_ready(app, label).await?;
    let mut rx = dispatch_fetch(&window, manager, url, search_keyword)?;
    diag(
        app,
        &format!(
            "[网关链路] dispatch 发起 label={label} url={url} search={}",
            search_keyword.is_some()
        ),
    );

    let mut last: Option<(u16, String)> = None;
    for _attempt in 0..4 {
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
        let body_for_check = reply.body.clone();
        // 挑战判定从紧:仅认 403 状态或短小的挑战标题页。
        // 不做正文包含判断——正常站点页面也会引用 challenge-platform 脚本
        let is_challenge = status == 403
            || (body_for_check.len() < 6000
                && (body_for_check.contains("Just a moment") || body_for_check.contains("请稍候")));
        diag(
            app,
            &format!(
                "[网关链路] 回传到达 attempt status={status} bytes={} is_challenge={is_challenge}",
                body_for_check.len()
            ),
        );
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
        diag(app, "[网关链路] 挑战态,等待就绪后重试");
        wait_until_ready(&window, 60).await;
        rx = dispatch_fetch(&window, manager, url, search_keyword)?;
        diag(app, "[网关链路] 重试 dispatch 已发起");
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
    gateway_fetch_inner(app, manager, label, url, None).await
}

/// 公开入口:经网关提交站点搜索表单并返回结果页文本(hlib)
pub async fn gateway_search(
    app: &AppHandle,
    manager: &GatewayManager,
    label: &str,
    keyword: &str,
) -> Result<String, String> {
    gateway_fetch_inner(app, manager, label, "", Some(keyword)).await
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
