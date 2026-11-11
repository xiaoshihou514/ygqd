//! 本地流代理服务(仅绑定 127.0.0.1,不对外监听)。
//!
//! 职责:
//! 1. HLS 转发:/hls/{b64url(真实地址)} —— m3u8 文本重写子地址为代理
//!    地址;分片/其他资源带站点 Referer 转发,Range 请求透传上游,
//!    已取分片做内存缓存(插入序 FIFO,超限约 96MB 时批量淘汰最旧
//!    一半;频繁快进/回看零重复下载)
//! 2. 图片转发:/img/{b64url} —— 防盗链封面(带站点 Referer);
//!    已实现但当前无调用方(封面统一走 fetch_image IPC 取回二进制)
//! 3. 网关回传:POST /gwresult —— 远程页面(无 IPC 桥)把抓取结果
//!    回传给 Rust(替代所有依赖 webkit 行为的方案)
//!
//! 为什么必须有本地代理:上游 CDN 校验 Referer/拒绝浏览器 TLS 栈,
//! 页面侧任何直连方式都不可行;而回环 HTTP 是所有 WebView 100%
//! 支持的标准通道(业界视频应用的标准架构)。

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
use regex::Regex;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Mutex, OnceLock},
    time::Duration,
};

/// 分片缓存:code -> (content_type, bytes),上限 ~96MB
const CACHE_LIMIT: usize = 96 * 1024 * 1024;

#[derive(Default)]
struct Cache {
    map: Mutex<HashMap<String, (String, Vec<u8>)>>,
    order: Mutex<Vec<String>>,
    size: Mutex<usize>,
}

impl Cache {
    fn get(&self, key: &str) -> Option<(String, Vec<u8>)> {
        self.map.lock().ok()?.get(key).cloned()
    }
    fn put(&self, key: String, ct: String, bytes: Vec<u8>) {
        let Ok(mut size) = self.size.lock() else {
            return;
        };
        if *size + bytes.len() > CACHE_LIMIT {
            // 淘汰最旧的一半
            let Ok(mut order) = self.order.lock() else {
                return;
            };
            let drop_n = order.len() / 2;
            let dropped: Vec<String> = order.drain(..drop_n).collect();
            if let Ok(mut map) = self.map.lock() {
                for k in &dropped {
                    if let Some((_, b)) = map.remove(k) {
                        *size = size.saturating_sub(b.len());
                    }
                }
            }
        }
        *size += bytes.len();
        if let Ok(mut order) = self.order.lock() {
            order.push(key.clone());
        }
        if let Ok(mut map) = self.map.lock() {
            map.insert(key, (ct, bytes));
        }
    }
}

static CACHE: OnceLock<Cache> = OnceLock::new();
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

/// setup 时注入 AppHandle(供网关回传写入 state)
pub fn set_app(app: tauri::AppHandle) {
    let _ = APP.set(app);
}
static PORT: OnceLock<u16> = OnceLock::new();

fn cache() -> &'static Cache {
    CACHE.get_or_init(Cache::default)
}

pub fn port() -> u16 {
    PORT.get().copied().unwrap_or(0)
}

/// 代理地址封装:真实 HLS 地址 -> http://127.0.0.1:{port}/hls/{b64}
pub fn hls_url(real: &str) -> String {
    format!(
        "http://127.0.0.1:{}/hls/{}",
        port(),
        B64.encode(real.as_bytes())
    )
}

fn referer_for(url: &str) -> &'static str {
    if url.contains("surrit.com") {
        "https://missav.ai/"
    } else {
        // mushroomtrack/fs1/fourhoi 及其余一律 fs1(referer 校验同源)
        "https://fs1.app/"
    }
}

fn decode_b64(segment: &str) -> Option<String> {
    B64.decode(segment)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
}

/// 把 m3u8 文本中的子地址重写为本代理地址(相对地址先解析为绝对)
fn rewrite_playlist(text: &str, base: &str) -> String {
    let re_key_uri = Regex::new(r#"URI="([^"]+)""#).expect("static regex");
    let mut out = String::with_capacity(text.len() + 512);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("#EXT-X-KEY") || trimmed.starts_with("#EXT-X-MAP") {
            // 加密密钥/初始化段地址:重写 URI="..." 为代理地址(hls.js
            // 会请求该地址拿 AES-128 密钥,相对地址+防盗链必须转发)
            let rewritten_line = re_key_uri
                .replace_all(trimmed, |caps: &regex::Captures| {
                    let raw = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                    let absolute = if raw.starts_with("http://") || raw.starts_with("https://") {
                        raw.to_string()
                    } else if let Ok(b) = url::Url::parse(base) {
                        b.join(raw)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|_| raw.to_string())
                    } else {
                        raw.to_string()
                    };
                    format!("URI=\"{}\"", hls_url(&absolute))
                })
                .to_string();
            out.push_str(&rewritten_line);
            out.push('\n');
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out.push_str(line);
        } else {
            let absolute = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
                trimmed.to_string()
            } else if let Ok(b) = url::Url::parse(base) {
                b.join(trimmed)
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| trimmed.to_string())
            } else {
                trimmed.to_string()
            };
            out.push_str(&hls_url(&absolute));
        }
        out.push('\n');
    }
    out
}

fn upstream_fetch(url: &str, range: Option<&str>) -> Result<(String, Vec<u8>), u16> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36",
        )
        .build()
        .map_err(|_| 500u16)?;
    let mut req = client.get(url).header("Referer", referer_for(url));
    if let Some(r) = range {
        req = req.header("Range", r);
    }
    let resp = req.send().map_err(|_| 502u16)?;
    let status = resp.status().as_u16();
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = resp.bytes().map_err(|_| 502u16)?.to_vec();
    if status >= 400 {
        return Err(status);
    }
    Ok((ct, bytes))
}

fn handle_hls(path_b64: &str, range: Option<&str>) -> (u16, String, Vec<u8>, bool) {
    let Some(real) = decode_b64(path_b64) else {
        return (400, "text/plain".into(), b"bad url".to_vec(), false);
    };
    let head = real.ends_with(".m3u8");
    // 分片优先查缓存(m3u8 不缓存,内容小且需每次重写)
    if !head {
        if let Some((ct, bytes)) = cache().get(&real) {
            return (200, ct, bytes, false);
        }
    }
    match upstream_fetch(&real, range) {
        Ok((ct, bytes)) => {
            let head2 = ct.contains("mpegurl")
                || ct.contains("m3u8")
                || head
                || (bytes.len() >= 7 && &bytes[..7] == b"#EXTM3U");
            if head2 {
                if let Ok(text) = String::from_utf8(bytes.clone()) {
                    let rewritten = rewrite_playlist(&text, &real);
                    return (
                        200,
                        "application/vnd.apple.mpegurl".into(),
                        rewritten.into_bytes(),
                        false,
                    );
                }
            }
            // 分片入缓存
            let ct2 = if ct.is_empty() {
                "application/octet-stream".to_string()
            } else {
                ct.clone()
            };
            cache().put(real, ct2, bytes.clone());
            (
                200,
                if ct.is_empty() {
                    "application/octet-stream".into()
                } else {
                    ct
                },
                bytes,
                false,
            )
        }
        Err(status) => (
            status,
            "text/plain".into(),
            format!("upstream {status}").into_bytes(),
            false,
        ),
    }
}

fn handle_img(path_b64: &str) -> (u16, String, Vec<u8>) {
    let Some(real) = decode_b64(path_b64) else {
        return (400, "text/plain".into(), b"bad url".to_vec());
    };
    let key = format!("img:{real}");
    if let Some((ct, bytes)) = cache().get(&key) {
        return (200, ct, bytes);
    }
    match upstream_fetch(&real, None) {
        Ok((ct, bytes)) => {
            let ct = if ct.is_empty() {
                "image/jpeg".into()
            } else {
                ct
            };
            cache().put(key, ct.clone(), bytes.clone());
            (200, ct, bytes)
        }
        Err(status) => (
            status,
            "text/plain".into(),
            format!("upstream {status}").into_bytes(),
        ),
    }
}

fn respond(stream: &mut TcpStream, status: u16, ct: &str, body: Vec<u8>, extra: &[(&str, &str)]) {
    let reason = match status {
        200 => "OK",
        206 => "Partial Content",
        400 => "Bad Request",
        404 => "Not Found",
        s => {
            let _ = s;
            "Error"
        }
    };
    let mut head = format!("HTTP/1.1 {status} {reason}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n", body.len());
    for (k, v) in extra {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    head.push_str("\r\n");
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(&body);
    let _ = stream.flush();
}

fn urldecode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("");
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
            }
            b'+' => out.push(b' '),
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

/// 拉取 HLS 资源:m3u8 返回重写后的文本;分片返回原始字节。
/// blocking 网络 IO 经 spawn_blocking,不阻塞异步执行器
pub async fn fetch_hls(url: &str) -> Result<(bool, String, Vec<u8>), String> {
    let owned = url.to_string();
    // CDN 偶发 502(限流/瞬时故障):指数间隔重试 3 次
    let mut last_err: Option<String> = None;
    for wait in [0u64, 1500, 4000] {
        let owned = owned.clone();
        if wait > 0 {
            tokio::time::sleep(Duration::from_millis(wait)).await;
        }
        match tokio::task::spawn_blocking(move || fetch_hls_blocking(&owned))
            .await
            .map_err(|e| e.to_string())?
        {
            Ok(r) => return Ok(r),
            Err(e) => {
                let retryable = e.contains("502") || e.contains("503") || e.contains("429");
                last_err = Some(e);
                if !retryable {
                    break;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| "hls 拉取失败".into()))
}

/// 阻塞版(供同步 handler 与 spawn_blocking)
fn fetch_hls_blocking(url: &str) -> Result<(bool, String, Vec<u8>), String> {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    if host.is_empty() {
        return Err("无效地址".into());
    }
    let (ct, bytes) = upstream_fetch(url, None).map_err(|s| format!("CDN {s}"))?;
    let head = &bytes[..bytes.len().min(7)];
    let is_playlist = ct.contains("mpegurl")
        || ct.contains("m3u8")
        || url.ends_with(".m3u8")
        || (ct.is_empty() && head == b"#EXTM3U");
    if is_playlist {
        let text = String::from_utf8(bytes).map_err(|_| "m3u8 不是有效 UTF-8".to_string())?;
        return Ok((true, rewrite_playlist(&text, url), Vec::new()));
    }
    Ok((false, ct, bytes))
}

fn handle_client(mut stream: TcpStream) {
    let Ok(reader_clone) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(reader_clone);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    // 读取全部头
    let mut headers = String::new();
    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                if line == "\r\n" || line == "\n" {
                    break;
                }
                headers.push_str(&line);
            }
            Err(_) => return,
        }
    }
    let range = headers
        .lines()
        .find_map(|l| l.to_lowercase().strip_prefix("range: ").map(str::to_string))
        .map(|s| s.trim().trim_end_matches("\r").to_string());
    let content_length = headers
        .lines()
        .find_map(|l| {
            l.to_lowercase()
                .strip_prefix("content-length: ")
                .and_then(|v| v.trim().parse::<usize>().ok())
        })
        .unwrap_or(0);
    // 读取 POST body
    let mut body_bytes = vec![0u8; content_length];
    if content_length > 0 && reader.read_exact(&mut body_bytes).is_err() {
        return;
    }

    // 解析路径与 query
    let target = request_line
        .split_whitespace()
        .nth(1)
        .unwrap_or("")
        .to_string();
    let path = target.split('?').next().unwrap_or("").to_string();

    if let Some(rest) = path.strip_prefix("/hls/") {
        let (status, ct, body, _) = handle_hls(rest, range.as_deref());
        respond(
            &mut stream,
            status,
            &ct,
            body,
            &[
                ("Access-Control-Allow-Origin", "*"),
                ("Accept-Ranges", "bytes"),
            ],
        );
    } else if let Some(rest) = path.strip_prefix("/img/") {
        let (status, ct, body) = handle_img(rest);
        respond(
            &mut stream,
            status,
            &ct,
            body,
            &[("Access-Control-Allow-Origin", "*")],
        );
    } else {
        respond(&mut stream, 404, "text/plain", b"not found".to_vec(), &[]);
    }
}

/// 启动本地代理服务(仅 127.0.0.1),返回实际端口。
/// 独立线程 + 专用阻塞 runtime(协议 handler 为同步上下文)。
pub fn spawn() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let _ = PORT.set(port);
    std::thread::Builder::new()
        .name("ygqd-proxy".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .expect("proxy runtime");
            for stream in listener.incoming() {
                match stream {
                    Ok(stream) => {
                        rt.spawn_blocking(move || handle_client(stream));
                    }
                    Err(_) => continue,
                }
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(port)
}
