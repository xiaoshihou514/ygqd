//! 跨源共享工具:percent 解码、UA 常量、HTTP 状态文案、日志摘要截断、
//! 短列表保序去重。(RateGate 已删除:限速闸由 lib.rs 的 niacg_search_gate 承担)

// ── 静态正则/选择器:一律 `std::sync::LazyLock` 静态量,首次使用编译、
// 之后复用(解析热路径每条目都会命中,禁止在函数体内 `Regex::new`) ──

// ── percent 解码(video.rs 手写版迁移,行为等价) ──

/// 简易 percent 解码(%XX 十六进制对;非法序列原样保留)
pub fn percent_decode_simple(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ── 错误文案(IPC 边界:前端直接展示 Err 字符串,须简短中文;
// 调试细节只写 stderr) ──

/// HTTP 状态码 → 简短中文提示,`site` 为站点显示名(如 "哔咔")
pub fn http_status_error(site: &str, status: reqwest::StatusCode) -> String {
    let code = status.as_u16();
    match code {
        403 => format!("{site}拒绝了访问（403），请稍后重试"),
        404 => format!("{site}内容不存在或已被删除（404）"),
        429 => format!("{site}请求过于频繁，请稍后再试"),
        500..=599 => format!("{site}服务器暂时不可用（{code}），请稍后重试"),
        _ => format!("{site}请求失败（{code}）"),
    }
}

/// reqwest 错误 → 简短中文提示;完整错误链(含 URL)写 stderr 供调试
pub fn net_error(site: &str, err: &reqwest::Error) -> String {
    eprintln!("[{site}] request error: {err:?}");
    if err.is_timeout() {
        format!("{site}请求超时，请检查网络后重试")
    } else if err.is_connect() {
        format!("无法连接{site}服务器，请检查网络")
    } else if let Some(status) = err.status() {
        http_status_error(site, status)
    } else if err.is_body() || err.is_decode() {
        format!("{site}响应读取中断，请重试")
    } else {
        format!("{site}网络请求失败，请稍后重试")
    }
}

/// 按字节上限截断且不切断 UTF-8 字符(直接 `&s[..n]` 遇中文会 panic),
/// 用于日志里的响应体摘要
pub fn snippet(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }
    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

// ── 列表工具 ──

/// 保序去重(标签/作者等短列表:线性查找,免去 HashSet 的逐项克隆)
pub fn dedup_in_order(items: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for item in items {
        if !out.contains(&item) {
            out.push(item);
        }
    }
    out
}

// ── UA/指纹常量集中 ──

/// fs1 直连 UA(Jable 系,通用移动 Chrome)
pub const FS1_UA: &str =
    "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_decode_basic_and_invalid() {
        assert_eq!(percent_decode_simple("%E7%80%AC%E6%88%B8"), "瀬戸");
        assert_eq!(percent_decode_simple("plain"), "plain");
        assert_eq!(percent_decode_simple("%zz"), "%zz");
        assert_eq!(percent_decode_simple("%4"), "%4");
    }

    #[test]
    fn snippet_respects_char_boundary() {
        assert_eq!(snippet("abc", 10), "abc");
        assert_eq!(snippet("abcdef", 3), "abc");
        // "哔" 占 3 字节,上限 4 时只能保留 1 个完整字符
        assert_eq!(snippet("哔咔", 4), "哔");
        assert_eq!(snippet("哔咔", 2), "");
    }

    #[test]
    fn dedup_keeps_first_occurrence_order() {
        let input = ["b", "a", "b", "c", "a"].map(String::from);
        assert_eq!(dedup_in_order(input), vec!["b", "a", "c"]);
    }

    #[test]
    fn status_errors_are_concise_chinese() {
        use reqwest::StatusCode;
        assert_eq!(
            http_status_error("哔咔", StatusCode::BAD_GATEWAY),
            "哔咔服务器暂时不可用（502），请稍后重试"
        );
        assert_eq!(
            http_status_error("niacg", StatusCode::IM_A_TEAPOT),
            "niacg请求失败（418）"
        );
    }
}
