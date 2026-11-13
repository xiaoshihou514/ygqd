//! 两源共用的 HTML 文本小工具(正则/选择器均只编译一次)。

use regex::Regex;
use scraper::Selector;
use std::sync::LazyLock;

static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").expect("static regex"));
static OG_TITLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<meta property="og:title" content="([^"]*)""#).expect("static regex")
});
/// 属性值形态的 href(missav 搜索主解析与兜底扫描共用)
pub(super) static HREF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"href="([^"]+)""#).expect("static regex"));

/// 静态选择器(只在 LazyLock 初始化时调用)
pub(super) fn sel(value: &str) -> Selector {
    Selector::parse(value).expect("static selector")
}

/// 去标签并折叠空白
pub(super) fn strip_tags(s: &str) -> String {
    TAG.replace_all(s, " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// og:title(去首尾空白;空值视为无)
pub(super) fn og_title(html: &str) -> Option<String> {
    OG_TITLE
        .captures(html)
        .map(|c| c[1].trim().to_string())
        .filter(|t| !t.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_collapses_whitespace() {
        assert_eq!(strip_tags("<b>甲</b>\n  <i>乙</i> "), "甲 乙");
        assert_eq!(strip_tags("<span></span>"), "");
    }

    #[test]
    fn og_title_ignores_empty() {
        assert_eq!(
            og_title(r#"<meta property="og:title" content=" 标题 ">"#).as_deref(),
            Some("标题")
        );
        assert!(og_title(r#"<meta property="og:title" content="">"#).is_none());
    }
}
