//! missav 源的解析与抓取。
//! 从 video.rs 手工小步迁移(B2),每批编译核对。
use super::{to_base, BASE, MISSAV_BASE};
use crate::sources::shared::percent_decode_simple;
use regex::Regex;

/// 元数据区标签名(繁/简)→ 条目 kind
pub(crate) fn meta_kind_for_label(label: &str) -> Option<&'static str> {
    Some(match label {
        "女優" | "女优" => "actress",
        "男優" | "男优" => "actor",
        "類型" | "类型" => "genre",
        "系列" => "series",
        "發行商" | "发行商" => "maker",
        "導演" | "导演" => "director",
        "標籤" | "标签" => "label",
        _ => return None,
    })
}
/// missav 条目列表页地址:/ja/{路由}/{id}(id 按路径段编码),第 2 页起 ?page=N。
/// kind 兼容关注表里的单数形态(maker/actress)与元数据条目的 kind
pub(crate) fn missav_list_url(kind: &str, target_id: &str, page: i32) -> Result<String, String> {
    let route = missav_route_for_kind(kind).ok_or_else(|| format!("不支持的条目类型 {kind}"))?;
    let id = percent_decode_simple(target_id);
    if id.is_empty() {
        return Err("条目 id 为空".into());
    }
    let mut url = url::Url::parse(MISSAV_BASE).map_err(|e| e.to_string())?;
    // ja 语言前缀:列表卡片标题为日语原题(与 fs1 ?lang=jp、详情页「標題」一致)
    url.path_segments_mut()
        .map_err(|_| "无效地址".to_string())?
        .push("ja")
        .push(route)
        .push(&id);
    if page > 1 {
        url.set_query(Some(&format!("page={page}")));
    }
    Ok(url.to_string())
}
/// 条目 kind → missav 列表路由
pub(crate) fn missav_route_for_kind(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "maker" | "makers" => "makers",
        "actress" | "actresses" => "actresses",
        "actor" | "actors" => "actors",
        "series" => "series",
        "director" | "directors" => "directors",
        "label" | "labels" => "labels",
        "genre" | "genres" => "genres",
        _ => return None,
    })
}
pub(crate) fn absolute_url(value: &str) -> String {
    if value.starts_with("https://") || value.starts_with("http://") {
        value.to_string()
    } else if value.starts_with("//") {
        format!("https:{value}")
    } else if value.starts_with('/') {
        format!("{BASE}{value}")
    } else if value.is_empty() {
        String::new()
    } else {
        format!("{BASE}/{value}")
    }
}
/// 提取 meta-refresh 跳转地址并解析为绝对 URL(无则返回 None)。
/// missav 详情路径会先返回形如
/// `<meta http-equiv="refresh" content="0;url='https://missav.ai/dm82/…'">` 的跳转页
pub(crate) fn meta_refresh_url(html: &str, base_url: &str) -> Option<String> {
    let tag_re =
        Regex::new(r#"(?is)<meta[^>]*http-equiv\s*=\s*['"]?refresh[^>]*>"#).expect("static regex");
    let url_re = Regex::new(r#"(?i)url\s*=\s*['"]?([^'">\s]+)"#).expect("static regex");
    let tag = tag_re.find(html)?.as_str();
    let target = url_re.captures(tag)?.get(1)?.as_str();
    let base = url::Url::parse(base_url).ok()?;
    base.join(&target.replace("&amp;", "&"))
        .ok()
        .map(|u| u.to_string())
}
/// 解码 Dean Edwards packer:
/// `eval(function(p,a,c,k,e,d){…}('载荷',进制,词数,'k1|k2|…'.split('|'),0,{}))`
/// 与网关 JS 的 `eval` 求值等价:词表按序号(转 base 进制)回填载荷
pub(crate) fn unpack_packed(block: &str) -> Option<String> {
    let re = Regex::new(
        r#"(?s)\}\('((?:[^'\\]|\\.)*)',(\d+),(\d+),'((?:[^'\\]|\\.)*)'\.split\('\|'\),0,\{\}\)\)$"#,
    )
    .expect("static regex");
    let caps = re.captures(block.trim())?;
    let payload = js_unescape(caps.get(1)?.as_str())?;
    let radix: u32 = caps.get(2)?.as_str().parse().ok()?;
    let count: usize = caps.get(3)?.as_str().parse().ok()?;
    let words: Vec<String> = js_unescape(caps.get(4)?.as_str())?
        .split('|')
        .map(str::to_string)
        .collect();
    if !(2..=62).contains(&radix) {
        return None;
    }
    let mut out = payload;
    for (i, word) in words.iter().enumerate().rev().take(count) {
        if word.is_empty() {
            continue;
        }
        let token = to_base(i as u32, radix);
        let token_re = Regex::new(&format!(r"\b{}\b", regex::escape(&token))).expect("token regex");
        out = token_re
            .replace_all(&out, regex::NoExpand(word))
            .to_string();
    }
    Some(out)
}
/// 从 HTML 中解码首个含 m3u8 的 packer 块(与网关 JS 一致取首个地址)
pub(crate) fn extract_packed_m3u8(html: &str) -> Option<String> {
    let re = Regex::new(r"eval\(function\(p,a,c,k,e,d\)\{[\s\S]*?\}\)\)").expect("static regex");
    let m3u8_re = Regex::new(r#"https?://[^"'\s]+\.m3u8[^"'\s]*"#).expect("static regex");
    for block in re.find_iter(html) {
        let Some(decoded) = unpack_packed(block.as_str()) else {
            continue;
        };
        if let Some(m) = m3u8_re.find(&decoded) {
            return Some(m.as_str().to_string());
        }
    }
    None
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
                let hi = chars.next()?;
                let lo = chars.next()?;
                let byte = u8::from_str_radix(&format!("{hi}{lo}"), 16).ok()?;
                out.push(byte as char);
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
