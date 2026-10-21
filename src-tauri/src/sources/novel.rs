//! 小说板块:hlib.cc,经 WebView 网关抓取(站内有 CF 挑战+登录墙,
//! 网关窗口由用户完成一次登录与验证,cookie 持久化后 fetch 直通)。
//!
//! 页面结构(参照社区油猴脚本 hlib图书馆下载 的选择器):
//! - 系列目录 /s/{id}:章节链接 a[href^="/n/"],文本形如 "#12 标题";
//!   目录自身按 ?p=N 分页
//! - 文章正文 /n/{id}?p=N:正文容器 #content,标题 h3.text-center,
//!   作者为正文首行"作者：xxx"或 a[href^="/u/"],页码选择器
//!   select[onchange*="rp('p'"] 的 option 集合
//! - 站点对连续抓取敏感,章节间隔应 >= 10 秒(由调用方控制)

use crate::gateway;
use crate::models::{NovelChapter, NovelPage, NovelSeries};
use scraper::{Html, Selector};
use std::time::Duration;
use tauri::AppHandle;

const LABEL: &str = "hlib";

fn sel(v: &str) -> Selector {
    Selector::parse(v).expect("static selector")
}

async fn fetch_html(app: &AppHandle, url: &str) -> Result<String, String> {
    use tauri::Manager as _;
    let manager = app.state::<crate::GatewayManager>();
    gateway::gateway_fetch(app, &manager, LABEL, url).await
}

/// 从用户输入提取系列 id:"https://hlib.cc/s/abc?x" 或 "/s/abc" 或 "abc"
pub fn parse_series_id(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let id = if let Some(rest) = trimmed.strip_prefix("https://hlib.cc/s/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("http://hlib.cc/s/") {
        rest
    } else if let Some(rest) = trimmed.strip_prefix("/s/") {
        rest
    } else if trimmed.starts_with("http") {
        return Err("仅支持 hlib.cc 的系列(/s/)链接或纯 ID".into());
    } else {
        trimmed
    };
    let id = id.split(['?', '#', '/']).next().unwrap_or("").to_string();
    if id.is_empty() {
        return Err("无法从输入中提取系列 ID".into());
    }
    Ok(id)
}

fn normalize_text(text: &str) -> String {
    text.replace("\u{a0}", " ")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn is_challenge(html: &str) -> bool {
    html.contains("请稍候") || html.contains("Just a moment") || html.contains("正在进行安全验证")
}

fn total_pages(doc: &Html) -> i64 {
    let mut max = 1i64;
    for opt in doc.select(&sel("select option")) {
        if let Ok(v) = opt.value().attr("value").unwrap_or("").parse::<i64>() {
            max = max.max(v);
        }
    }
    for a in doc.select(&sel("a[href*='p=']")) {
        let href = a.value().attr("href").unwrap_or("");
        if let Some(p) = href.split("p=").nth(1).and_then(|s| s.parse::<i64>().ok()) {
            max = max.max(p);
        }
    }
    max
}

/// 解析系列目录一页,返回(标题, 作者, 本页章节)
fn parse_series_page(html: &str) -> (String, String, Vec<NovelChapter>) {
    let doc = Html::parse_document(html);
    let title = doc
        .select(&sel("h3.text-center"))
        .next()
        .map(|h| h.text().collect::<String>())
        .unwrap_or_default();
    let author = doc
        .select(&sel("a[href^='/u/']"))
        .next()
        .map(|a| a.text().collect::<String>())
        .unwrap_or_default();
    let mut chapters = Vec::new();
    for a in doc.select(&sel("a[href^='/n/']")) {
        let Some(id) = a
            .value()
            .attr("href")
            .and_then(|h| h.strip_prefix("/n/"))
            .map(|s| s.split(['?', '#']).next().unwrap_or("").to_string())
        else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        let text = a.text().collect::<String>();
        // "#12 标题" 提取序号
        let trimmed = text.trim();
        let (order, chapter_title) = if let Some(rest) = trimmed.strip_prefix('#') {
            let mut split = rest.splitn(2, char::is_whitespace);
            let num = split.next().unwrap_or("");
            match num.parse::<i64>() {
                Ok(n) => (n, split.next().unwrap_or("").trim().to_string()),
                Err(_) => continue,
            }
        } else {
            continue;
        };
        chapters.push(NovelChapter {
            id,
            order,
            title: chapter_title,
        });
    }
    (normalize_text(&title), normalize_text(&author), chapters)
}

/// 拉取整个系列目录(分页)入库。调用方负责入库与限速。
pub async fn fetch_series(app: &AppHandle, id: &str) -> Result<NovelSeries, String> {
    let mut all: Vec<NovelChapter> = Vec::new();
    let mut title = String::new();
    let mut author = String::new();
    let mut page = 1;
    loop {
        let url = if page == 1 {
            format!("https://hlib.cc/s/{id}")
        } else {
            format!("https://hlib.cc/s/{id}?p={page}")
        };
        let html = fetch_html(app, &url).await?;
        if is_challenge(&html) {
            return Err("hlib 安全验证未通过:请先在弹出的网关窗口完成验证/登录".into());
        }
        let (t, a, chapters) = parse_series_page(&html);
        if title.is_empty() {
            title = t;
            author = a;
        }
        // tendril(非 Send)不能跨 await,放进临时作用域即取即弃
        let tp = { total_pages(&Html::parse_document(&html)) };
        let before = all.len();
        for ch in chapters {
            if !all.iter().any(|c| c.id == ch.id) {
                all.push(ch);
            }
        }
        if page >= tp || all.len() == before {
            break;
        }
        page += 1;
        tokio::time::sleep(Duration::from_secs(4)).await;
    }
    if all.is_empty() {
        return Err("系列目录中没有解析到章节(链接需以 #序号 开头)".into());
    }
    all.sort_by_key(|c| c.order);
    Ok(NovelSeries {
        id: id.to_string(),
        title,
        author,
        chapters: all,
        added_at: 0,
    })
}

/// 拉取文章某一页正文
pub async fn fetch_article_page(app: &AppHandle, id: &str, page: i64) -> Result<NovelPage, String> {
    let url = format!("https://hlib.cc/n/{id}?p={page}");
    let html = fetch_html(app, &url).await?;
    if is_challenge(&html) {
        return Err("hlib 安全验证未通过:请先在弹出的网关窗口完成验证/登录".into());
    }
    let doc = Html::parse_document(&html);
    let content = doc
        .select(&sel("#content"))
        .next()
        .ok_or("未找到正文(#content),可能未登录或文章未解锁")?;
    let text = normalize_text(&content.text().collect::<String>());
    if text.is_empty() {
        return Err("正文为空".into());
    }
    let title = doc
        .select(&sel("h3.text-center"))
        .next()
        .map(|h| normalize_text(&h.text().collect::<String>()))
        .unwrap_or_default();
    let author = text
        .lines()
        .find_map(|l| l.strip_prefix("作者：").or_else(|| l.strip_prefix("作者:")))
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    let tp = total_pages(&doc);
    Ok(NovelPage {
        title,
        author,
        text,
        total_pages: tp,
    })
}

/// 搜索/发现命中的系列
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelHit {
    pub id: String,
    pub title: String,
}

/// 从任意 HTML 提取系列链接(/s/{id} -> 链接文本)。
/// 不依赖具体页面结构,列表/搜索/首页通用。
fn extract_series(html: &str) -> Vec<NovelHit> {
    let doc = Html::parse_document(html);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for a in doc.select(&sel("a[href^='/s/']")) {
        let Some(id) = a
            .value()
            .attr("href")
            .and_then(|h| h.strip_prefix("/s/"))
            .map(|s| s.split(['?', '#']).next().unwrap_or("").to_string())
        else {
            continue;
        };
        if id.is_empty() || !seen.insert(id.clone()) {
            continue;
        }
        let title = normalize_text(&a.text().collect::<String>());
        if title.is_empty() {
            continue;
        }
        out.push(NovelHit { id, title });
    }
    out
}

/// 站内搜索:搜索端点结构未知(CF 挡侦察),对常见形态自适应探测,
/// 首个返回有效系列链接的即采用。挑战页返回引导错误。
pub async fn search_series(app: &AppHandle, keyword: &str) -> Result<Vec<NovelHit>, String> {
    let encoded: String = url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect();
    let candidates = [
        format!("https://hlib.cc/search?q={encoded}"),
        format!("https://hlib.cc/search/{encoded}"),
        format!("https://hlib.cc/search?keyword={encoded}"),
        format!("https://hlib.cc/s?kw={encoded}"),
    ];
    let mut last_err = String::new();
    for url in &candidates {
        match fetch_html(app, url).await {
            Ok(html) => {
                if is_challenge(&html) {
                    return Err("hlib 安全验证未通过:请先在网关窗口完成验证/登录".into());
                }
                let hits = extract_series(&html);
                if !hits.is_empty() {
                    return Ok(hits);
                }
                last_err = "搜索端点无结果".into();
            }
            Err(e) => last_err = e,
        }
    }
    Err(format!("未能找到可用的搜索端点({last_err})"))
}

/// 发现:首页提取最新系列(不依赖具体列表结构)
pub async fn latest_series(app: &AppHandle) -> Result<Vec<NovelHit>, String> {
    let html = fetch_html(app, "https://hlib.cc/").await?;
    if is_challenge(&html) {
        return Err("hlib 安全验证未通过:请先在网关窗口完成验证/登录".into());
    }
    let hits = extract_series(&html);
    if hits.is_empty() {
        return Err("首页未解析到系列链接".into());
    }
    Ok(hits)
}
