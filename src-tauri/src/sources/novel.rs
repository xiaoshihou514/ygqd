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

fn show_hlib_gateway(app: &AppHandle) {
    use tauri::Manager as _;
    if let Some(window) = app.get_webview_window("gateway_hlib") {
        gateway::present(&window);
    }
}

async fn fetch_html(app: &AppHandle, url: &str) -> Result<String, String> {
    use tauri::Manager as _;
    let manager = app.state::<crate::GatewayManager>();
    let html = match gateway::gateway_fetch(app, &manager, LABEL, url).await {
        Ok(html) => html,
        Err(error) if error.contains("HTTP 403") => {
            show_hlib_gateway(app);
            return Err("hlib 拒绝了请求:请在弹出窗口确认已登录并完成安全验证后重试".into());
        }
        Err(error) => return Err(error),
    };
    if is_challenge(&html) {
        // 会话失效:把网关窗口弹回前台,用户当场重新验证/登录
        show_hlib_gateway(app);
        return Err("hlib 安全验证未通过:已在弹出窗口中打开 hlib,请完成验证/登录后重试".into());
    }
    if is_login_page(&html) {
        // 登录墙:网关直接打开登录页并前置,用户登录一次即 cookie 持久复用
        gateway::navigate_and_present(app, LABEL, "https://hlib.cc/login");
        return Err("hlib 需要登录:已在弹出窗口打开登录页,请登录后重试".into());
    }
    Ok(html)
}

/// 从导航 href 提取系列 id("/s/abc" 相对/绝对均兼容)
fn series_id_from_href_nav(href: &str) -> Option<String> {
    let path = url::Url::parse("https://hlib.cc/")
        .ok()?
        .join(href)
        .ok()?
        .path()
        .to_string();
    let id = path
        .strip_prefix("/s/")?
        .split(['?', '#'])
        .next()?
        .to_string();
    (!id.is_empty()).then_some(id)
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

/// 登录墙判定:hlib 部分端点(如 search?q=)未登录时返回独立登录页
/// (title 即「登录」——精确形态匹配,避免正常页面含登录按钮的误判)
fn is_login_page(html: &str) -> bool {
    re_title_login()
        .captures(html)
        .map(|c| {
            c.get(1)
                .map(|t| t.as_str().trim() == "登录")
                .unwrap_or(false)
        })
        .unwrap_or(false)
}

fn re_title_login() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| regex::Regex::new(r"<title>([^<]*)</title>").expect("static regex"))
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
            return Err("hlib 安全验证未通过:请在网关窗口完成验证/登录后重试".into());
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
        return Err("hlib 安全验证未通过:请在网关窗口完成验证/登录后重试".into());
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
    /// 作者名(条目级 /u/ 链接文本;列表页 DOM 含该字段)
    #[serde(default)]
    pub author: String,
    /// 标签集合(条目级 .tags 容器的 /tag/ 链接文本)
    #[serde(default)]
    pub tags: Vec<String>,
}

/// 从站内任意形式的系列链接(/s/{id})提取标识,兼容绝对与相对地址。
fn series_id_from_href(href: &str) -> Option<String> {
    let base = url::Url::parse("https://hlib.cc/").ok()?;
    let url = base.join(href).ok()?;
    let host = url.host_str()?;
    if host != "hlib.cc" && host != "www.hlib.cc" {
        return None;
    }
    let mut segments = url.path_segments()?;
    if segments.next()? != "s" {
        return None;
    }
    let id = segments.next()?.to_string();
    (!id.is_empty()).then_some(id)
}

/// 从任意 HTML 提取系列信息。优先条目级解析(系列/列表/搜索结果的
/// li.list-group-item 结构:id+标题+作者+标签),无该结构时回退全文
/// /s/ 链接扫描(仅 id+title,如首页等异构页面)。
fn extract_series(html: &str) -> Vec<NovelHit> {
    let doc = Html::parse_document(html);
    let entry_sel = sel("li.list-group-item");
    let entries = doc.select(&entry_sel);
    if entries.clone().count() > 0 {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for item in entries {
            let Some(link) = item.select(&sel("a[href^='/s/']")).next() else {
                continue;
            };
            let Some(id) = link.value().attr("href").and_then(series_id_from_href_nav) else {
                continue;
            };
            if !seen.insert(id.clone()) {
                continue;
            }
            let title = normalize_text(&link.text().collect::<String>());
            let author = item
                .select(&sel("a[href^='/u/']"))
                .next()
                .map(|a| normalize_text(&a.text().collect::<String>()))
                .unwrap_or_default();
            let tags: Vec<String> = item
                .select(&sel(".tags a[href^='/tag/']"))
                .map(|a| normalize_text(&a.text().collect::<String>()))
                .filter(|t| !t.is_empty())
                .collect();
            let final_title = if title.is_empty() { id.clone() } else { title };
            out.push(NovelHit {
                id,
                title: final_title,
                author,
                tags,
            });
        }
        if !out.is_empty() {
            return out;
        }
    }
    // 回退:异构页面(首页等)全文 /s/ 链接扫描,仅 id+title
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for a in doc.select(&sel("a[href*='/s/']")) {
        let Some(id) = a.value().attr("href").and_then(series_id_from_href) else {
            continue;
        };
        if !seen.insert(id.clone()) {
            continue;
        }
        let title = normalize_text(&a.text().collect::<String>());
        let title = if title.is_empty() {
            a.value().attr("title").unwrap_or(&id).to_string()
        } else {
            title
        };
        out.push(NovelHit {
            id,
            title,
            author: String::new(),
            tags: Vec::new(),
        });
    }
    out
}

/// 站内搜索端点(2024-11 样本取证定案):站内搜索表单为
/// `<form method="get" action="/s/search"><input name="query">`,
/// 即 GET /s/search?query={关键词}(需登录,未登录返回登录页)。
/// 只打这一个端点:历史上的表单探测与多模板级联(/s?kw= 固定列表、
/// search?q= 全站搜索页无系列条目)每次搜索多发 1~3 个请求,
/// 会累加站点反爬闸「伊蕾娜」的访问频率计数。
const SEARCH_URL: &str = "https://hlib.cc/s/search?query=";

pub async fn search_series(app: &AppHandle, keyword: &str) -> Result<Vec<NovelHit>, String> {
    Ok(search_series_page(app, keyword, 1).await?.hits)
}

/// 搜索结果的一页。站点按页返回(实测每页 30 条;热门词如 NTR 达 336 页),
/// 页码选择器 select[onchange*="rp('p'"] 的最大值即总页数
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelSearchPage {
    pub hits: Vec<NovelHit>,
    pub page: i64,
    pub total_pages: i64,
}

/// 第 page 页的搜索地址(page<=1 为首页,不带 p 参数)
fn search_url(keyword: &str, page: i64) -> String {
    let encoded: String = url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect();
    if page <= 1 {
        format!("{SEARCH_URL}{encoded}")
    } else {
        format!("{SEARCH_URL}{encoded}&p={page}")
    }
}

pub async fn search_series_page(
    app: &AppHandle,
    keyword: &str,
    page: i64,
) -> Result<NovelSearchPage, String> {
    let page = page.max(1);
    let html = fetch_html(app, &search_url(keyword, page)).await?;
    let hits = extract_series(&html);
    let total_pages = total_pages(&Html::parse_document(&html)).max(page);
    Ok(NovelSearchPage {
        hits,
        page,
        total_pages,
    })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_entries_with_author_and_tags() {
        // 真实样本(系列列表页 2024-11)结构:li.list-group-item 条目含
        // /s/ 标题、/u/ 作者、.tags 标签集合(占位文本代替真实标签词)
        let html = r#"
          <ul class="list-group mt-2">
            <li class="list-group-item">
              <div class="my-1"><a class="text-decoration-none" href="/s/16345198">
                <span class="text-body fw-bold">示例系列标题一</span></a></div>
              <div class="my-1"><a class="me-2" href="/u/74094590">
                <i class="bi bi-person-fill"></i><span class="ms-2 text-body">示例作者甲</span></a>
                <span class="text-sm text-gray">4篇</span></div>
              <div class="my-1 tags"><a href="/tag/标签甲">标签甲</a><span>，</span>
                <a href="/tag/标签乙">标签乙</a><span>，</span><a href="/tag/标签丙">标签丙</a></div>
            </li>
            <li class="list-group-item">
              <div class="my-1"><a href="/s/16345199">
                <span class="text-body fw-bold">示例系列标题二</span></a></div>
            </li>
          </ul>"#;
        let hits = extract_series(html);
        assert_eq!(hits.len(), 2, "条目级提取应命中两条: {hits:?}");
        assert_eq!(hits[0].id, "16345198");
        assert_eq!(hits[0].title, "示例系列标题一");
        assert_eq!(hits[0].author, "示例作者甲");
        assert_eq!(hits[0].tags, ["标签甲", "标签乙", "标签丙"]);
        // 无作者/标签的条目容错(空值回退)
        assert_eq!(hits[1].author, "");
        assert!(hits[1].tags.is_empty());
        assert_eq!(hits[1].title, "示例系列标题二");
    }

    #[test]
    fn builds_paged_search_url() {
        assert_eq!(search_url("NTR", 1), "https://hlib.cc/s/search?query=NTR");
        assert_eq!(search_url("NTR", 0), "https://hlib.cc/s/search?query=NTR");
        assert_eq!(
            search_url("汉服", 3),
            "https://hlib.cc/s/search?query=%E6%B1%89%E6%9C%8D&p=3"
        );
    }

    #[test]
    fn detects_login_page_by_exact_title() {
        assert!(is_login_page(
            "<html><head><title>登录</title></head><body>表单</body></html>"
        ));
        // 正常页面含「登录」按钮文本不误判(title 才是判定面)
        assert!(!is_login_page(
            "<html><head><title>系列 - H图书馆</title></head><body><a>登录</a></body></html>"
        ));
    }

    #[test]
    fn extracts_relative_and_absolute_series_links() {
        let html = r#"
            <a href="/s/relative-1">相对链接系列</a>
            <a href="https://hlib.cc/s/absolute-2?from=search">绝对链接系列</a>
            <a href="https://other.example/s/not-hlib">外站链接</a>
            <a href="/s/relative-1">重复系列</a>
        "#;
        let hits = extract_series(html);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "relative-1");
        assert_eq!(hits[0].title, "相对链接系列");
        assert_eq!(hits[1].id, "absolute-2");
    }
}
