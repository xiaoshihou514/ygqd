//! niacg HTML 解析(纯函数,网络请求在 commands 层)。
//!
//! 约定:选择器/正则全部为 `LazyLock` 静态量(列表页每个卡片都会命中,
//! 逐次编译曾是解析热点);节点缺失一律跳过或给空值,不 panic。

use super::shared::dedup_in_order;
use crate::models::*;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;
use std::sync::LazyLock;

const BASE: &str = "https://www.niacg.com";
/// 封面/图片懒加载属性的取值优先级
const IMG_ATTRS: &[&str] = &["data-src", "data-original", "src"];

/// 声明静态选择器:首次使用时 parse,字面量写错属编码错误(单测即暴露)
macro_rules! selectors {
    ($($name:ident = $css:expr;)*) => {
        $(static $name: LazyLock<Selector> =
            LazyLock::new(|| Selector::parse($css).expect("static selector"));)*
    };
}

selectors! {
    ITEM_LINK = "a[href*='moehome']";
    ITEM_TITLE = ".video-title";
    ITEM_TAG = ".tag";
    ITEM_LIKES = "[id^='albim_likes_']";
    CATEGORY_LABEL = ".label-category, .label-sub";
    IMG = "img";
    LIST_CARDS = ".owl-item .p-b-15, .owl-item, .list-col .p-b-15, .p-b-15";
    SEARCH_CARDS = ".list-col .p-b-15, .owl-item .p-b-15, .p-b-15";
    PAGINATION_LINK = ".pagination a";
    DETAIL_AUTHOR_BLOCK = ".tag-block [data-type='author']";
    BTN_LINK = "a.btn";
    DETAIL_WORKS = ".tag-block [data-type='works'] a.btn";
    DETAIL_ACTORS = ".tag-block [data-type='actor'] a.btn";
    DETAIL_TAGS = ".tag-block [data-type='tags'] a.btn";
    DETAIL_TITLE = "h1, .panel-title";
    DETAIL_COVER = "#album_photo_cover img";
    DETAIL_LIKES = "#diggnum, [id^='albim_likes_']";
    COMIC_IMG = "img.comic_img";
}

macro_rules! regexes {
    ($($name:ident = $re:expr;)*) => {
        $(static $name: LazyLock<Regex> =
            LazyLock::new(|| Regex::new($re).expect("static regex"));)*
    };
}

regexes! {
    ITEM_HREF_RE = r"moehome-(\d+)-(\d+)\.html";
    LIST_PAGE_RE = r"listinfo-\d+-(\d+)\.html";
    SEARCH_PAGE_RE = r"(?i)[?&]page=(\d+)";
    TAG_PAGE_RE = r"tags-[^-]+-(\d+)\.html";
}

/// 节点文本去首尾空白(无需裁剪时不做二次分配)
fn text(el: ElementRef<'_>) -> String {
    let raw: String = el.text().collect();
    let trimmed = raw.trim();
    if trimmed.len() == raw.len() {
        raw
    } else {
        trimmed.to_owned()
    }
}

/// 首个匹配节点的文本(无节点为空串)
fn first_text(scope: ElementRef<'_>, selector: &Selector) -> String {
    scope.select(selector).next().map(text).unwrap_or_default()
}

/// 全部匹配节点的非空文本,保序去重
fn texts<'a>(nodes: impl Iterator<Item = ElementRef<'a>>) -> Vec<String> {
    dedup_in_order(nodes.map(text).filter(|v| !v.is_empty()))
}

/// 首个分类标签的非空文本,缺失时按分类 id 回退为内置名称
fn category_label(scope: ElementRef<'_>, category_id: i32) -> String {
    scope
        .select(&CATEGORY_LABEL)
        .next()
        .map(text)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| category(category_id))
}

fn category(id: i32) -> String {
    match id {
        1 => "COS",
        2 => "CG",
        3 => "本子",
        4 => "套图",
        9 => "A漫",
        19 => "里番",
        20 => "3D",
        21 => "同人",
        _ => "",
    }
    .into()
}

pub fn resolve_url(value: &str) -> String {
    if value.is_empty() {
        String::new()
    } else if value.starts_with("http://") || value.starts_with("https://") {
        value.into()
    } else if value.starts_with("//") {
        format!("https:{value}")
    } else {
        format!("{BASE}/{}", value.trim_start_matches('/'))
    }
}

/// 按 `names` 优先级取首个存在的属性并补全为绝对地址
fn attr(el: Option<ElementRef<'_>>, names: &[&str]) -> String {
    el.and_then(|e| names.iter().find_map(|name| e.value().attr(name)))
        .map(resolve_url)
        .unwrap_or_default()
}

/// 解析单个作品卡片;缺少作品链接或链接格式不符时跳过(返回 None)
fn item(el: ElementRef<'_>) -> Option<ComicItem> {
    let href = el.select(&ITEM_LINK).next()?.value().attr("href")?;
    let caps = ITEM_HREF_RE.captures(href)?;
    let category_id = caps[1].parse().ok()?;
    let id = caps[2].to_string();
    Some(ComicItem {
        source: Source::Niacg,
        id,
        title: first_text(el, &ITEM_TITLE),
        author: String::new(),
        thumbnail: attr(el.select(&IMG).next(), IMG_ATTRS),
        category: category_label(el, category_id),
        category_id,
        tags: texts(el.select(&ITEM_TAG)),
        likes: first_text(el, &ITEM_LIKES),
        link: href.to_string(),
    })
}

/// 按 id 去重(多个卡片选择器会重叠命中同一作品)
fn unique_items(doc: &Html, selector: &Selector) -> Vec<ComicItem> {
    let mut seen = HashSet::new();
    doc.select(selector)
        .filter_map(item)
        .filter(|v| seen.insert(v.id.clone()))
        .collect()
}

/// 分页链接 href 中按正则第 1 组提取的页号
fn page_numbers<'a>(doc: &'a Html, re: &'a Regex) -> impl Iterator<Item = i32> + 'a {
    doc.select(&PAGINATION_LINK)
        .filter_map(|a| a.value().attr("href"))
        .filter_map(|h| re.captures(h)?.get(1)?.as_str().parse().ok())
}

pub fn list(html: &str) -> CategoryListResult {
    let doc = Html::parse_document(html);
    let items = unique_items(&doc, &LIST_CARDS);
    let pages: Vec<i32> = page_numbers(&doc, &LIST_PAGE_RE).collect();
    let current = pages.first().copied().unwrap_or(0);
    let total = pages.iter().max().copied().unwrap_or(current + 1);
    CategoryListResult {
        pagination: PaginationInfo {
            current,
            total,
            // 空页(越界或解析不到卡片)不再报告有下一页,避免前端无限滚动空转
            has_next: current < total && !items.is_empty(),
            has_prev: current > 0,
        },
        items,
    }
}

pub fn search(html: &str) -> SearchResult {
    let doc = Html::parse_document(html);
    let items = unique_items(&doc, &SEARCH_CARDS);
    let total = page_numbers(&doc, &SEARCH_PAGE_RE).max().unwrap_or(1) - 1;
    let page_url_template = doc
        .select(&PAGINATION_LINK)
        .find_map(|a| a.value().attr("href"))
        .map(|u| SEARCH_PAGE_RE.replace(u, "?page={}").into_owned());
    SearchResult {
        items,
        pagination: PaginationInfo {
            current: 0,
            total,
            has_next: total > 0,
            has_prev: false,
        },
        page_url_template,
    }
}

pub fn tag_max_page(html: &str) -> i32 {
    TAG_PAGE_RE
        .captures_iter(html)
        .filter_map(|c| c[1].parse().ok())
        .max()
        .unwrap_or(0)
}

pub fn detail(html: &str, category_id: i32, id: String) -> ComicDetail {
    let doc = Html::parse_document(html);
    let root = doc.root_element();
    // 页面可能出现多个作者块,只取第一个
    let author = doc
        .select(&DETAIL_AUTHOR_BLOCK)
        .next()
        .map(|block| texts(block.select(&BTN_LINK)).join(", "))
        .unwrap_or_default();
    ComicDetail {
        source: Source::Niacg,
        id,
        title: first_text(root, &DETAIL_TITLE),
        thumbnail: attr(doc.select(&DETAIL_COVER).next(), IMG_ATTRS),
        category: category_label(root, category_id),
        category_id,
        author,
        works: texts(doc.select(&DETAIL_WORKS)),
        characters: texts(doc.select(&DETAIL_ACTORS)),
        tags: texts(doc.select(&DETAIL_TAGS)),
        likes: first_text(root, &DETAIL_LIKES),
        episodes: vec![],
        images: vec![],
        // niacg 无账号体系,点赞/收藏状态恒为 false(收藏由本地表管理)
        is_liked: false,
        is_favourite: false,
    }
}

pub fn images(html: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let mut seen = HashSet::new();
    doc.select(&COMIC_IMG)
        .map(|e| attr(Some(e), IMG_ATTRS))
        .filter(|u| {
            !u.is_empty()
                && (u.contains("boom") || u.contains("xunge") || u.contains("hen"))
                && seen.insert(u.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_author_only_from_the_first_author_block() {
        let parsed = detail(
            r#"<html><body>
                <div class="tag-block"><span data-type="author"><a class="btn">Author</a></span></div>
                <div class="tag-block"><span data-type="author"><a class="btn">Author</a></span></div>
            </body></html>"#,
            3,
            "42".into(),
        );

        assert_eq!(parsed.author, "Author");
    }

    #[test]
    fn deduplicates_tags_keeping_order() {
        let parsed = detail(
            r#"<html><body><h1>Example</h1>
                <div class="tag-block"><span data-type="tags">
                    <a class="btn">纯爱</a><a class="btn">纯爱</a><a class="btn">萝莉</a><a class="btn">萝莉</a>
                </span></div>
            </body></html>"#,
            3,
            "42".into(),
        );

        assert_eq!(parsed.tags, vec!["纯爱", "萝莉"]);
    }

    #[test]
    fn deduplicates_works_keeping_order() {
        let parsed = detail(
            r#"<html><body><h1>Example</h1>
                <div class="tag-block"><span data-type="works">
                    <a class="btn">作品A</a><a class="btn">作品B</a><a class="btn">作品A</a>
                </span></div>
            </body></html>"#,
            3,
            "42".into(),
        );

        assert_eq!(parsed.works, vec!["作品A", "作品B"]);
    }

    #[test]
    fn detail_of_empty_page_falls_back_without_panicking() {
        let parsed = detail("<html><body></body></html>", 3, "42".into());
        assert_eq!(parsed.title, "");
        assert_eq!(parsed.category, "本子");
        assert_eq!(parsed.thumbnail, "");
        assert!(parsed.tags.is_empty());
    }

    #[test]
    fn list_parses_cards_dedups_and_skips_broken_ones() {
        let parsed = list(
            r#"<html><body><div class="list-col">
                <div class="p-b-15">
                    <a href="/moehome-3-101.html"><img data-src="//img.example/a.jpg"></a>
                    <div class="video-title"> 标题A </div>
                    <span class="tag">纯爱</span><span class="tag">纯爱</span><span class="tag"> </span>
                    <span id="albim_likes_101">12</span>
                </div>
                <div class="p-b-15"><a href="/moehome-3-101.html">重复</a></div>
                <div class="p-b-15"><a href="/moehome-x-y.html">坏链接</a></div>
                <div class="p-b-15"><span>无链接</span></div>
            </div>
            <div class="pagination">
                <a href="/listinfo-3-0.html">1</a><a href="/listinfo-3-4.html">5</a>
            </div></body></html>"#,
        );
        assert_eq!(parsed.items.len(), 1);
        let item = &parsed.items[0];
        assert_eq!(item.id, "101");
        assert_eq!(item.category_id, 3);
        assert_eq!(item.category, "本子");
        assert_eq!(item.title, "标题A");
        assert_eq!(item.tags, vec!["纯爱"]);
        assert_eq!(item.likes, "12");
        assert_eq!(item.thumbnail, "https://img.example/a.jpg");
        assert_eq!(parsed.pagination.total, 4);
        assert!(parsed.pagination.has_next);
    }

    #[test]
    fn empty_list_page_reports_no_next() {
        let parsed = list("<html><body></body></html>");
        assert!(parsed.items.is_empty());
        assert!(!parsed.pagination.has_next);
    }

    #[test]
    fn search_extracts_total_and_page_template() {
        let parsed = search(
            r#"<div class="pagination">
                <a href="/e/search/result/?page=1&searchid=9">2</a>
                <a href="/e/search/result/?page=3&searchid=9">4</a>
            </div>"#,
        );
        assert_eq!(parsed.pagination.total, 2);
        assert!(parsed.pagination.has_next);
        assert_eq!(
            parsed.page_url_template.as_deref(),
            Some("/e/search/result/?page={}&searchid=9")
        );
    }

    #[test]
    fn images_filter_hosts_and_dedup() {
        let parsed = images(
            r#"<img class="comic_img" data-src="https://boom.example/1.jpg">
               <img class="comic_img" src="https://boom.example/1.jpg">
               <img class="comic_img" src="https://ads.example/x.jpg">
               <img class="comic_img" data-original="//hen.example/2.jpg">"#,
        );
        assert_eq!(
            parsed,
            vec!["https://boom.example/1.jpg", "https://hen.example/2.jpg"]
        );
    }

    #[test]
    fn tag_max_page_reads_highest_number() {
        assert_eq!(
            tag_max_page(r#"<a href="/tags-abc-2.html"></a><a href="/tags-abc-7.html"></a>"#),
            7
        );
        assert_eq!(tag_max_page(""), 0);
    }
}
