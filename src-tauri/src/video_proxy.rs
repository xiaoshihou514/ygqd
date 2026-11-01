//! 视频流拉取:HLS 的 m3u8/分片由 Rust 端 reqwest 转发(带站点 Referer),
//! 经 hls.js 自定义 loader 通过 Tauri invoke 通道拉取。
//!
//! 为什么必须 Rust 转发:
//! - surrit.com(missav)校验 Referer=missav.ai,播放器直连 403
//! - mushroomtrack.com(fs1)拒绝浏览器 TLS 栈,直连 403
//! 为什么走 invoke 而不是自定义 scheme fetch:webkit 的 fetch 对
//! 自定义 scheme 的支持随版本漂移(实测不可靠),invoke 是原生
//! IPC 通道,必然可用。
//!
//! m3u8 返回前会把子播放列表/分片地址重写为绝对真实地址,hls.js
//! 拿到后继续用同一命令请求,链路闭环。

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

/// 把 m3u8 文本中的子播放列表/分片地址解析为绝对真实地址。
/// 非注释行即 URI 行;URI 行也可能是绝对地址。
fn resolve_playlist(text: &str, base: &str) -> String {
    let mut out = String::with_capacity(text.len() + 256);
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            out.push_str(line);
        } else if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
            out.push_str(trimmed);
        } else if let Ok(b) = url::Url::parse(base) {
            out.push_str(
                &b.join(trimmed)
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| trimmed.to_string()),
            );
        } else {
            out.push_str(trimmed);
        }
        out.push('\n');
    }
    out
}

/// 拉取 HLS 资源:m3u8 返回重写后的文本;分片返回原始字节
pub async fn fetch_hls(url: &str) -> Result<(bool, String, Vec<u8>), String> {
    let host = url::Url::parse(url)
        .ok()
        .and_then(|u| u.host_str().map(str::to_string))
        .unwrap_or_default();
    if host.is_empty() {
        return Err("无效地址".into());
    }
    let referer = referer_for(&host);
    let client = reqwest::Client::builder()
        .user_agent(
            "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36",
        )
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get(url)
        .header("Referer", referer)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("CDN 返回 {}", status.as_u16()));
    }
    let content_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?.to_vec();
    let head = &bytes[..bytes.len().min(7)];
    let is_playlist = content_type.contains("mpegurl")
        || content_type.contains("m3u8")
        || url.ends_with(".m3u8")
        || (content_type.is_empty() && head == b"#EXTM3U");
    if is_playlist {
        let text = String::from_utf8(bytes).map_err(|_| "m3u8 不是有效 UTF-8".to_string())?;
        return Ok((true, resolve_playlist(&text, url), Vec::new()));
    }
    Ok((false, content_type, bytes))
}

/// hls.js 自定义 loader 的后端:真实地址 -> (是播放列表, 文本或二进制)。
/// 播放列表返回重写后文本;分片返回原始字节。
#[tauri::command]
pub async fn hls_fetch(app: tauri::AppHandle, url: String) -> Result<HlsPayload, String> {
    let result = fetch_hls(&url).await;
    match &result {
        Ok((is_playlist, _, _)) => {
            crate::gateway::diag(
                &app,
                &format!("hls_fetch ok url={url} playlist={is_playlist}"),
            );
        }
        Err(e) => {
            crate::gateway::diag(&app, &format!("hls_fetch FAIL url={url} err={e}"));
        }
    }
    let (is_playlist, text, bytes) = result?;
    if is_playlist {
        Ok(HlsPayload {
            playlist: Some(text),
            bytes: Vec::new(),
            content_type: "application/vnd.apple.mpegurl".into(),
        })
    } else {
        Ok(HlsPayload {
            playlist: None,
            bytes,
            content_type: text,
        })
    }
}

/// hls.js loader 载荷:playlist 为 Some 时是重写后的 m3u8 文本,
/// 否则 bytes 为分片二进制
#[derive(serde::Serialize)]
pub struct HlsPayload {
    pub playlist: Option<String>,
    pub bytes: Vec<u8>,
    pub content_type: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_playlist_uris() {
        let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\n1280x720/video.m3u8\n";
        let out = resolve_playlist(master, "https://surrit.com/abc/playlist.m3u8");
        assert!(
            out.contains("https://surrit.com/abc/1280x720/video.m3u8"),
            "相对地址应解析为绝对: {out}"
        );
    }

    #[test]
    fn keeps_absolute_uris() {
        let out = resolve_playlist(
            "https://other.example/seg.ts\n",
            "https://surrit.com/abc/playlist.m3u8",
        );
        assert!(out.contains("https://other.example/seg.ts"));
    }
}
