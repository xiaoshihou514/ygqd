//! 视频流代理:HLS 的 m3u8/分片经自定义 stream:// 协议由 Rust 端
//! reqwest 转发(带正确 Referer),前端播放器只接触 stream:// 地址。
//!
//! 为什么必须代理:
//! - surrit.com(missav)校验 Referer=missav.ai,应用页面直连 403
//! - mushroomtrack.com(fs1)拒绝浏览器 TLS 栈,hls.js/webview 直连 403
//! 而 reqwest(rustls) + 站点 Referer 实测两者皆可拉取。
//!
//! 协议形态:stream://hls/{base64url(真实URL)}
//! 返回 m3u8 文本时把其中子播放列表/分片地址整体重写为代理地址,
//! 播放器无需理解真实网络。CDN 域 -> Referer 映射内置。

use base64::{engine::general_purpose::URL_SAFE_NO_PAD as B64, Engine as _};
use std::{sync::OnceLock, time::Duration};
use tauri::http::StatusCode;
type HttpResponse = tauri::http::Response<std::borrow::Cow<'static, [u8]>>;

fn encode(url: &str) -> String {
    B64.encode(url.as_bytes())
}

fn decode(segment: &str) -> Option<String> {
    B64.decode(segment)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
}

/// CDN 域 -> 站点 Referer(按后缀匹配)
fn referer_for(host: &str) -> &'static str {
    if host.ends_with("surrit.com") {
        "https://missav.ai/"
    } else if host.ends_with("mushroomtrack.com") {
        "https://fs1.app/"
    } else {
        "https://fs1.app/"
    }
}

fn proxy_url(real: &str) -> String {
    format!("stream://hls/{}", encode(real))
}

/// 重写 HLS 标签属性中的 URI(密钥、初始化分片等)。
fn rewrite_uri_attributes(line: &str, base: &str) -> String {
    let mut out = String::with_capacity(line.len() + 96);
    let mut cursor = 0;
    while let Some(relative) = line[cursor..].find("URI=") {
        let marker = cursor + relative;
        let value_start = marker + 4;
        let Some(&quote) = line.as_bytes().get(value_start) else {
            break;
        };
        if quote != b'"' && quote != b'\'' {
            cursor = value_start;
            continue;
        }
        let content_start = value_start + 1;
        let Some(end_relative) = line[content_start..].find(quote as char) else {
            break;
        };
        let content_end = content_start + end_relative;
        let uri = &line[content_start..content_end];
        out.push_str(&line[cursor..content_start]);
        if uri.starts_with("stream://") || uri.starts_with("data:") {
            out.push_str(uri);
        } else {
            out.push_str(&proxy_url(&resolve(uri, base)));
        }
        out.push(quote as char);
        cursor = content_end + 1;
    }
    out.push_str(&line[cursor..]);
    out
}

/// 把 m3u8 中的子播放清单、分片和 URI 属性统一改写为代理地址。
fn rewrite_playlist(text: &str, base: &str) -> String {
    let mut out = String::with_capacity(text.len() + 256);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push_str(line);
        } else if trimmed.starts_with('#') {
            out.push_str(&rewrite_uri_attributes(line, base));
        } else if trimmed.starts_with("stream://") {
            out.push_str(trimmed);
        } else {
            // 相对/绝对地址统一解析为绝对
            let absolute = resolve(trimmed, base);
            out.push_str(&proxy_url(&absolute));
        }
        out.push('\n');
    }
    out
}

fn resolve(uri: &str, base: &str) -> String {
    if uri.starts_with("http://") || uri.starts_with("https://") {
        uri.to_string()
    } else if let Ok(b) = url::Url::parse(base) {
        b.join(uri)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| uri.to_string())
    } else {
        uri.to_string()
    }
}

static MEDIA_CLIENT: OnceLock<Result<reqwest::Client, String>> = OnceLock::new();

fn media_client() -> Result<&'static reqwest::Client, String> {
    MEDIA_CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(
                    "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36",
                )
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build()
                .map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)
}

async fn fetch_media(url: &str) -> Result<(String, String, Vec<u8>), String> {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    let referer = referer_for(&host);
    let resp = media_client()?
        .get(url)
        .header("Referer", referer)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let final_url = resp.url().to_string();
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!("CDN 返回 {}", status.as_u16()));
    }
    Ok((content_type, final_url, bytes.to_vec()))
}

pub(crate) async fn process(real: String) -> Result<(String, Vec<u8>), String> {
    match fetch_media(&real).await {
        Ok((ct, final_url, bytes)) => {
            let head = &bytes[..bytes.len().min(7)];
            let is_playlist = ct.to_ascii_lowercase().contains("mpegurl")
                || ct.to_ascii_lowercase().contains("m3u8")
                || url::Url::parse(&final_url)
                    .ok()
                    .is_some_and(|parsed| parsed.path().ends_with(".m3u8"))
                || head == b"#EXTM3U";
            if is_playlist {
                if let Ok(text) = String::from_utf8(bytes.clone()) {
                    return Ok((
                        "application/vnd.apple.mpegurl".to_string(),
                        rewrite_playlist(&text, &final_url).into_bytes(),
                    ));
                }
            }
            Ok((
                if ct.is_empty() {
                    "application/octet-stream".to_string()
                } else {
                    ct
                },
                bytes,
            ))
        }
        Err(e) => Err(e),
    }
}

/// stream:// 协议处理器(在 tauri::Builder 上注册)
pub fn register(
    _ctx: tauri::UriSchemeContext<'_, tauri::Wry>,
    request: tauri::http::Request<Vec<u8>>,
) -> HttpResponse {
    let uri = request.uri().to_string();
    // 形如 stream://hls/{b64}
    let payload = uri
        .strip_prefix("stream://hls/")
        .or_else(|| uri.strip_prefix("stream:///hls/"))
        .unwrap_or("");
    let Some(real) = decode(payload) else {
        return tauri::http::response::Builder::new()
            .status(StatusCode::BAD_REQUEST)
            .body(std::borrow::Cow::Borrowed(b"invalid stream url".as_slice()))
            .unwrap();
    };
    // 协议 handler 在同步上下文:借用当前 runtime,没有则临时建一个
    let result = match tokio::runtime::Handle::try_current() {
        Ok(h) => tokio::task::block_in_place(|| h.block_on(process(real))),
        Err(_) => tokio::runtime::Runtime::new()
            .expect("rt")
            .block_on(process(real)),
    };
    match result {
        Ok((ct, body)) => tauri::http::response::Builder::new()
            .header("Content-Type", ct)
            .header("Access-Control-Allow-Origin", "*")
            .body(std::borrow::Cow::Owned(body))
            .unwrap(),
        Err(e) => tauri::http::response::Builder::new()
            .status(StatusCode::BAD_GATEWAY)
            .header("Content-Type", "text/plain; charset=utf-8")
            .body(std::borrow::Cow::Owned(e.into_bytes()))
            .unwrap(),
    }
}

/// 供 command 层暴露给前端:把真实 m3u8 包装成代理地址
pub fn wrap(m3u8: &str) -> String {
    proxy_url(m3u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rewrites_playlist_uris_to_stream_proxy() {
        let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\n1280x720/video.m3u8\n";
        let out = rewrite_playlist(master, "https://surrit.com/abc/playlist.m3u8");
        assert!(out.contains("stream://hls/"), "应包含代理地址: {out}");
        assert!(
            !out.contains("1280x720/video.m3u8\n"),
            "原始相对地址应被替换"
        );
        // 解码验证指向正确绝对地址
        let line = out.lines().find(|l| l.starts_with("stream://")).unwrap();
        let b64 = line.trim_start_matches("stream://hls/");
        let real = decode(b64).expect("可解码");
        assert_eq!(real, "https://surrit.com/abc/1280x720/video.m3u8");
    }

    #[test]
    fn rewrites_key_map_and_segment_uris() {
        let playlist = concat!(
            "#EXTM3U\n",
            "#EXT-X-KEY:METHOD=AES-128,URI=\"keys/key.bin\"\n",
            "#EXT-X-MAP:URI='init.mp4'\n",
            "segment-001.ts\n",
        );
        let out = rewrite_playlist(playlist, "https://cdn.example/path/master.m3u8");
        assert!(out.contains(&format!(
            "URI=\"{}\"",
            proxy_url("https://cdn.example/path/keys/key.bin")
        )));
        assert!(out.contains(&format!(
            "URI='{}'",
            proxy_url("https://cdn.example/path/init.mp4")
        )));
        assert!(out.contains(&proxy_url("https://cdn.example/path/segment-001.ts")));
    }

    #[test]
    fn roundtrip_wrap_and_decode() {
        let real = "https://surrit.com/x/playlist.m3u8";
        let wrapped = wrap(real);
        assert!(wrapped.starts_with("stream://hls/"));
        assert_eq!(
            decode(wrapped.trim_start_matches("stream://hls/")).unwrap(),
            real
        );
    }
}
