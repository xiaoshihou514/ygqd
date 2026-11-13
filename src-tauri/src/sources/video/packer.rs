//! Dean Edwards packer 解码(missav 详情页的播放地址藏在 packer 混淆脚本内)。

use regex::{Captures, Regex};
use std::collections::HashMap;
use std::sync::LazyLock;

/// `}('载荷',进制,词数,'k1|k2|…'.split('|'),0,{}))` 参数尾
static PACKED_ARGS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?s)\}\('((?:[^'\\]|\\.)*)',(\d+),(\d+),'((?:[^'\\]|\\.)*)'\.split\('\|'\),0,\{\}\)\)$"#,
    )
    .expect("static regex")
});
/// 载荷中的词元(与 packer 运行时的 `\b\w+\b` 一致)
static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\w+").expect("static regex"));
/// 页面中的 packer 块
static PACKED_BLOCK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"eval\(function\(p,a,c,k,e,d\)\{[\s\S]*?\}\)\)").expect("static regex")
});
static M3U8_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"https?://[^"'\s]+\.m3u8[^"'\s]*"#).expect("static regex"));

/// JS Number.toString(radix) 同款进制表示(0-9a-zA-Z,radix ≤ 62)
pub(super) fn to_base(mut n: u32, radix: u32) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    if n == 0 {
        return "0".into();
    }
    let mut buf = Vec::new();
    while n > 0 {
        buf.push(DIGITS[(n % radix) as usize]);
        n /= radix;
    }
    buf.reverse();
    String::from_utf8(buf).expect("ascii digits")
}

/// 解码 Dean Edwards packer:
/// `eval(function(p,a,c,k,e,d){…}('载荷',进制,词数,'k1|k2|…'.split('|'),0,{}))`
/// 与浏览器 `eval` 求值等价:载荷中每个词元按序号(base 进制表示)单趟
/// 查词表回填,词表项为空或无对应序号的词元原样保留
pub(super) fn unpack_packed(block: &str) -> Option<String> {
    let caps = PACKED_ARGS.captures(block.trim())?;
    let payload = js_unescape(caps.get(1)?.as_str())?;
    let radix: u32 = caps.get(2)?.as_str().parse().ok()?;
    let count: usize = caps.get(3)?.as_str().parse().ok()?;
    if !(2..=62).contains(&radix) {
        return None;
    }
    let words = js_unescape(caps.get(4)?.as_str())?;
    let dict: HashMap<String, &str> = words
        .split('|')
        .take(count)
        .enumerate()
        .filter(|(_, word)| !word.is_empty())
        .map(|(i, word)| (to_base(i as u32, radix), word))
        .collect();
    let decoded = WORD.replace_all(&payload, |c: &Captures| {
        let token = &c[0];
        dict.get(token).copied().unwrap_or(token).to_string()
    });
    Some(decoded.into_owned())
}

/// 从 HTML 中解码首个含 m3u8 的 packer 块(与网关 JS 一致取首个地址)
pub(super) fn extract_packed_m3u8(html: &str) -> Option<String> {
    PACKED_BLOCK.find_iter(html).find_map(|block| {
        let decoded = unpack_packed(block.as_str())?;
        M3U8_URL.find(&decoded).map(|m| m.as_str().to_string())
    })
}

/// JS 字符串字面量反转义(处理 packer 载荷中的 \\' \\\\ 等转义)
fn js_unescape(s: &str) -> Option<String> {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'b' => out.push('\u{8}'),
            'f' => out.push('\u{c}'),
            'x' => {
                let hi = chars.next()?.to_digit(16)?;
                let lo = chars.next()?.to_digit(16)?;
                out.push(char::from((hi * 16 + lo) as u8));
            }
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                let cp = u32::from_str_radix(&hex, 16).ok()?;
                out.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
            }
            other => out.push(other),
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 真实样本:ssis-960-chinese-subtitle 详情页的 source 配置块(radix 15)
    const SAMPLE: &str = r#"eval(function(p,a,c,k,e,d){e=function(c){return c.toString(36)};if(!''.replace(/^/,String)){while(c--){d[c.toString(a)]=k[c]||c.toString(a)}k=[function(e){return d[e]}];e=function(){return'\\w+'};c=1};while(c--){if(k[c]){p=p.replace(new RegExp('\\b'+e(c)+'\\b','g'),k[c])}}return p}('e=\'8://7.6/5-4-3-2-1/d.0\';c=\'8://7.6/5-4-3-2-1/a/9.0\';b=\'8://7.6/5-4-3-2-1/a/9.0\';',15,15,'m3u8|b0b0abf841ac|83f0|4897|3c25|4a5f763b|com|surrit|https|video|720p|source1280|source842|playlist|source'.split('|'),0,{}))"#;

    #[test]
    fn unpacks_packed_player_source() {
        let decoded = unpack_packed(SAMPLE).expect("应解码 packer 块");
        assert_eq!(
            decoded,
            "source='https://surrit.com/4a5f763b-3c25-4897-83f0-b0b0abf841ac/playlist.m3u8';\
             source842='https://surrit.com/4a5f763b-3c25-4897-83f0-b0b0abf841ac/720p/video.m3u8';\
             source1280='https://surrit.com/4a5f763b-3c25-4897-83f0-b0b0abf841ac/720p/video.m3u8';"
        );
        assert_eq!(
            extract_packed_m3u8(&format!("<html>{SAMPLE}</html>")).as_deref(),
            Some("https://surrit.com/4a5f763b-3c25-4897-83f0-b0b0abf841ac/playlist.m3u8")
        );
    }

    #[test]
    fn unpack_is_single_pass() {
        // 回填进去的词不再被二次替换(浏览器 eval 语义):词表 1 号词
        // 恰为 "0",不得再被替换成 0 号词
        let block = r"}('1 0',10,2,'zero|0'.split('|'),0,{}))";
        assert_eq!(unpack_packed(block).as_deref(), Some("0 zero"));
    }

    #[test]
    fn radix_digits_match_js() {
        assert_eq!(to_base(0, 36), "0");
        assert_eq!(to_base(35, 36), "z");
        assert_eq!(to_base(36, 36), "10");
        assert_eq!(to_base(61, 62), "Z");
        assert_eq!(js_unescape(r"\x41中\'").as_deref(), Some("A中'"));
    }
}
