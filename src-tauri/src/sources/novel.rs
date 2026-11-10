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
use serde::Deserialize;
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

/// 作者链接 → (作者名, 作者编号)。只认 /u/{纯数字} 的作者页链接
fn author_of(scope: scraper::ElementRef<'_>) -> (String, String) {
    for a in scope.select(&sel("a[href^='/u/']")) {
        let id = a
            .value()
            .attr("href")
            .and_then(|h| h.strip_prefix("/u/"))
            .map(|r| r.split(['/', '?', '#']).next().unwrap_or(""))
            .unwrap_or("");
        if !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) {
            return (
                normalize_text(&a.text().collect::<String>()),
                id.to_string(),
            );
        }
    }
    (String::new(), String::new())
}

/// 解析系列目录一页,返回(标题, 作者, 简介, 本页章节)
fn parse_series_page(html: &str) -> (String, String, String, String, Vec<NovelChapter>) {
    let doc = Html::parse_document(html);
    let title = doc
        .select(&sel("h3.text-center"))
        .next()
        .map(|h| h.text().collect::<String>())
        .unwrap_or_default();
    // 简介:meta description(结构稳定)优先,正文区「简介」li 兜底
    let introduction = doc
        .select(&sel("meta[name=description]"))
        .next()
        .and_then(|m| m.value().attr("content"))
        .map(str::to_string)
        .or_else(|| {
            doc.select(&sel("ul.list-group li.list-group-item"))
                .map(|li| li.text().collect::<String>())
                .find(|t| t.trim() == "简介")
                .and_then(|_| {
                    doc.select(&sel("ul.list-group li.list-group-item"))
                        .map(|li| li.text().collect::<String>())
                        .find(|t| t.trim() != "简介" && !t.trim().is_empty())
                })
        })
        .map(|t| normalize_text(&t))
        .unwrap_or_default();
    let (author, author_id) = author_of(doc.root_element());
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
    (
        normalize_text(&title),
        author,
        author_id,
        introduction,
        chapters,
    )
}

/// 单篇作品包装为一章的系列(标题取正文页 h3,作者取 /u/ 链接)
async fn fetch_single_article(
    app: &AppHandle,
    series_id: &str,
    article_id: &str,
) -> Result<NovelSeries, String> {
    let html = fetch_html(app, &format!("https://hlib.cc/n/{article_id}")).await?;
    if is_challenge(&html) {
        return Err("hlib 安全验证未通过:请在网关窗口完成验证/登录后重试".into());
    }
    let (title, author, author_id) = {
        let doc = Html::parse_document(&html);
        let title = doc
            .select(&sel("h3.text-center"))
            .next()
            .map(|h| normalize_text(&h.text().collect::<String>()))
            .unwrap_or_default();
        let (author, author_id) = author_of(doc.root_element());
        (title, author, author_id)
    };
    if title.is_empty() {
        return Err("未找到作品正文(可能未登录或作品已删除)".into());
    }
    Ok(NovelSeries {
        id: series_id.to_string(),
        title: title.clone(),
        author,
        author_id,
        cover: String::new(),
        introduction: String::new(),
        chapters: vec![NovelChapter {
            id: article_id.to_string(),
            order: 1,
            title,
        }],
        added_at: 0,
    })
}

/// 拉取整个系列目录(分页)入库。调用方负责入库与限速。
pub async fn fetch_series(app: &AppHandle, id: &str) -> Result<NovelSeries, String> {
    // 作者页的单篇作品(不属于任何系列)以 n_{文章编号} 表示:包装成只有一章的系列,
    // 详情页/阅读器无需区分
    if let Some(article) = id.strip_prefix("n_") {
        return fetch_single_article(app, id, article).await;
    }
    let mut all: Vec<NovelChapter> = Vec::new();
    let mut title = String::new();
    let mut author = String::new();
    let mut author_id = String::new();
    let mut introduction = String::new();
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
        let (t, a, aid, introduction_page, chapters) = parse_series_page(&html);
        if title.is_empty() {
            title = t;
            author = a;
            author_id = aid;
            if introduction.is_empty() {
                introduction = introduction_page;
            }
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
        author_id,
        cover: String::new(),
        introduction,
        chapters: all,
        added_at: 0,
    })
}

/// 拉取文章某一页正文
/// 章节抓取限速状态:上次抓取的(文章 id, 页码, 完成时刻)。
/// 站点对连续抓取敏感,节流在抓取入口统一控制(跨实例无需可靠,
/// 单进程内存态即可;重启归零由首次抓取无等待自然覆盖)。
static LAST_ARTICLE_FETCH: std::sync::Mutex<Option<(String, i64, std::time::Instant)>> =
    std::sync::Mutex::new(None);

/// 同章翻页与跨章的最低间隔(秒)——对齐站点反爬敏感度
const ARTICLE_THROTTLE_SAME_CHAPTER: u64 = 3;
const ARTICLE_THROTTLE_CROSS_CHAPTER: u64 = 10;

/// 正文分段提取(B3):遍历 #content 的 <p> 段间用 \n 连接;无 <p>
/// 的页面把 <br> 当换行(逐字符扫描,标签内容跳过)。
fn extract_article_text(content: &scraper::ElementRef) -> String {
    let paragraphs: Vec<String> = content
        .select(&sel("p"))
        .map(|p| normalize_text(&p.text().collect::<String>()))
        .filter(|t| !t.is_empty())
        .collect();
    if !paragraphs.is_empty() {
        return paragraphs.join("\n");
    }
    // 兜底:无 <p> 结构,逐字符扫描,<br> 系列输出换行,其余标签剥除
    let inner = content.inner_html();
    let lower = inner.to_lowercase();
    let orig: Vec<char> = inner.chars().collect();
    let low: Vec<char> = lower.chars().collect();
    let mut out = String::with_capacity(inner.len());
    let mut i = 0;
    let mut in_tag = false;
    while i < orig.len() {
        if in_tag {
            if orig[i] == '>' {
                in_tag = false;
            }
            i += 1;
            continue;
        }
        if orig[i] == '<' {
            // 探测 <br 系列标签(后随 > 空白或 /)
            let rest: String = low[i..(i + 5).min(low.len())].iter().collect();
            if rest.starts_with("<br") {
                let after = low.get(i + 3).copied().unwrap_or('>');
                if matches!(after, '>' | ' ' | '/' | '\t' | '\n' | '\r') {
                    out.push('\n');
                }
            }
            in_tag = true;
            i += 1;
            continue;
        }
        out.push(orig[i]);
        i += 1;
    }
    normalize_text(&out)
}

pub async fn fetch_article_page(app: &AppHandle, id: &str, page: i64) -> Result<NovelPage, String> {
    // 统一节流:与上次抓取比,同章 ≥3 秒、跨章 ≥10 秒,不足则异步等待补齐
    // 计算需等待的秒数(短暂持锁,不跨 await——MutexGuard 非 Send)
    let wait_secs = {
        let last = LAST_ARTICLE_FETCH.lock().expect("article throttle lock");
        let min_secs = if last.as_ref().map(|(cid, _, _)| cid == id).unwrap_or(false) {
            ARTICLE_THROTTLE_SAME_CHAPTER
        } else {
            ARTICLE_THROTTLE_CROSS_CHAPTER
        };
        let elapsed = last
            .as_ref()
            .map(|(_, _, at)| at.elapsed())
            .unwrap_or(std::time::Duration::ZERO);
        if elapsed.as_secs() >= min_secs {
            0u64
        } else {
            min_secs - elapsed.as_secs().max(1)
        }
    };
    if wait_secs > 0 {
        tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
    }
    LAST_ARTICLE_FETCH
        .lock()
        .expect("article throttle lock")
        .replace((id.to_string(), page, std::time::Instant::now()));

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
    // B3 修复:站点正文是多个 <p> 且段间无换行符,.text() 拼接得到
    // 单段(前端按 \n 切段失效、作者行提取失效)——遍历 p 段间用 \n
    let text = extract_article_text(&content);
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
    /// 作者编号(条目级 /u/{数字} 链接;关注作者用)
    #[serde(default)]
    pub author_id: String,
    /// 标签集合(条目级 .tags 容器的 /tag/ 链接文本)
    #[serde(default)]
    pub tags: Vec<String>,
    /// 封面 URL。hlib 站点不提供系列封面(系列页/列表页均无封面元素,
    /// 样本取证确认),恒为空串——字段保留供接口一致性与前端占位逻辑,
    /// 封面图展示走 fetch_image 代理的通用占位
    #[serde(default)]
    pub cover: String,
    /// 发布日期(简介 meta 行「YYYY/MM/DD 首发」正则提取;
    /// 依赖作者发帖格式,格式不符时为空)
    #[serde(default)]
    pub published_date: Option<String>,
    /// 字数(登录态条目「N字」计数 span 提取)
    #[serde(default)]
    pub word_count: Option<String>,
    /// 阅读量(登录态条目 eye-fill 图标配对计数;未登录会话不渲染)
    #[serde(default)]
    pub views: Option<String>,
    /// 收藏量(登录态条目 heart-fill 图标配对计数;未登录会话不渲染)
    #[serde(default)]
    pub favorites: Option<String>,
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
// 需求 15(登录态结构化计数)的提取正则:模块级 OnceLock + helper
// (循环内只调用 helper,编译一次)
static RE_EYE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
static RE_HEART: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
static RE_WORDS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
static RE_DATE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();

fn re_eye() -> &'static regex::Regex {
    RE_EYE.get_or_init(|| {
        regex::Regex::new(r#"bi-eye-fill[^>]*></i><span[^>]*>([^<]+)</span>"#).unwrap()
    })
}
fn re_heart() -> &'static regex::Regex {
    RE_HEART.get_or_init(|| {
        regex::Regex::new(r#"bi-heart-fill[^>]*></i><span[^>]*>([^<]+)</span>"#).unwrap()
    })
}
fn re_words() -> &'static regex::Regex {
    RE_WORDS.get_or_init(|| regex::Regex::new(r#"text-sm text-gray">([\d,]+字)</span>"#).unwrap())
}
fn re_date() -> &'static regex::Regex {
    RE_DATE.get_or_init(|| regex::Regex::new(r"(\d{4}/\d{2}/\d{2})").unwrap())
}

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
            let (author, author_id) = author_of(item);
            let tags: Vec<String> = item
                .select(&sel(".tags a[href^='/tag/']"))
                .map(|a| normalize_text(&a.text().collect::<String>()))
                .filter(|t| !t.is_empty())
                .collect();
            // 需求 15(登录态结构化计数):字数/收藏(heart-fill)/
            // 阅读(eye-fill)——正则模块级 OnceLock(见 extract_series 上方)
            let re_eye = re_eye();
            let re_heart = re_heart();
            let re_words = re_words();
            let re_date = re_date();
            let item_html = item.html();
            let views = re_eye
                .captures(&item_html)
                .map(|c| c[1].trim().to_string())
                .filter(|s| !s.is_empty());
            let favorites = re_heart
                .captures(&item_html)
                .map(|c| c[1].trim().to_string())
                .filter(|s| !s.is_empty());
            let word_count = re_words
                .captures(&item_html)
                .map(|c| c[1].trim().to_string())
                .filter(|s| !s.is_empty());
            // 发布日期:条目简介 meta 行「YYYY/MM/DD 首发」(作者发帖格式,
            // 格式不符时为空)
            let intro_text = normalize_text(&item.text().collect::<String>());
            let published_date = re_date.captures(&intro_text).map(|c| c[1].to_string());
            let final_title = if title.is_empty() { id.clone() } else { title };
            out.push(NovelHit {
                id,
                title: final_title,
                author,
                author_id,
                tags,
                cover: String::new(),
                published_date,
                word_count,
                views,
                favorites,
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
            author_id: String::new(),
            tags: Vec::new(),
            cover: String::new(),
            published_date: None,
            word_count: None,
            views: None,
            favorites: None,
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
    Ok(search_series_page(app, keyword, 1, None).await?.hits)
}

/// 搜索结果的一页。站点按页返回(实测每页 30 条;热门词如 NTR 达 336 页),
/// 页码选择器 select[onchange*="rp('p'"] 的最大值即总页数
/// 发现流(最新系列)的一页。站点首页/深页统一 ?p= 页码机制,
/// 总页数取页码控件(select[onchange*="rp('p'"])的最大值
#[derive(Deserialize, Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelLatestPage {
    pub hits: Vec<NovelHit>,
    pub page: i64,
    pub total_pages: i64,
}

/// 搜索结果的一页。站点按页返回(实测每页 30 条;热门词如 NTR 达 336 页),
/// 页码选择器 select[onchange*="rp('p'"] 的最大值即总页数
#[derive(Deserialize, Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelSearchPage {
    pub hits: Vec<NovelHit>,
    pub page: i64,
    pub total_pages: i64,
}

/// 第 page 页的搜索地址(page<=1 为首页,不带 p 参数;sort 可选排序)
fn search_url(keyword: &str, page: i64, sort: Option<&str>) -> String {
    let encoded: String = url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect();
    let sort_seg = valid_sort(sort)
        .map(|s| format!("&sort={s}"))
        .unwrap_or_default();
    if page <= 1 {
        format!("{SEARCH_URL}{encoded}{sort_seg}")
    } else {
        format!("{SEARCH_URL}{encoded}&p={page}{sort_seg}")
    }
}

/// 榜单端点(系列排行榜;/rank/s 需登录,登录后会话 cookie 可用)
const RANK_URL: &str = "https://hlib.cc/rank/s";

/// 搜索页的合法排序值(列表页 rp('sort', …) 五个按钮:关联度/时间/篇数/字数/收藏)。
/// 不传 sort = 网页端搜索框提交的默认排序(表单只带 query);非法值忽略
fn valid_sort(sort: Option<&str>) -> Option<&str> {
    sort.filter(|s| matches!(*s, "score" | "date" | "total" | "char" | "like"))
}

pub async fn search_series_page(
    app: &AppHandle,
    keyword: &str,
    page: i64,
    sort: Option<&str>,
) -> Result<NovelSearchPage, String> {
    let page = page.max(1);
    let html = fetch_html(app, &search_url(keyword, page, sort)).await?;
    let hits = extract_series(&html);
    let total_pages = total_pages(&Html::parse_document(&html)).max(page);
    Ok(NovelSearchPage {
        hits,
        page,
        total_pages,
    })
}

/// 作者页作品流:提取 /u/{author} 页面的 /n/ 单篇列表。
/// 侦察取证(recon_2):作者页 title 含作者名,作品为 /n/ 链接集合,
/// 无需登录即可访问。
pub async fn author_page_works(app: &AppHandle, author_id: &str) -> Result<Vec<NovelHit>, String> {
    let url = format!("https://hlib.cc/u/{author_id}");
    let html = fetch_html(app, &url).await?;
    if is_challenge(&html) {
        return Err("hlib 安全验证未通过:请先在网关窗口完成验证/登录".into());
    }
    // 作者页若以系列条目(li + /s/ 链接)列出作品,按系列返回(可进系列详情);
    // 否则退回 /n/ 单篇(n_ 前缀,fetch_series 包装为单章系列)
    let series = extract_series(&html);
    if !series.is_empty() {
        return Ok(series);
    }
    let doc = Html::parse_document(&html);
    let (author, _) = author_of(doc.root_element());
    // /n/ 链接文本即作品标题;去重保持出现顺序
    let mut out: Vec<NovelHit> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for a in doc.select(&sel("a[href^='/n/']")) {
        let href = a.value().attr("href").unwrap_or_default();
        let id = href
            .trim_start_matches("/n/")
            .split('?')
            .next()
            .unwrap_or("");
        if id.is_empty() || !seen.insert(id.to_string()) {
            continue;
        }
        let title = normalize_text(&a.text().collect::<String>());
        if title.is_empty() {
            continue;
        }
        out.push(NovelHit {
            id: format!("n_{id}"),
            title,
            author: author.clone(),
            author_id: author_id.to_string(),
            tags: Vec::new(),
            cover: String::new(),
            published_date: None,
            word_count: None,
            views: None,
            favorites: None,
        });
    }
    if out.is_empty() {
        return Err("作者页未解析到作品链接".into());
    }
    Ok(out)
}

/// 榜单周期
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RankPeriod {
    Day,
    Week,
    Month,
}

impl RankPeriod {
    pub fn parse(v: &str) -> Self {
        match v {
            "week" | "weekly" => Self::Week,
            "month" | "monthly" => Self::Month,
            _ => Self::Day,
        }
    }

    /// 站点切换控件文案的判定:控件多为「日榜/周榜/月榜」「今日/本周/本月」「24小时/7天/30天」
    fn matches_label(self, text: &str) -> bool {
        let t = text.trim();
        if t.is_empty() || t.chars().count() > 8 {
            return false;
        }
        match self {
            Self::Day => {
                t.contains('日') || t.contains("24") || t.contains("今天") || t.contains("1天")
            }
            Self::Week => {
                t.contains('周') || t.contains('週') || t.contains("7天") || t.contains("七天")
            }
            Self::Month => t.contains('月') || t.contains("30天") || t.contains("三十天"),
        }
    }
}

/// 榜单页上「切到某周期」的控件指向:直接链接,或 rp('键','值') 形态的参数
#[derive(Clone, Debug, PartialEq)]
enum RankSwitch {
    Href(String),
    Param(String, String),
}

fn re_rp() -> &'static regex::Regex {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(r#"rp\(\s*['"]([A-Za-z_]\w*)['"]\s*,\s*['"]?([^'")\s]+)['"]?\s*\)"#)
            .expect("static regex")
    })
}

/// 从榜单页发现周期切换控件(站点列表页统一用 rp(键,值) 改查询参数;
/// 也兼容普通链接与 <select onchange="rp('键', this.value)"> 下拉)。
/// 榜单参数形态未在登录态取证,故按控件文案自适应识别,不写死参数名。
fn discover_rank_switch(html: &str, period: RankPeriod) -> Option<RankSwitch> {
    let doc = Html::parse_document(html);
    for el in doc.select(&sel("a, button, li, span, label")) {
        let text = normalize_text(&el.text().collect::<String>());
        if !period.matches_label(&text) {
            continue;
        }
        let v = el.value();
        if let Some(onclick) = v.attr("onclick") {
            if let Some(c) = re_rp().captures(onclick) {
                if &c[1] != "p" {
                    return Some(RankSwitch::Param(c[1].to_string(), c[2].to_string()));
                }
            }
        }
        if let Some(href) = v.attr("href") {
            if let Some(c) = re_rp().captures(href) {
                if &c[1] != "p" {
                    return Some(RankSwitch::Param(c[1].to_string(), c[2].to_string()));
                }
            } else if href.contains("/rank") && !href.contains("p=") {
                return Some(RankSwitch::Href(href.to_string()));
            }
        }
    }
    let re_select_key = regex::Regex::new(r#"rp\(\s*['"]([A-Za-z_]\w*)['"]\s*,\s*this\.value"#)
        .expect("static regex");
    for select in doc.select(&sel("select[onchange]")) {
        let onchange = select.value().attr("onchange").unwrap_or("");
        let Some(key) = re_select_key.captures(onchange).map(|c| c[1].to_string()) else {
            continue;
        };
        if key == "p" {
            continue;
        }
        for opt in select.select(&sel("option")) {
            let text = normalize_text(&opt.text().collect::<String>());
            if period.matches_label(&text) {
                if let Some(val) = opt.value().attr("value") {
                    return Some(RankSwitch::Param(key, val.to_string()));
                }
            }
        }
    }
    None
}

/// 只认页码下拉(rp('p', …))的最大值:榜单页可能还有周期下拉(值如 1/7/30),
/// 通用 total_pages 会把它误当页数
fn rank_total_pages(doc: &Html) -> i64 {
    let mut max = 1i64;
    for opt in doc.select(&sel("select[onchange*=\"'p'\"] option")) {
        if let Ok(v) = opt.value().attr("value").unwrap_or("").parse::<i64>() {
            max = max.max(v);
        }
    }
    max
}

/// 在地址上设置查询参数(已有同名参数则替换)
fn with_query(base: &str, key: &str, value: &str) -> String {
    let Ok(mut url) = url::Url::parse(base) else {
        return base.to_string();
    };
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(k, _)| k != key)
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();
    url.query_pairs_mut()
        .clear()
        .extend_pairs(pairs)
        .append_pair(key, value);
    url.to_string()
}

/// 周期 → 已发现的榜单基础地址(进程内记忆,避免每次先抓默认页再切换)
static RANK_BASES: std::sync::Mutex<Vec<(RankPeriod, String)>> = std::sync::Mutex::new(Vec::new());

fn rank_page_url(base: &str, page: i64) -> String {
    if page <= 1 {
        base.to_string()
    } else {
        with_query(base, "p", &page.to_string())
    }
}

/// 系列排行榜(/rank/s,需登录;未登录返回登录页由调用方引导)。
/// 先抓默认榜单页,从页面的切换控件找到所求周期的地址再抓;找不到控件时
/// 日榜按默认页返回,周/月榜如实报错(不猜参数名去撞站点反爬闸)。
pub async fn rank_series(
    app: &AppHandle,
    period: RankPeriod,
    page: i64,
) -> Result<NovelLatestPage, String> {
    let page = page.max(1);
    let remembered = RANK_BASES
        .lock()
        .ok()
        .and_then(|v| v.iter().find(|(p, _)| *p == period).map(|(_, u)| u.clone()));
    let (_base, html) = match remembered {
        Some(base) => {
            let html = fetch_rank_html(app, &rank_page_url(&base, page)).await?;
            (base, html)
        }
        None => {
            let default_html = fetch_rank_html(app, RANK_URL).await?;
            let base = match discover_rank_switch(&default_html, period) {
                Some(RankSwitch::Href(href)) => url::Url::parse(RANK_URL)
                    .and_then(|b| b.join(&href))
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| RANK_URL.to_string()),
                Some(RankSwitch::Param(k, v)) => with_query(RANK_URL, &k, &v),
                None if period == RankPeriod::Day => RANK_URL.to_string(),
                None => {
                    return Err(match period {
                        RankPeriod::Month => "站点榜单页未找到月榜切换,暂时无法获取月榜".into(),
                        _ => "站点榜单页未找到周榜切换,暂时无法获取周榜".into(),
                    })
                }
            };
            if let Ok(mut v) = RANK_BASES.lock() {
                v.push((period, base.clone()));
            }
            let html = if base == RANK_URL && page <= 1 {
                default_html
            } else {
                fetch_rank_html(app, &rank_page_url(&base, page)).await?
            };
            (base, html)
        }
    };
    let hits = extract_series(&html);
    let total_pages = rank_total_pages(&Html::parse_document(&html)).max(page);
    if hits.is_empty() && page <= 1 {
        return Err("排行榜未解析到系列链接".into());
    }
    Ok(NovelLatestPage {
        hits,
        page,
        total_pages,
    })
}

async fn fetch_rank_html(app: &AppHandle, url: &str) -> Result<String, String> {
    let html = fetch_html(app, url).await?;
    if is_challenge(&html) {
        return Err("hlib 安全验证未通过:请先在网关窗口完成验证/登录".into());
    }
    Ok(html)
}

/// 发现:首页提取最新系列(不依赖具体列表结构)
pub async fn latest_series(app: &AppHandle, page: i64) -> Result<NovelLatestPage, String> {
    let page = page.max(1);
    // 发现流:首页(第 1 页不带参数),深页走全站统一的 ?p= 页码机制
    let url = if page <= 1 {
        "https://hlib.cc/".to_string()
    } else {
        format!("https://hlib.cc/?p={page}")
    };
    let html = fetch_html(app, &url).await?;
    if is_challenge(&html) {
        return Err("hlib 安全验证未通过:请先在网关窗口完成验证/登录".into());
    }
    let hits = extract_series(&html);
    if hits.is_empty() {
        return Err("首页未解析到系列链接".into());
    }
    let total_pages = total_pages(&Html::parse_document(&html)).max(page);
    Ok(NovelLatestPage {
        hits,
        page,
        total_pages,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovers_rank_period_switches() {
        // rp(键,值) 按钮形态(站点列表页同款脚本)
        let html = r#"<div><button onclick="rp('t','d')">日榜</button>
            <button onclick="rp('t', 'w')">周榜</button>
            <button onclick="rp('t','m')">月榜</button>
            <select onchange="rp('p', this.value)"><option value="1">1</option><option value="5">5</option></select></div>"#;
        assert_eq!(
            discover_rank_switch(html, RankPeriod::Month),
            Some(RankSwitch::Param("t".into(), "m".into()))
        );
        assert_eq!(
            discover_rank_switch(html, RankPeriod::Day),
            Some(RankSwitch::Param("t".into(), "d".into()))
        );
        // 下拉形态
        let html = r#"<select onchange="rp('range', this.value)"><option value="1">今日</option>
            <option value="30">30天</option></select>"#;
        assert_eq!(
            discover_rank_switch(html, RankPeriod::Month),
            Some(RankSwitch::Param("range".into(), "30".into()))
        );
        // 普通链接形态
        let html = r#"<a href="/rank/s?type=month">本月</a><a href="/rank/s?p=2">2</a>"#;
        assert_eq!(
            discover_rank_switch(html, RankPeriod::Month),
            Some(RankSwitch::Href("/rank/s?type=month".into()))
        );
        // 无控件
        assert_eq!(discover_rank_switch("<p>无</p>", RankPeriod::Month), None);
        // 页码只认 rp('p') 下拉,周期下拉的 30 不算页数
        let doc = Html::parse_document(
            r#"<select onchange="rp('range', this.value)"><option value="30">30天</option></select>
               <select onchange="rp('p', this.value)"><option value="3">3</option></select>"#,
        );
        assert_eq!(rank_total_pages(&doc), 3);
        assert_eq!(
            with_query("https://hlib.cc/rank/s?t=m", "p", "2"),
            "https://hlib.cc/rank/s?t=m&p=2"
        );
    }

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
        assert_eq!(
            search_url("NTR", 1, None),
            "https://hlib.cc/s/search?query=NTR"
        );
        assert_eq!(
            search_url("NTR", 0, None),
            "https://hlib.cc/s/search?query=NTR"
        );
        assert_eq!(
            search_url("汉服", 3, None),
            "https://hlib.cc/s/search?query=%E6%B1%89%E6%9C%8D&p=3"
        );
    }

    #[test]
    fn builds_sorted_search_url() {
        // 合法排序值直通;非法值忽略(列表页 rp() 四维之外不透传)
        assert_eq!(
            search_url("NTR", 1, Some("date")),
            "https://hlib.cc/s/search?query=NTR&sort=date"
        );
        assert_eq!(
            search_url("NTR", 2, Some("like")),
            "https://hlib.cc/s/search?query=NTR&p=2&sort=like"
        );
        assert_eq!(
            search_url("NTR", 1, Some("bogus")),
            "https://hlib.cc/s/search?query=NTR",
            "非法排序值应被忽略"
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
    fn article_text_splits_paragraphs_and_recovers_author() {
        // 真实样本结构(第一章样本):#content 下多个 <p>,段间无换行符;
        // .text() 拼接得到单段导致阅读器分段与作者行提取双双失效
        let html = r#"<div id="content">
            <p>作者：示例作者名</p>
            <p>第一段占位文本,描述故事的开端。</p>
            <p>第二段占位文本,推进情节发展。</p>
        </div>"#;
        let doc = Html::parse_document(html);
        let content = doc.select(&sel("#content")).next().unwrap();
        let text = extract_article_text(&content);
        // 段间有换行(分段生效)
        assert!(text.contains('\n'), "段间应有换行符");
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() >= 3, "应至少 3 段,实际 {}", lines.len());
        // 作者行提取恢复生效(分段后 lines() 能拿到首行)
        let author = text
            .lines()
            .find_map(|l| l.strip_prefix("作者：").or_else(|| l.strip_prefix("作者:")))
            .map(|s| s.trim().to_string());
        assert_eq!(author.as_deref(), Some("示例作者名"));
    }

    #[test]
    fn article_text_br_fallback() {
        // 无 <p> 结构的页面:<br> 当换行兜底
        let html = r#"<div id="content">第一行占位<br>第二行占位<br/>第三行占位</div>"#;
        let doc = Html::parse_document(html);
        let content = doc.select(&sel("#content")).next().unwrap();
        let text = extract_article_text(&content);
        let lines: Vec<&str> = text.lines().collect();
        assert!(lines.len() >= 3, "br 应转为换行,实际 {}", lines.len());
    }

    #[test]
    fn parses_series_introduction_from_meta_description() {
        // 真实样本(系列页 /s/1457728)结构:meta description 与「简介」li
        // 并存,meta 结构稳定作为优先来源
        let html = r#"
          <html><head>
            <meta name="description" content="示例简介文本——来自 meta description">
            <title>示例系列 - H图书馆</title>
          </head><body>
            <h3 class="text-center">示例系列标题</h3>
            <a href="/u/123">示例作者</a>
            <ul class="list-group mt-2">
              <li class="list-group-item">简介</li>
              <li class="list-group-item">示例简介文本——来自简介 li</li>
            </ul>
            <a href="/n/10001"><span>#1 示例章节</span></a>
          </body></html>"#;
        let (title, author, author_id, introduction, chapters) = parse_series_page(html);
        assert_eq!(title, "示例系列标题");
        assert_eq!(author, "示例作者");
        assert_eq!(author_id, "123");
        assert_eq!(
            introduction, "示例简介文本——来自 meta description",
            "简介应取 meta description(优先于简介 li)"
        );
        assert_eq!(chapters.len(), 1);
        assert_eq!(chapters[0].order, 1);
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

    #[test]
    fn author_id_only_from_numeric_user_links() {
        // 导航栏的 /u/n/{id}(作者小说列表)等非作者页链接不算;取首个 /u/{纯数字}
        let html = r#"<div><a href="/u/n/111">列表</a><a href="/u/36084587?x=1"> 甲作者 </a><a href="/u/999">乙</a></div>"#;
        let doc = Html::parse_document(html);
        let (name, id) = author_of(doc.root_element());
        assert_eq!((name.as_str(), id.as_str()), ("甲作者", "36084587"));
        let none = Html::parse_document(r#"<a href="/u/abc">x</a>"#);
        assert_eq!(
            author_of(none.root_element()),
            (String::new(), String::new())
        );
    }

    #[test]
    fn search_hits_carry_author_id_and_sort_whitelist() {
        let html = r#"<ul><li class="list-group-item"><a href="/s/123">系列甲</a>
            <a href="/u/74094590">作者甲</a></li></ul>"#;
        let hits = extract_series(html);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].author, "作者甲");
        assert_eq!(hits[0].author_id, "74094590");
        // 网页端五个排序按钮均可透传;未指定时不带 sort(等同网页搜索框默认)
        assert!(search_url("k", 1, Some("score")).ends_with("&sort=score"));
        assert!(!search_url("k", 1, None).contains("sort="));
        assert!(!search_url("k", 1, Some("bogus")).contains("sort="));
    }
}
