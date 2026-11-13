//! 本地流代理服务(仅绑定 127.0.0.1,不对外监听)。
//!
//! 端点:
//! 1. `GET /hls/{b64url(真实地址)}` —— HLS 转发:m3u8 文本重写子地址
//!    (分片、`URI="…"` 属性)为代理地址;分片/其他资源带站点 Referer
//!    转发,Range 请求透传上游,已取分片做内存缓存(插入序,超限约
//!    96MB 时批量淘汰最旧一半;频繁快进/回看零重复下载)。
//! 2. `GET /img/{b64url}` —— 防盗链图片转发(带站点 Referer);
//!    已实现但当前无调用方(封面统一走 fetch_image IPC 取回二进制)。
//!
//! 为什么必须有本地代理:上游 CDN 校验 Referer/拒绝浏览器 TLS 栈,
//! 页面侧任何直连方式都不可行;而回环 HTTP 是所有 WebView 100%
//! 支持的标准通道(业界视频应用的标准架构)。
//!
//! 代码分层:请求解析(`read_request`)→ 路由(`route`)→ 处理器
//! (`serve_proxied`:缓存命中 `serve_cached` / 上游流式转发)。
//!
//! HTTP 约定:
//! - 每连接只处理一个请求(`Connection: close`):回环建连成本可忽略,
//!   且阻塞线程不会被 WebView 的空闲长连接占住;
//! - 支持 GET/HEAD/OPTIONS(CORS 预检),其余方法 405;
//! - 分片边收边发(不整段缓冲),长度已知发 `Content-Length`,
//!   未知用 chunked(中途断流时客户端可感知,不会把残段当完整分片);
//! - 上游错误:HTTP 状态原样透传,超时 504,网络错误 502。

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
use reqwest::header::{
    HeaderMap, HeaderName, ACCEPT_ENCODING, CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, RANGE,
    REFERER,
};
use std::{
    borrow::Cow,
    collections::{HashMap, VecDeque},
    fmt::Write as _,
    io::{self, BufRead, BufReader, BufWriter, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

// ---------------------------------------------------------------------------
// 配置
// ---------------------------------------------------------------------------

/// 内存缓存总上限(分片 + 图片)
const CACHE_LIMIT: usize = 96 * 1024 * 1024;
/// 单条缓存上限:超过的响应(如整段 mp4)只转发不缓存,避免挤掉全部分片
const CACHE_ENTRY_MAX: usize = 16 * 1024 * 1024;
/// 播放清单最大字节数(长片 VOD 清单通常 < 1MB)
const PLAYLIST_MAX: u64 = 16 * 1024 * 1024;
/// 请求行/单个头部行长度上限
const MAX_LINE: usize = 8 * 1024;
/// 全部请求头总长度上限
const MAX_HEADER_BYTES: usize = 32 * 1024;
/// 客户端读请求超时(防止半开连接长期占用线程)
const CLIENT_READ_TIMEOUT: Duration = Duration::from_secs(15);
/// 客户端写超时(播放器暂停不读时最多挂这么久)
const CLIENT_WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const UPSTREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// 上游超时:作用于建连+收头,以及流式读正文时的每次读取
const UPSTREAM_IO_TIMEOUT: Duration = Duration::from_secs(30);
/// 转发缓冲区大小
const COPY_BUF: usize = 64 * 1024;
const USER_AGENT: &str =
    "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36";
const M3U8_TYPE: &str = "application/vnd.apple.mpegurl";
/// 每个响应都带的 CORS 头(WebView 页面源与代理源不同)
const CORS_HEADERS: &str = "Access-Control-Allow-Origin: *\r\n\
     Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges\r\n";

// ---------------------------------------------------------------------------
// 全局状态
// ---------------------------------------------------------------------------

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
static PORT: OnceLock<u16> = OnceLock::new();

/// setup 时注入 AppHandle(原供网关回传端点使用;该端点已随小说板块
/// 删除,保留注入口以免 lib.rs 侧改动)
pub fn set_app(app: tauri::AppHandle) {
    let _ = APP.set(app);
}

/// 代理实际监听端口(未启动时为 0)
pub fn port() -> u16 {
    PORT.get().copied().unwrap_or(0)
}

/// 代理地址前缀:`http://127.0.0.1:{port}/hls/`
fn hls_prefix() -> String {
    format!("http://127.0.0.1:{}/hls/", port())
}

/// 代理地址封装:真实 HLS 地址 -> http://127.0.0.1:{port}/hls/{b64}
pub fn hls_url(real: &str) -> String {
    let mut out = hls_prefix();
    B64.encode_string(real.as_bytes(), &mut out);
    out
}

fn decode_b64(segment: &str) -> Option<String> {
    B64.decode(segment)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
}

// ---------------------------------------------------------------------------
// 内存缓存
// ---------------------------------------------------------------------------

/// 缓存条目。正文用 `Arc<[u8]>`:命中时只增引用计数,不复制整段分片。
#[derive(Clone)]
struct CacheEntry {
    content_type: String,
    /// 仅范围条目(上游 206)有值:上游 `Content-Range` 原文
    content_range: Option<String>,
    body: Arc<[u8]>,
}

/// 插入序缓存:超限时批量淘汰最旧一半(分片按播放顺序进入,
/// 插入序即近似访问序,不必维护 LRU 链)。
struct Cache {
    map: HashMap<String, CacheEntry>,
    order: VecDeque<String>,
    size: usize,
    limit: usize,
}

impl Cache {
    fn new(limit: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            size: 0,
            limit,
        }
    }

    fn get(&self, key: &str) -> Option<CacheEntry> {
        self.map.get(key).cloned()
    }

    fn put(&mut self, key: String, entry: CacheEntry) {
        let len = entry.body.len();
        // 并发请求同一分片时首个写入者胜出,避免重复计数
        if len > self.limit || self.map.contains_key(&key) {
            return;
        }
        while self.size + len > self.limit && !self.order.is_empty() {
            let n = (self.order.len() / 2).max(1);
            for k in self.order.drain(..n) {
                if let Some(old) = self.map.remove(&k) {
                    self.size = self.size.saturating_sub(old.body.len());
                }
            }
        }
        self.size += len;
        self.order.push_back(key.clone());
        self.map.insert(key, entry);
    }
}

fn cache() -> &'static Mutex<Cache> {
    static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(Cache::new(CACHE_LIMIT)))
}

fn cache_get(key: &str) -> Option<CacheEntry> {
    cache().lock().ok()?.get(key)
}

fn cache_put(key: String, entry: CacheEntry) {
    if let Ok(mut c) = cache().lock() {
        c.put(key, entry);
    }
}

/// 缓存键:完整资源为 `{prefix}{url}`,范围响应为 `{prefix}range:{range}|{url}`
fn cache_key(prefix: &str, url: &str, range: Option<&str>) -> String {
    match range {
        Some(r) => format!("{prefix}range:{r}|{url}"),
        None => format!("{prefix}{url}"),
    }
}

// ---------------------------------------------------------------------------
// HLS 清单
// ---------------------------------------------------------------------------

fn referer_for(url: &str) -> &'static str {
    if url.contains("surrit.com") {
        "https://missav.ai/"
    } else {
        // mushroomtrack/fs1/fourhoi 及其余一律 fs1(referer 校验同源)
        "https://fs1.app/"
    }
}

/// 地址路径是否以 `.m3u8` 结尾(忽略 query)
fn url_is_m3u8(u: &str) -> bool {
    url::Url::parse(u)
        .map(|p| p.path().ends_with(".m3u8"))
        .unwrap_or_else(|_| u.ends_with(".m3u8"))
}

/// 按 Content-Type / 地址 / 正文魔数判断是否为播放清单
fn is_playlist(content_type: &str, url: &str, prefix: &[u8]) -> bool {
    let ct = content_type.to_ascii_lowercase();
    ct.contains("mpegurl")
        || ct.contains("m3u8")
        || url_is_m3u8(url)
        || prefix.starts_with(b"#EXTM3U")
}

/// 清单中的地址解析为待代理的绝对地址;`None` 表示保持原样
/// (`data:`/`skd:` 等非 http 方案,代理无法也无须转发)。
fn resolve_url<'a>(raw: &'a str, base: Option<&url::Url>) -> Option<Cow<'a, str>> {
    if raw.starts_with("http://") || raw.starts_with("https://") {
        return Some(Cow::Borrowed(raw));
    }
    match url::Url::parse(raw) {
        Ok(abs) => matches!(abs.scheme(), "http" | "https").then_some(Cow::Borrowed(raw)),
        Err(_) => Some(
            base.and_then(|b| b.join(raw).ok())
                .map_or(Cow::Borrowed(raw), |u| Cow::Owned(u.into())),
        ),
    }
}

/// 把单个地址写成代理地址(或原样)追加到 `out`
fn push_proxied(raw: &str, base: Option<&url::Url>, prefix: &str, out: &mut String) {
    match resolve_url(raw, base) {
        Some(abs) => {
            out.push_str(prefix);
            B64.encode_string(abs.as_bytes(), out);
        }
        None => out.push_str(raw),
    }
}

const URI_ATTR: &str = "URI=\"";

/// 重写标签行中所有 `URI="…"` 属性(EXT-X-KEY 密钥、EXT-X-MAP 初始化段、
/// EXT-X-MEDIA 备用音轨等;hls.js 会请求这些地址,相对地址+防盗链必须转发)
fn push_uri_attrs(line: &str, base: Option<&url::Url>, prefix: &str, out: &mut String) {
    let mut rest = line;
    while let Some(i) = rest.find(URI_ATTR) {
        let value_start = i + URI_ATTR.len();
        let Some(len) = rest[value_start..].find('"') else {
            break;
        };
        let raw = &rest[value_start..value_start + len];
        out.push_str(&rest[..value_start]);
        if raw.is_empty() {
            // 空 URI 原样保留
        } else {
            push_proxied(raw, base, prefix, out);
        }
        out.push('"');
        rest = &rest[value_start + len + 1..];
    }
    out.push_str(rest);
}

/// 把 m3u8 文本中的子地址重写为本代理地址(相对地址先按 `base` 解析为绝对),
/// 单遍扫描;注释与空行原样保留
fn rewrite_playlist(text: &str, base: &str) -> String {
    let base = url::Url::parse(base).ok();
    let prefix = hls_prefix();
    // base64 膨胀约 4/3,外加每行前缀
    let mut out = String::with_capacity(text.len() * 2 + 512);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') && trimmed.contains(URI_ATTR) {
            push_uri_attrs(trimmed, base.as_ref(), &prefix, &mut out);
        } else if trimmed.is_empty() || trimmed.starts_with('#') {
            out.push_str(line);
        } else {
            push_proxied(trimmed, base.as_ref(), &prefix, &mut out);
        }
        out.push('\n');
    }
    out
}

// ---------------------------------------------------------------------------
// 上游
// ---------------------------------------------------------------------------

/// 上游请求失败原因
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UpstreamError {
    /// 上游返回 >= 400,原样透传
    Status(u16),
    Timeout,
    Network,
    /// HTTP 客户端初始化失败
    Client,
}

impl UpstreamError {
    fn from_reqwest(e: &reqwest::Error) -> Self {
        if e.is_timeout() {
            Self::Timeout
        } else {
            Self::Network
        }
    }

    /// 回给客户端的状态码
    fn status(self) -> u16 {
        match self {
            Self::Status(s) => s,
            Self::Timeout => 504,
            Self::Network => 502,
            Self::Client => 500,
        }
    }

    /// 限流/瞬时故障/超时值得重试
    #[cfg(test)]
    fn retryable(self) -> bool {
        matches!(self.status(), 429 | 502 | 503 | 504)
    }
}

/// 进程级共享阻塞客户端:复用连接池与 TLS 会话(原先每个请求新建
/// 客户端,每次都要重新握手并起一个内部运行时线程)。
/// 只能在阻塞上下文(普通线程 / spawn_blocking)里首次调用。
fn upstream_client() -> Option<&'static reqwest::blocking::Client> {
    static CLIENT: OnceLock<Option<reqwest::blocking::Client>> = OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::blocking::Client::builder()
                .user_agent(USER_AGENT)
                .connect_timeout(UPSTREAM_CONNECT_TIMEOUT)
                .timeout(UPSTREAM_IO_TIMEOUT)
                .build()
                .ok()
        })
        .as_ref()
}

/// 发起上游请求并检查状态;正文未读,由调用方流式消费
fn upstream_open(
    url: &str,
    range: Option<&str>,
) -> Result<reqwest::blocking::Response, UpstreamError> {
    let client = upstream_client().ok_or(UpstreamError::Client)?;
    let mut req = client.get(url).header(REFERER, referer_for(url));
    if let Some(r) = range {
        // 范围请求禁用压缩:Content-Range 针对编码后字节,解压后会对不上
        req = req.header(RANGE, r).header(ACCEPT_ENCODING, "identity");
    }
    let resp = req.send().map_err(|e| UpstreamError::from_reqwest(&e))?;
    let status = resp.status().as_u16();
    if status >= 400 {
        return Err(UpstreamError::Status(status));
    }
    Ok(resp)
}

fn header_str(headers: &HeaderMap, name: HeaderName) -> Option<&str> {
    headers.get(name).and_then(|v| v.to_str().ok())
}

/// 读取正文时的 IO 错误映射为状态码:超时 504,其余 502
fn io_status(e: &io::Error) -> u16 {
    let timed_out = e.kind() == io::ErrorKind::TimedOut
        || e.get_ref()
            .and_then(|inner| inner.downcast_ref::<reqwest::Error>())
            .is_some_and(reqwest::Error::is_timeout);
    if timed_out {
        504
    } else {
        502
    }
}

/// `fetch_hls` 失败原因(Display 文案保持原有格式)
#[cfg(test)]
enum FetchError {
    InvalidUrl,
    NotUtf8,
    Upstream(UpstreamError),
}

#[cfg(test)]
impl FetchError {
    fn retryable(&self) -> bool {
        matches!(self, Self::Upstream(e) if e.retryable())
    }
}

#[cfg(test)]
impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUrl => f.write_str("无效地址"),
            Self::NotUtf8 => f.write_str("m3u8 不是有效 UTF-8"),
            Self::Upstream(e) => write!(f, "CDN {}", e.status()),
        }
    }
}

/// 拉取 HLS 资源:m3u8 返回重写后的文本;分片返回原始字节。
/// 仅供 `sources::video` 的上游联调测试使用(前端 IPC 加载器已下线),
/// 故只在测试构建中编译。blocking 网络 IO 经 spawn_blocking,不阻塞异步执行器。
#[cfg(test)]
pub async fn fetch_hls(url: &str) -> Result<(bool, String, Vec<u8>), String> {
    // CDN 偶发 502(限流/瞬时故障):指数间隔重试 3 次
    let mut last_err: Option<String> = None;
    for wait in [0u64, 1500, 4000] {
        if wait > 0 {
            tokio::time::sleep(Duration::from_millis(wait)).await;
        }
        let owned = url.to_string();
        match tokio::task::spawn_blocking(move || fetch_hls_blocking(&owned))
            .await
            .map_err(|e| e.to_string())?
        {
            Ok(r) => return Ok(r),
            Err(e) => {
                let retryable = e.retryable();
                last_err = Some(e.to_string());
                if !retryable {
                    break;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| "hls 拉取失败".into()))
}

/// 阻塞版(供 spawn_blocking)
#[cfg(test)]
fn fetch_hls_blocking(url: &str) -> Result<(bool, String, Vec<u8>), FetchError> {
    let has_host = url::Url::parse(url)
        .ok()
        .is_some_and(|u| u.host_str().is_some_and(|h| !h.is_empty()));
    if !has_host {
        return Err(FetchError::InvalidUrl);
    }
    let resp = upstream_open(url, None).map_err(FetchError::Upstream)?;
    let ct = header_str(resp.headers(), CONTENT_TYPE)
        .unwrap_or("")
        .to_string();
    let bytes = resp
        .bytes()
        .map_err(|e| FetchError::Upstream(UpstreamError::from_reqwest(&e)))?
        .to_vec();
    if is_playlist(&ct, url, &bytes[..bytes.len().min(7)]) {
        let text = String::from_utf8(bytes).map_err(|_| FetchError::NotUtf8)?;
        return Ok((true, rewrite_playlist(&text, url), Vec::new()));
    }
    Ok((false, ct, bytes))
}

// ---------------------------------------------------------------------------
// HTTP 请求解析
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Method {
    Get,
    Head,
    Options,
    Other,
}

#[derive(Debug, PartialEq, Eq)]
struct Request {
    method: Method,
    /// 去掉 query 的路径
    path: String,
    range: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
enum ParseError {
    /// 连接已断开/读超时/截断:直接关闭,不回响应
    Closed,
    /// 回给客户端的错误状态码
    Status(u16),
}

enum LineError {
    Closed,
    TooLong,
}

/// 读一行(去掉行尾 CRLF),超过 `limit` 字节报 TooLong
fn read_line_limited<R: BufRead>(
    r: &mut R,
    buf: &mut Vec<u8>,
    limit: usize,
) -> Result<(), LineError> {
    buf.clear();
    let n = r
        .by_ref()
        .take(limit as u64 + 1)
        .read_until(b'\n', buf)
        .map_err(|_| LineError::Closed)?;
    if !buf.ends_with(b"\n") {
        return Err(if n > limit {
            LineError::TooLong
        } else {
            LineError::Closed
        });
    }
    while matches!(buf.last(), Some(b'\n' | b'\r')) {
        buf.pop();
    }
    Ok(())
}

/// 解析请求行与请求头(只关心方法、路径与 Range;不读请求体)
fn read_request<R: BufRead>(r: &mut R) -> Result<Request, ParseError> {
    let mut line = Vec::with_capacity(512);
    read_line_limited(r, &mut line, MAX_LINE).map_err(|e| match e {
        LineError::Closed => ParseError::Closed,
        LineError::TooLong => ParseError::Status(414),
    })?;
    let text = std::str::from_utf8(&line).map_err(|_| ParseError::Status(400))?;
    let mut parts = text.split_ascii_whitespace();
    let (Some(method), Some(target)) = (parts.next(), parts.next()) else {
        return Err(ParseError::Status(400));
    };
    let method = match method {
        "GET" => Method::Get,
        "HEAD" => Method::Head,
        "OPTIONS" => Method::Options,
        _ => Method::Other,
    };
    let path = target.split_once('?').map_or(target, |(p, _)| p).to_owned();

    let mut range = None;
    let mut total = 0usize;
    loop {
        read_line_limited(r, &mut line, MAX_LINE).map_err(|e| match e {
            LineError::Closed => ParseError::Closed,
            LineError::TooLong => ParseError::Status(431),
        })?;
        if line.is_empty() {
            break;
        }
        total += line.len();
        if total > MAX_HEADER_BYTES {
            return Err(ParseError::Status(431));
        }
        let Ok(header) = std::str::from_utf8(&line) else {
            continue;
        };
        if let Some((name, value)) = header.split_once(':') {
            if name.trim().eq_ignore_ascii_case("range") {
                range = Some(value.trim().to_owned());
            }
        }
    }
    Ok(Request {
        method,
        path,
        range,
    })
}

/// 单段 Range 相对已知长度的解析结果
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ByteRange {
    /// 无效/多段/非 bytes 单位:按规范忽略,返回完整内容
    Full,
    /// 闭区间 [start, end]
    Partial { start: usize, end: usize },
    /// 416
    Unsatisfiable,
}

fn resolve_range(header: &str, len: usize) -> ByteRange {
    let header = header.trim();
    let Some(spec) = header
        .get(..6)
        .filter(|unit| unit.eq_ignore_ascii_case("bytes="))
        .map(|_| &header[6..])
    else {
        return ByteRange::Full;
    };
    if spec.contains(',') {
        return ByteRange::Full;
    }
    let Some((a, b)) = spec.split_once('-') else {
        return ByteRange::Full;
    };
    let (a, b) = (a.trim(), b.trim());
    if a.is_empty() {
        // 后缀范围:最后 n 字节
        let Ok(n) = b.parse::<usize>() else {
            return ByteRange::Full;
        };
        if n == 0 || len == 0 {
            return ByteRange::Unsatisfiable;
        }
        return ByteRange::Partial {
            start: len.saturating_sub(n),
            end: len - 1,
        };
    }
    let Ok(start) = a.parse::<usize>() else {
        return ByteRange::Full;
    };
    let end = if b.is_empty() {
        usize::MAX
    } else {
        match b.parse::<usize>() {
            Ok(e) => e,
            Err(_) => return ByteRange::Full,
        }
    };
    if end < start {
        return ByteRange::Full;
    }
    if start >= len {
        return ByteRange::Unsatisfiable;
    }
    ByteRange::Partial {
        start,
        end: end.min(len - 1),
    }
}

// ---------------------------------------------------------------------------
// HTTP 响应
// ---------------------------------------------------------------------------

/// 响应正文定界方式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Framing {
    Length(u64),
    Chunked,
    /// 204 等无正文响应
    None,
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        206 => "Partial Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        414 => "URI Too Long",
        416 => "Range Not Satisfiable",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",
        s if s < 400 => "OK",
        _ => "Error",
    }
}

fn write_head<W: Write>(
    w: &mut W,
    status: u16,
    content_type: &str,
    framing: Framing,
    extra: &[(&str, &str)],
) -> io::Result<()> {
    let mut h = String::with_capacity(320);
    let _ = write!(h, "HTTP/1.1 {status} {}\r\n", reason(status));
    if !content_type.is_empty() {
        let _ = write!(h, "Content-Type: {content_type}\r\n");
    }
    match framing {
        Framing::Length(n) => {
            let _ = write!(h, "Content-Length: {n}\r\n");
        }
        Framing::Chunked => h.push_str("Transfer-Encoding: chunked\r\n"),
        Framing::None => {}
    }
    h.push_str(CORS_HEADERS);
    h.push_str("Connection: close\r\n");
    for (k, v) in extra {
        let _ = write!(h, "{k}: {v}\r\n");
    }
    h.push_str("\r\n");
    w.write_all(h.as_bytes())
}

/// 头 + 内存正文(HEAD 只发头)
fn send_bytes<W: Write>(
    w: &mut W,
    method: Method,
    status: u16,
    content_type: &str,
    body: &[u8],
    extra: &[(&str, &str)],
) -> io::Result<()> {
    write_head(
        w,
        status,
        content_type,
        Framing::Length(body.len() as u64),
        extra,
    )?;
    if method != Method::Head {
        w.write_all(body)?;
    }
    w.flush()
}

fn send_text<W: Write>(w: &mut W, method: Method, status: u16, msg: &str) -> io::Result<()> {
    send_bytes(w, method, status, "text/plain", msg.as_bytes(), &[])
}

/// 把 `src` 正文转发到 `dst`(`chunked` 时按 chunked 编码分帧)。
/// `keep` 为 Some 时同时留存一份正文供缓存,超过 `keep_max` 即放弃留存。
/// 返回 (转发字节数, 留存正文)。
fn pump_body<R: Read, W: Write>(
    src: &mut R,
    dst: &mut W,
    chunked: bool,
    mut keep: Option<Vec<u8>>,
    keep_max: usize,
) -> io::Result<(u64, Option<Vec<u8>>)> {
    let mut buf = vec![0u8; COPY_BUF];
    let mut total = 0u64;
    loop {
        let n = match src.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        let chunk = &buf[..n];
        if chunked {
            write!(dst, "{n:X}\r\n")?;
            dst.write_all(chunk)?;
            dst.write_all(b"\r\n")?;
        } else {
            dst.write_all(chunk)?;
        }
        total += n as u64;
        if let Some(k) = keep.as_mut() {
            if k.len() + n > keep_max {
                keep = None;
            } else {
                k.extend_from_slice(chunk);
            }
        }
    }
    if chunked {
        dst.write_all(b"0\r\n\r\n")?;
    }
    Ok((total, keep))
}

/// 读满 `buf` 或到 EOF,返回读到的字节数(用于嗅探正文魔数)
fn read_prefix<R: Read>(r: &mut R, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(filled)
}

// ---------------------------------------------------------------------------
// 处理器
// ---------------------------------------------------------------------------

/// 代理端点的差异化配置
#[derive(Clone, Copy)]
struct Route {
    /// 缓存键前缀(与 HLS 分片区分)
    key_prefix: &'static str,
    /// 上游缺 Content-Type 时的默认值
    default_type: &'static str,
    /// HLS 端点:识别并重写清单、透传 Range
    hls: bool,
}

const HLS_ROUTE: Route = Route {
    key_prefix: "",
    default_type: "application/octet-stream",
    hls: true,
};

const IMG_ROUTE: Route = Route {
    key_prefix: "img:",
    default_type: "image/jpeg",
    hls: false,
};

/// 尝试从缓存应答;未命中返回 Ok(false)
fn serve_cached<W: Write>(
    out: &mut W,
    method: Method,
    route: Route,
    real: &str,
    range: Option<&str>,
) -> io::Result<bool> {
    let accept: &[(&str, &str)] = if route.hls {
        &[("Accept-Ranges", "bytes")]
    } else {
        &[]
    };
    if let Some(r) = range {
        // 与之前完全相同的范围请求(byterange 清单回看)
        if let Some(e) = cache_get(&cache_key(route.key_prefix, real, Some(r))) {
            let cr = e.content_range.as_deref().unwrap_or("");
            send_bytes(
                out,
                method,
                206,
                &e.content_type,
                &e.body,
                &[("Accept-Ranges", "bytes"), ("Content-Range", cr)],
            )?;
            return Ok(true);
        }
    }
    let Some(e) = cache_get(&cache_key(route.key_prefix, real, None)) else {
        return Ok(false);
    };
    let len = e.body.len();
    match range.map_or(ByteRange::Full, |r| resolve_range(r, len)) {
        ByteRange::Full => send_bytes(out, method, 200, &e.content_type, &e.body, accept)?,
        ByteRange::Partial { start, end } => {
            let cr = format!("bytes {start}-{end}/{len}");
            send_bytes(
                out,
                method,
                206,
                &e.content_type,
                &e.body[start..=end],
                &[("Accept-Ranges", "bytes"), ("Content-Range", &cr)],
            )?;
        }
        ByteRange::Unsatisfiable => {
            let cr = format!("bytes */{len}");
            send_bytes(
                out,
                method,
                416,
                "text/plain",
                b"",
                &[("Accept-Ranges", "bytes"), ("Content-Range", &cr)],
            )?;
        }
    }
    Ok(true)
}

/// /hls/ 与 /img/ 的公共处理:缓存命中直接应答,否则流式转发上游
fn serve_proxied<W: Write>(out: &mut W, req: &Request, b64: &str, route: Route) -> io::Result<()> {
    let method = req.method;
    let Some(real) = decode_b64(b64) else {
        return send_text(out, method, 400, "bad url");
    };
    let playlist_url = route.hls && url_is_m3u8(&real);
    // 清单不缓存(内容小且需每次按当前端口重写),也不透传 Range;
    // 图片端点同样忽略 Range
    let range = if route.hls && !playlist_url {
        req.range.as_deref()
    } else {
        None
    };
    if !playlist_url && serve_cached(out, method, route, &real, range)? {
        return Ok(());
    }

    let mut resp = match upstream_open(&real, range) {
        Ok(r) => r,
        Err(e) => {
            let s = e.status();
            return send_text(out, method, s, &format!("upstream {s}"));
        }
    };
    let status = resp.status().as_u16();
    let headers = resp.headers();
    let ct = header_str(headers, CONTENT_TYPE).unwrap_or("").to_owned();
    let content_range = header_str(headers, CONTENT_RANGE).map(str::to_owned);
    // 自动解压的响应 reqwest 会移除 Content-Length,此时走 chunked
    let length = header_str(headers, CONTENT_LENGTH).and_then(|v| v.trim().parse::<u64>().ok());

    let mut prefix = [0u8; 7];
    let n = match read_prefix(&mut resp, &mut prefix) {
        Ok(n) => n,
        Err(e) => {
            let s = io_status(&e);
            return send_text(out, method, s, &format!("upstream {s}"));
        }
    };
    let mut body = (&prefix[..n]).chain(resp);

    if route.hls && is_playlist(&ct, &real, &prefix[..n]) {
        let mut raw = Vec::with_capacity(length.unwrap_or(0).min(PLAYLIST_MAX) as usize);
        if let Err(e) = body.by_ref().take(PLAYLIST_MAX + 1).read_to_end(&mut raw) {
            let s = io_status(&e);
            return send_text(out, method, s, &format!("upstream {s}"));
        }
        if raw.len() as u64 > PLAYLIST_MAX {
            return send_text(out, method, 502, "playlist too large");
        }
        return match String::from_utf8(raw) {
            Ok(text) => {
                let rewritten = rewrite_playlist(&text, &real);
                send_bytes(
                    out,
                    method,
                    200,
                    M3U8_TYPE,
                    rewritten.as_bytes(),
                    &[("Accept-Ranges", "bytes")],
                )
            }
            // 非文本:不是清单,按普通资源原样返回
            Err(e) => {
                let ct = if ct.is_empty() {
                    route.default_type
                } else {
                    &ct
                };
                send_bytes(out, method, status, ct, e.as_bytes(), &[])
            }
        };
    }

    let content_type = if ct.is_empty() {
        route.default_type
    } else {
        &ct
    };
    let mut extra: Vec<(&str, &str)> = Vec::with_capacity(2);
    if route.hls {
        extra.push(("Accept-Ranges", "bytes"));
    }
    if let Some(cr) = content_range.as_deref() {
        extra.push(("Content-Range", cr));
    }
    let framing = length.map_or(Framing::Chunked, Framing::Length);
    write_head(out, status, content_type, framing, &extra)?;
    if method == Method::Head {
        return out.flush();
    }

    // 只缓存完整资源(200)或带 Content-Range 的范围响应(206)
    let cache_as = match status {
        200 => Some(cache_key(route.key_prefix, &real, None)),
        206 if content_range.is_some() => {
            range.map(|r| cache_key(route.key_prefix, &real, Some(r)))
        }
        _ => None,
    };
    // 已知超过单条上限(整段 mp4 等)的直接不留存,免得白占 16MB 再丢弃
    let keep_hint = length.unwrap_or(0);
    let keep = (cache_as.is_some() && keep_hint <= CACHE_ENTRY_MAX as u64)
        .then(|| Vec::with_capacity(keep_hint as usize));
    let (total, kept) = pump_body(
        &mut body,
        out,
        framing == Framing::Chunked,
        keep,
        CACHE_ENTRY_MAX,
    )?;
    out.flush()?;

    // 上游提前结束(短于 Content-Length)的残段不入缓存
    if let (Some(key), Some(bytes)) = (cache_as, kept) {
        if length.is_none_or(|l| l == total) {
            cache_put(
                key,
                CacheEntry {
                    content_type: content_type.to_owned(),
                    content_range: if status == 206 { content_range } else { None },
                    body: bytes.into(),
                },
            );
        }
    }
    Ok(())
}

/// 请求分发
fn route<W: Write>(out: &mut W, parsed: Result<Request, ParseError>) -> io::Result<()> {
    let req = match parsed {
        Ok(r) => r,
        Err(ParseError::Closed) => return Ok(()),
        Err(ParseError::Status(s)) => return send_text(out, Method::Get, s, "bad request"),
    };
    match req.method {
        Method::Options => {
            // CORS 预检(带 Range 等非简单头时 WebView 会先发)
            write_head(
                out,
                204,
                "",
                Framing::None,
                &[
                    ("Access-Control-Allow-Methods", "GET, HEAD, OPTIONS"),
                    ("Access-Control-Allow-Headers", "*"),
                    ("Access-Control-Max-Age", "86400"),
                ],
            )?;
            return out.flush();
        }
        Method::Other => {
            return send_bytes(
                out,
                Method::Get,
                405,
                "text/plain",
                b"method not allowed",
                &[("Allow", "GET, HEAD, OPTIONS")],
            );
        }
        Method::Get | Method::Head => {}
    }
    if let Some(rest) = req.path.strip_prefix("/hls/") {
        serve_proxied(out, &req, rest, HLS_ROUTE)
    } else if let Some(rest) = req.path.strip_prefix("/img/") {
        serve_proxied(out, &req, rest, IMG_ROUTE)
    } else {
        send_text(out, req.method, 404, "not found")
    }
}

fn handle_client(stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(CLIENT_READ_TIMEOUT));
    let _ = stream.set_write_timeout(Some(CLIENT_WRITE_TIMEOUT));
    let parsed = read_request(&mut BufReader::new(&stream));
    let mut out = BufWriter::with_capacity(COPY_BUF + 64, &stream);
    // 写失败 = 客户端已断开(快进/切集时 hls.js 会中止旧请求),静默丢弃
    let _ = route(&mut out, parsed);
}

// ---------------------------------------------------------------------------
// 启动
// ---------------------------------------------------------------------------

/// 启动本地代理服务(仅 127.0.0.1),返回实际端口。
/// 独立监听线程;连接处理均为阻塞 IO,交给 tokio 阻塞线程池(线程复用,
/// 空闲自动回收),线程池不可用时退化为每连接一线程。
pub fn spawn() -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let _ = PORT.set(port);
    std::thread::Builder::new()
        .name("ygqd-proxy".into())
        .spawn(move || accept_loop(listener))
        .map_err(|e| e.to_string())?;
    Ok(port)
}

fn accept_loop(listener: TcpListener) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .thread_name("ygqd-proxy-conn")
        .build()
        .ok();
    for stream in listener.incoming() {
        let stream = match stream {
            Ok(s) => s,
            Err(_) => {
                // fd 耗尽等持续性错误:稍候再 accept,避免空转烧 CPU
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
        };
        match &rt {
            Some(rt) => {
                rt.spawn_blocking(move || handle_client(stream));
            }
            None => {
                let _ = std::thread::Builder::new()
                    .name("ygqd-proxy-conn".into())
                    .spawn(move || handle_client(stream));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 取出重写后代理地址中的真实地址
    fn unwrap_proxied(s: &str) -> String {
        let (_, b64) = s.split_once("/hls/").expect("proxied url");
        decode_b64(b64).expect("b64")
    }

    #[test]
    fn hls_url_format_is_stable() {
        let u = hls_url("https://a.com/x.m3u8");
        assert_eq!(
            u,
            format!(
                "http://127.0.0.1:{}/hls/{}",
                port(),
                B64.encode("https://a.com/x.m3u8")
            )
        );
        assert_eq!(unwrap_proxied(&u), "https://a.com/x.m3u8");
    }

    #[test]
    fn rewrite_segments_relative_and_absolute() {
        let text = "#EXTM3U\r\n#EXT-X-TARGETDURATION:4\r\n\r\n#EXTINF:4.0,\r\nseg0.ts\r\n#EXTINF:4.0,\r\nhttps://cdn.b.com/seg1.ts?t=1\r\n/root/seg2.ts\r\n//other.com/seg3.ts\r\n#EXT-X-ENDLIST";
        let out = rewrite_playlist(text, "https://a.com/v/1080p/index.m3u8?tok=9");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "#EXTM3U");
        assert_eq!(lines[1], "#EXT-X-TARGETDURATION:4");
        assert_eq!(lines[2], "");
        assert_eq!(lines[3], "#EXTINF:4.0,");
        assert_eq!(unwrap_proxied(lines[4]), "https://a.com/v/1080p/seg0.ts");
        assert_eq!(unwrap_proxied(lines[6]), "https://cdn.b.com/seg1.ts?t=1");
        assert_eq!(unwrap_proxied(lines[7]), "https://a.com/root/seg2.ts");
        assert_eq!(unwrap_proxied(lines[8]), "https://other.com/seg3.ts");
        assert_eq!(lines[9], "#EXT-X-ENDLIST");
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn rewrite_variant_playlist() {
        let text =
            "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1,RESOLUTION=1280x720\n1280x720/video.m3u8\n";
        let out = rewrite_playlist(text, "https://a.com/hls/playlist.m3u8");
        let line = out.lines().nth(2).unwrap();
        assert_eq!(
            unwrap_proxied(line),
            "https://a.com/hls/1280x720/video.m3u8"
        );
        assert!(!out.contains("\n1280x"));
    }

    #[test]
    fn rewrite_uri_attributes() {
        let text = concat!(
            "#EXTM3U\n",
            "#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\",IV=0x01\n",
            "#EXT-X-MAP:URI=\"https://c.com/init.mp4\",BYTERANGE=\"10@0\"\n",
            "#EXT-X-KEY:METHOD=AES-128,URI=\"data:text/plain;base64,AAAA\"\n",
            "#EXT-X-KEY:METHOD=AES-128,URI=\"\"\n",
            "#EXT-X-KEY:METHOD=AES-128,URI=\"unterminated\n",
        );
        let out = rewrite_playlist(text, "https://a.com/v/index.m3u8");
        let lines: Vec<&str> = out.lines().collect();
        let key = lines[1];
        assert!(key.starts_with("#EXT-X-KEY:METHOD=AES-128,URI=\"http://127.0.0.1:"));
        assert!(key.ends_with("\",IV=0x01"));
        let uri = key.split('"').nth(1).unwrap();
        assert_eq!(unwrap_proxied(uri), "https://a.com/v/key.bin");
        let map_uri = lines[2].split('"').nth(1).unwrap();
        assert_eq!(unwrap_proxied(map_uri), "https://c.com/init.mp4");
        assert!(lines[2].ends_with(",BYTERANGE=\"10@0\""));
        // data: 内联密钥保持原样
        assert_eq!(
            lines[3],
            "#EXT-X-KEY:METHOD=AES-128,URI=\"data:text/plain;base64,AAAA\""
        );
        assert_eq!(lines[4], "#EXT-X-KEY:METHOD=AES-128,URI=\"\"");
        assert_eq!(lines[5], "#EXT-X-KEY:METHOD=AES-128,URI=\"unterminated");
    }

    #[test]
    fn playlist_detection() {
        assert!(is_playlist(
            "application/vnd.apple.mpegURL",
            "https://a/x",
            b""
        ));
        assert!(is_playlist("audio/x-mpegurl", "https://a/x", b""));
        assert!(is_playlist("", "https://a/x.m3u8?t=1", b""));
        assert!(is_playlist("text/plain", "https://a/x", b"#EXTM3U"));
        assert!(!is_playlist(
            "video/mp2t",
            "https://a/x.ts",
            b"\x47\x40\x00\x10\x00\x00\xb0"
        ));
        assert!(!url_is_m3u8("https://a/x.m3u8.ts"));
    }

    #[test]
    fn range_parsing() {
        use ByteRange::*;
        assert_eq!(
            resolve_range("bytes=0-99", 1000),
            Partial { start: 0, end: 99 }
        );
        assert_eq!(
            resolve_range("bytes=500-", 1000),
            Partial {
                start: 500,
                end: 999
            }
        );
        assert_eq!(
            resolve_range("bytes=900-5000", 1000),
            Partial {
                start: 900,
                end: 999
            }
        );
        assert_eq!(
            resolve_range("bytes=-100", 1000),
            Partial {
                start: 900,
                end: 999
            }
        );
        assert_eq!(
            resolve_range("bytes=-5000", 1000),
            Partial { start: 0, end: 999 }
        );
        assert_eq!(
            resolve_range("BYTES= 1-2", 10),
            Partial { start: 1, end: 2 }
        );
        assert_eq!(resolve_range("bytes=1000-", 1000), Unsatisfiable);
        assert_eq!(resolve_range("bytes=-0", 1000), Unsatisfiable);
        assert_eq!(resolve_range("bytes=0-", 0), Unsatisfiable);
        assert_eq!(resolve_range("bytes=5-1", 1000), Full);
        assert_eq!(resolve_range("bytes=0-1,5-6", 1000), Full);
        assert_eq!(resolve_range("items=0-1", 1000), Full);
        assert_eq!(resolve_range("bytes=abc", 1000), Full);
        assert_eq!(resolve_range("bytes=99999999999999999999999-", 1000), Full);
        assert_eq!(resolve_range("", 1000), Full);
        assert_eq!(resolve_range("by", 1000), Full);
    }

    #[test]
    fn request_parsing() {
        let raw = b"GET /hls/abc?x=1 HTTP/1.1\r\nHost: 127.0.0.1\r\nrange:  bytes=0-1 \r\n\r\nBODY";
        let req = read_request(&mut &raw[..]).unwrap();
        assert_eq!(
            req,
            Request {
                method: Method::Get,
                path: "/hls/abc".into(),
                range: Some("bytes=0-1".into()),
            }
        );
        let req = read_request(&mut &b"HEAD /img/x HTTP/1.1\n\n"[..]).unwrap();
        assert_eq!(req.method, Method::Head);
        assert_eq!(req.range, None);
        let req = read_request(&mut &b"POST /x HTTP/1.1\r\n\r\n"[..]).unwrap();
        assert_eq!(req.method, Method::Other);
    }

    #[test]
    fn request_parsing_rejects_malformed() {
        assert_eq!(read_request(&mut &b""[..]), Err(ParseError::Closed));
        assert_eq!(
            read_request(&mut &b"GET /x HTTP/1.1\r\nHost"[..]),
            Err(ParseError::Closed)
        );
        assert_eq!(
            read_request(&mut &b"GARBAGE\r\n\r\n"[..]),
            Err(ParseError::Status(400))
        );
        assert_eq!(
            read_request(&mut &b"\xff\xfe /x\r\n\r\n"[..]),
            Err(ParseError::Status(400))
        );
        let long_line = format!("GET /{} HTTP/1.1\r\n\r\n", "a".repeat(MAX_LINE + 10));
        assert_eq!(
            read_request(&mut long_line.as_bytes()),
            Err(ParseError::Status(414))
        );
        let many = format!(
            "GET / HTTP/1.1\r\n{}\r\n",
            "X-A: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\r\n".repeat(2000)
        );
        assert_eq!(
            read_request(&mut many.as_bytes()),
            Err(ParseError::Status(431))
        );
    }

    #[test]
    fn cache_evicts_oldest_half() {
        let entry = |n: usize| CacheEntry {
            content_type: "video/mp2t".into(),
            content_range: None,
            body: vec![0u8; n].into(),
        };
        let mut c = Cache::new(100);
        for i in 0..4 {
            c.put(format!("k{i}"), entry(25));
        }
        assert_eq!(c.size, 100);
        // 重复键不重复计数
        c.put("k3".into(), entry(25));
        assert_eq!(c.size, 100);
        c.put("k4".into(), entry(10));
        assert!(c.get("k0").is_none() && c.get("k1").is_none());
        assert!(c.get("k2").is_some() && c.get("k4").is_some());
        assert_eq!(c.size, 60);
        // 超过总上限的条目不入缓存
        c.put("big".into(), entry(101));
        assert!(c.get("big").is_none());
        // 单条接近上限时持续淘汰直至放得下
        c.put("near".into(), entry(95));
        assert!(c.get("near").is_some());
        assert_eq!(c.size, 95);
        assert_eq!(c.order.len(), 1);
    }

    #[test]
    fn pump_plain_and_chunked() {
        let data: Vec<u8> = (0..200u8).collect();
        let mut out = Vec::new();
        let (n, kept) = pump_body(&mut &data[..], &mut out, false, Some(Vec::new()), 1000).unwrap();
        assert_eq!(n, 200);
        assert_eq!(out, data);
        assert_eq!(kept.unwrap(), data);

        let mut out = Vec::new();
        let (n, kept) = pump_body(&mut &b"hello"[..], &mut out, true, Some(Vec::new()), 3).unwrap();
        assert_eq!(n, 5);
        assert_eq!(out, b"5\r\nhello\r\n0\r\n\r\n");
        // 超过留存上限即放弃缓存
        assert!(kept.is_none());
    }

    #[test]
    fn pump_reports_client_disconnect() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let r = pump_body(&mut &b"data"[..], &mut Broken, false, None, 0);
        assert!(r.is_err());
    }

    fn get(path: &str, range: Option<&str>) -> String {
        let req = Request {
            method: Method::Get,
            path: path.into(),
            range: range.map(str::to_owned),
        };
        let mut out = Vec::new();
        route(&mut out, Ok(req)).unwrap();
        String::from_utf8_lossy(&out).into_owned()
    }

    #[test]
    fn routing_errors() {
        assert!(get("/nope", None).starts_with("HTTP/1.1 404 Not Found\r\n"));
        let r = get("/hls/!!!", None);
        assert!(r.starts_with("HTTP/1.1 400 Bad Request\r\n"));
        assert!(r.contains("Access-Control-Allow-Origin: *\r\n"));
        assert!(r.ends_with("\r\n\r\nbad url"));

        let mut out = Vec::new();
        let req = Request {
            method: Method::Other,
            path: "/hls/x".into(),
            range: None,
        };
        route(&mut out, Ok(req)).unwrap();
        assert!(out.starts_with(b"HTTP/1.1 405 Method Not Allowed\r\n"));

        let mut out = Vec::new();
        let req = Request {
            method: Method::Options,
            path: "/hls/x".into(),
            range: None,
        };
        route(&mut out, Ok(req)).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("HTTP/1.1 204 No Content\r\n"));
        assert!(!s.contains("\r\nContent-Length:"));
        assert!(s.contains("Access-Control-Allow-Headers: *\r\n"));

        let mut out = Vec::new();
        route(&mut out, Err(ParseError::Status(431))).unwrap();
        assert!(out.starts_with(b"HTTP/1.1 431 "));
        let mut out = Vec::new();
        route(&mut out, Err(ParseError::Closed)).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn cached_segment_serves_ranges() {
        let real = "https://unit.test/cached/seg0.ts";
        let body: Vec<u8> = (0..100u8).collect();
        cache_put(
            cache_key("", real, None),
            CacheEntry {
                content_type: "video/mp2t".into(),
                content_range: None,
                body: body.clone().into(),
            },
        );
        let path = format!("/hls/{}", B64.encode(real));

        let r = get(&path, None);
        assert!(r.starts_with("HTTP/1.1 200 OK\r\n"));
        assert!(r.contains("Content-Length: 100\r\n"));
        assert!(r.contains("Accept-Ranges: bytes\r\n"));

        let r = get(&path, Some("bytes=10-19"));
        assert!(r.starts_with("HTTP/1.1 206 Partial Content\r\n"));
        assert!(r.contains("Content-Length: 10\r\n"));
        assert!(r.contains("Content-Range: bytes 10-19/100\r\n"));
        let (_, payload) = r.split_once("\r\n\r\n").unwrap();
        assert_eq!(payload.as_bytes(), &body[10..20]);

        let r = get(&path, Some("bytes=100-"));
        assert!(r.starts_with("HTTP/1.1 416 "));
        assert!(r.contains("Content-Range: bytes */100\r\n"));

        // HEAD 只发头
        let mut out = Vec::new();
        let req = Request {
            method: Method::Head,
            path: path.clone(),
            range: None,
        };
        route(&mut out, Ok(req)).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.contains("Content-Length: 100\r\n") && s.ends_with("\r\n\r\n"));
    }

    #[test]
    fn cached_range_entry_exact_hit() {
        let real = "https://unit.test/cached/big.mp4";
        cache_put(
            cache_key("", real, Some("bytes=0-3")),
            CacheEntry {
                content_type: "video/mp4".into(),
                content_range: Some("bytes 0-3/9999".into()),
                body: b"abcd".to_vec().into(),
            },
        );
        let path = format!("/hls/{}", B64.encode(real));
        let r = get(&path, Some("bytes=0-3"));
        assert!(r.starts_with("HTTP/1.1 206 "));
        assert!(r.contains("Content-Range: bytes 0-3/9999\r\n"));
        assert!(r.ends_with("abcd"));
    }

    #[test]
    fn upstream_error_mapping() {
        assert_eq!(UpstreamError::Status(403).status(), 403);
        assert_eq!(UpstreamError::Timeout.status(), 504);
        assert_eq!(UpstreamError::Network.status(), 502);
        assert!(UpstreamError::Status(429).retryable());
        assert!(UpstreamError::Network.retryable());
        assert!(!UpstreamError::Status(404).retryable());
        assert_eq!(
            FetchError::Upstream(UpstreamError::Network).to_string(),
            "CDN 502"
        );
        assert_eq!(io_status(&io::Error::from(io::ErrorKind::TimedOut)), 504);
        assert_eq!(
            io_status(&io::Error::from(io::ErrorKind::ConnectionReset)),
            502
        );
    }
}
