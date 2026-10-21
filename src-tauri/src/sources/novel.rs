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
        let _ = window.show();
        let _ = window.set_focus();
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
    Ok(html)
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

/// 从任意 HTML 提取系列链接(/s/{id} -> 链接文本)。
/// 不依赖具体页面结构,列表/搜索/首页通用。
fn extract_series(html: &str) -> Vec<NovelHit> {
    let doc = Html::parse_document(html);
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
        out.push(NovelHit { id, title });
    }
    out
}

/// 站内搜索:搜索端点结构未知(CF 挡侦察),对常见形态自适应探测,
/// 首个返回有效系列链接的即采用。挑战页返回引导错误。
/// 探测成功的搜索端点模板({} 为关键词占位),进程内记忆避免逐次重试
static SEARCH_ENDPOINT: std::sync::OnceLock<String> = std::sync::OnceLock::new();

const SEARCH_TEMPLATES: [&str; 4] = [
    "https://hlib.cc/search?q={}",
    "https://hlib.cc/search/{}",
    "https://hlib.cc/search?keyword={}",
    "https://hlib.cc/s?kw={}",
];

pub async fn search_series(app: &AppHandle, keyword: &str) -> Result<Vec<NovelHit>, String> {
    use tauri::Manager as _;
    let encoded: String = url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect();
    let manager = app.state::<crate::GatewayManager>();

    // 先读取站点当前搜索表单并在同一 WebView 会话中提交,避免猜测已变更的路径和参数名。
    let mut last_err = match gateway::gateway_search(app, &manager, LABEL, keyword).await {
        Ok(html) if is_challenge(&html) => {
            show_hlib_gateway(app);
            return Err("hlib 安全验证未通过:已在弹出窗口中打开 hlib,请完成验证/登录后重试".into());
        }
        Ok(html) => {
            let hits = extract_series(&html);
            if !hits.is_empty() {
                return Ok(hits);
            }
            "站点搜索表单未返回系列链接".into()
        }
        Err(e) => {
            if e.contains("403") {
                show_hlib_gateway(app);
            }
            e
        }
    };

    // 兼容没有公开搜索表单或表单脚本提交的站点版本。
    let templates: Vec<&str> = match SEARCH_ENDPOINT.get() {
        Some(known) => vec![known.as_str()],
        None => SEARCH_TEMPLATES.to_vec(),
    };
    for tpl in &templates {
        let url = tpl.replace("{}", &encoded);
        match fetch_html(app, &url).await {
            Ok(html) => {
                let hits = extract_series(&html);
                if !hits.is_empty() {
                    if templates.len() > 1 {
                        let _ = SEARCH_ENDPOINT.set(tpl.to_string());
                    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
