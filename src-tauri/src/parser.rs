use crate::models::*;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::collections::HashSet;

const BASE: &str = "https://www.niacg.com";
fn sel(value: &str) -> Selector {
    Selector::parse(value).expect("static selector")
}
fn text(el: ElementRef<'_>) -> String {
    el.text().collect::<String>().trim().to_string()
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
        "".into()
    } else if value.starts_with("http://") || value.starts_with("https://") {
        value.into()
    } else if value.starts_with("//") {
        format!("https:{value}")
    } else {
        format!("{BASE}/{}", value.trim_start_matches('/'))
    }
}
fn attr(el: Option<ElementRef<'_>>, names: &[&str]) -> String {
    el.and_then(|e| {
        names
            .iter()
            .find_map(|name| e.value().attr(name))
            .map(resolve_url)
    })
    .unwrap_or_default()
}

fn item(el: ElementRef<'_>) -> Option<ComicItem> {
    let link = el.select(&sel("a[href*='moehome']")).next()?;
    let href = link.value().attr("href")?.to_string();
    let caps = Regex::new(r"moehome-(\d+)-(\d+)\.html")
        .unwrap()
        .captures(&href)?;
    let category_id = caps[1].parse().ok()?;
    let id = caps[2].to_string();
    let title = el
        .select(&sel(".video-title"))
        .next()
        .map(text)
        .unwrap_or_default();
    let tags = el
        .select(&sel(".tag"))
        .map(text)
        .filter(|v| !v.is_empty())
        .collect();
    let likes = el
        .select(&sel("[id^='albim_likes_']"))
        .next()
        .map(text)
        .unwrap_or_default();
    let parsed_category = el
        .select(&sel(".label-category, .label-sub"))
        .next()
        .map(text)
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| category(category_id));
    Ok::<_, ()>(ComicItem {
        id,
        title,
        thumbnail: attr(
            el.select(&sel("img")).next(),
            &["data-src", "data-original", "src"],
        ),
        category: parsed_category,
        category_id,
        tags,
        likes,
        link: href,
    })
    .ok()
}

fn unique_items(doc: &Html, selector: &str) -> Vec<ComicItem> {
    let mut seen = HashSet::new();
    doc.select(&sel(selector))
        .filter_map(item)
        .filter(|v| seen.insert(v.id.clone()))
        .collect()
}

pub fn home(html: &str) -> Vec<HomeSection> {
    let doc = Html::parse_document(html);
    let mut heading: Option<(i32, String)> = None;
    let mut sections = Vec::new();
    for element in doc.select(&sel("h4, .owl-carousel")) {
        if element.value().name() == "h4" {
            let value = text(element);
            heading = if value.contains("COS") {
                Some((1, "COS".into()))
            } else if value.contains("套图") {
                Some((4, "套图".into()))
            } else if value.contains("CG") {
                Some((2, "CG".into()))
            } else if value.contains("本本") || value.contains("本子") {
                Some((3, "本子".into()))
            } else if value.contains("里番") {
                Some((19, "里番".into()))
            } else if value.contains("3D") {
                Some((20, "3D".into()))
            } else if value.contains("同人") {
                Some((21, "同人".into()))
            } else {
                None
            };
        } else if let Some((category_id, category)) = heading.take() {
            let mut seen = HashSet::new();
            let items: Vec<_> = element
                .select(&sel(".owl-item .p-b-15"))
                .filter_map(item)
                .filter(|v| seen.insert(v.id.clone()))
                .collect();
            if !items.is_empty() {
                sections.push(HomeSection {
                    label: format!("{category}推荐"),
                    category,
                    category_id,
                    items,
                });
            }
        }
    }
    sections
}

pub fn list(html: &str) -> CategoryListResult {
    let doc = Html::parse_document(html);
    let items = unique_items(
        &doc,
        ".owl-item .p-b-15, .owl-item, .list-col .p-b-15, .p-b-15",
    );
    let re = Regex::new(r"listinfo-\d+-(\d+)\.html").unwrap();
    let pages: Vec<i32> = doc
        .select(&sel(".pagination a"))
        .filter_map(|a| a.value().attr("href"))
        .filter_map(|h| re.captures(h)?.get(1)?.as_str().parse().ok())
        .collect();
    let current = pages.first().copied().unwrap_or(0);
    let total = pages.iter().max().copied().unwrap_or(current + 1);
    CategoryListResult {
        items,
        pagination: PaginationInfo {
            current,
            total,
            has_next: current < total,
            has_prev: current > 0,
        },
    }
}

pub fn search(html: &str) -> SearchResult {
    let doc = Html::parse_document(html);
    let items = unique_items(&doc, ".list-col .p-b-15, .owl-item .p-b-15, .p-b-15");
    let re = Regex::new(r"(?i)[?&]page=(\d+)").unwrap();
    let urls: Vec<String> = doc
        .select(&sel(".pagination a"))
        .filter_map(|a| a.value().attr("href"))
        .map(str::to_string)
        .collect();
    let total = urls
        .iter()
        .filter_map(|u| re.captures(u)?.get(1)?.as_str().parse::<i32>().ok())
        .max()
        .unwrap_or(1)
        - 1;
    let page_url_template = urls.first().map(|u| re.replace(u, "?page={}").to_string());
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
    Regex::new(r"tags-[^-]+-(\d+)\.html")
        .unwrap()
        .captures_iter(html)
        .filter_map(|c| c[1].parse().ok())
        .max()
        .unwrap_or(0)
}

pub fn detail(html: &str, category_id: i32, id: String) -> ComicDetail {
    let doc = Html::parse_document(html);
    let page_text = doc.root_element().text().collect::<String>();
    let published_at = Regex::new(r"上架日期\s*[:：]\s*(\d{4}-\d{2}-\d{2})")
        .expect("static date regex")
        .captures(&page_text)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string());
    let list = |selector: &str| {
        doc.select(&sel(selector))
            .map(text)
            .filter(|v| !v.is_empty())
            .collect::<Vec<String>>()
    };
    let author = doc
        .select(&sel(".tag-block [data-type='author']"))
        .next()
        .map(|block| {
            block
                .select(&sel("a.btn"))
                .map(text)
                .filter(|value| !value.is_empty())
                .collect::<Vec<String>>()
                .join(", ")
        })
        .unwrap_or_default();
    ComicDetail {
        id,
        title: doc
            .select(&sel("h1, .panel-title"))
            .next()
            .map(text)
            .unwrap_or_default(),
        thumbnail: attr(
            doc.select(&sel("#album_photo_cover img")).next(),
            &["data-src", "data-original", "src"],
        ),
        category: doc
            .select(&sel(".label-category, .label-sub"))
            .next()
            .map(text)
            .unwrap_or_else(|| category(category_id)),
        category_id,
        author,
        works: list(".tag-block [data-type='works'] a.btn"),
        characters: list(".tag-block [data-type='actor'] a.btn"),
        tags: list(".tag-block [data-type='tags'] a.btn"),
        likes: doc
            .select(&sel("#diggnum, [id^='albim_likes_']"))
            .next()
            .map(text)
            .unwrap_or_default(),
        published_at,
        images: vec![],
    }
}

pub fn images(html: &str) -> Vec<String> {
    let doc = Html::parse_document(html);
    let mut seen = HashSet::new();
    doc.select(&sel("img.comic_img"))
        .filter_map(|e| {
            let u = attr(Some(e), &["data-src", "data-original", "src"]);
            (!u.is_empty()
                && (u.contains("boom") || u.contains("xunge") || u.contains("hen"))
                && seen.insert(u.clone()))
            .then_some(u)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::detail;

    #[test]
    fn parses_published_date_from_detail_text() {
        let parsed = detail(
            "<html><body><h1>Example</h1><div>上架日期 : 2026-06-08</div></body></html>",
            3,
            "42".into(),
        );
        assert_eq!(parsed.published_at.as_deref(), Some("2026-06-08"));
    }

    #[test]
    fn missing_published_date_is_none() {
        let parsed = detail("<html><body><h1>Example</h1></body></html>", 3, "42".into());
        assert_eq!(parsed.published_at, None);
    }

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
}
