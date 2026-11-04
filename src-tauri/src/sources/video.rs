//! 视频板块:fs1.app(Jable 系,服务端渲染,reqwest 直连可达)。
//! 列表/搜索/详情解析;m3u8 直接内嵌在详情页 HTML。
//! missav.ai 按 TLS 指纹拦截通用客户端(403):用 wreq 模拟 Chrome 指纹
//! 直连(搜索/兜底播放地址均不再经 WebView 网关);播放地址在详情页的
//! Dean Edwards packer 混淆脚本内,由 unpack_packed 解码提取。

use crate::models::*;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use scraper::{Html, Selector};
use std::{sync::OnceLock, time::Duration};

const BASE: &str = "https://fs1.app";

fn absolute_url(value: &str) -> String {
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

fn sel(value: &str) -> Selector {
    Selector::parse(value).expect("static selector")
}

/// 提取 meta-refresh 跳转地址并解析为绝对 URL(无则返回 None)。
/// missav 详情路径会先返回形如
/// `<meta http-equiv="refresh" content="0;url='https://missav.ai/dm82/…'">` 的跳转页
fn meta_refresh_url(html: &str, base_url: &str) -> Option<String> {
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
fn unpack_packed(block: &str) -> Option<String> {
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
    if radix < 2 || radix > 62 {
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
fn extract_packed_m3u8(html: &str) -> Option<String> {
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

/// JS Number.toString(radix) 同款进制表示(0-9a-zA-Z,radix ≤ 62)
fn to_base(mut n: u32, radix: u32) -> String {
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

/// 视频卡片项(列表/搜索共用)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoCard {
    /// 番号,如 "mida-829"(小写),同时作为详情 id;
    /// 中字变体的 code 为 "mida-829-chinese-subtitle"
    pub code: String,
    pub title: String,
    pub thumbnail: String,
    /// 观看数(原始文本,如 "315 903")
    pub views: String,
    /// 点赞数
    pub likes: String,
    /// 是否中文字幕变体
    pub chinese: bool,
}

/// 基础番号:去掉 -chinese-subtitle 后缀
fn base_code(code: &str) -> &str {
    code.strip_suffix("-chinese-subtitle").unwrap_or(code)
}

/// 多源合并后的 has_next:任一源本页还有货即 true。
/// fs1 按满页判定(每页 24 卡);missav dm539 每页仅 12 卡且末页可能
/// 不满,按「本页非空」判定——末页有余卡时会多一次空翻页,可接受。
fn merged_has_next(fs1_full_page: bool, missav_nonempty: bool) -> bool {
    fs1_full_page || missav_nonempty
}

/// fourhoi 图床判定(missav 卡片封面的图床)。其防盗链仅放行白名单
/// Referer(missav.ai)与空 Referer,fs1 等其他站 Referer 一律 403——
/// 封面请求对它必须不带 Referer(实测空 Referer 最宽松,不依赖白名单)。
pub fn is_fourhoi_url(url: &str) -> bool {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| h == "fourhoi.com" || h.ends_with(".fourhoi.com"))
        })
        .unwrap_or(false)
}

/// 视频列表响应
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoListResult {
    pub items: Vec<VideoCard>,
    pub pagination: PaginationInfo,
}

pub struct VideoApi {
    pub client: reqwest::Client,
    /// missav 专用:Chrome TLS 指纹(wreq 默认不跟随重定向,构建时已设跟随)
    pub missav: wreq::Client,
}

fn selectors() -> &'static (Selector, Selector, Selector, Selector) {
    static S: OnceLock<(Selector, Selector, Selector, Selector)> = OnceLock::new();
    S.get_or_init(|| {
        (
            sel("div.video-img-box"),
            sel("a[href*='/videos/']"),
            sel("img"),
            sel("p.sub-title"),
        )
    })
}

impl VideoApi {
    pub fn new() -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Linux; Android 13) AppleWebKit/537.36 Chrome/120 Mobile Safari/537.36",
            ),
        );
        headers.insert(REFERER, HeaderValue::from_static(BASE));
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        let missav = wreq::Client::builder()
            .emulation(wreq_util::Emulation::Chrome131)
            .redirect(wreq::redirect::Policy::limited(5))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self { client, missav })
    }

    async fn get(&self, path: &str) -> Result<String, String> {
        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            format!("{BASE}{path}")
        };
        self.client
            .get(url)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .error_for_status()
            .map_err(|e| e.to_string())?
            .text()
            .await
            .map_err(|e| e.to_string())
    }

    /// missav 直连 GET:HTTP 重定向由客户端跟随,HTML meta-refresh
    /// (dm 前缀轮换产生的跳转页)在此手动跟进,返回最终 HTML
    async fn missav_get(&self, url: &str) -> Result<String, String> {
        let mut current = url.to_string();
        for _hop in 0..3 {
            let resp = self
                .missav
                .get(&current)
                .header(REFERER, "https://missav.ai/")
                .send()
                .await
                .map_err(|e| e.to_string())?;
            let status = resp.status();
            let body = resp.text().await.map_err(|e| e.to_string())?;
            if !status.is_success() {
                return Err(format!("HTTP {status}"));
            }
            match meta_refresh_url(&body, &current) {
                Some(next) => current = next,
                None => return Ok(body),
            }
        }
        Err("missav 重定向次数过多".into())
    }

    /// missav 搜索(中文词兜底):dm539 搜索页为服务端渲染,直连可达。
    /// 返回结果卡片与「本页有货」标志(dm539 每页仅 12 卡,不能沿用
    /// fs1 的每页卡数阈值推断下一页)。
    async fn search_missav(
        &self,
        keyword: &str,
        page: i32,
    ) -> Result<(Vec<VideoCard>, bool), String> {
        let encoded = url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect::<String>();
        let html = self
            .missav_get(&format!(
                "https://missav.ai/dm539/search/{encoded}?page={page}"
            ))
            .await?;
        let mut cards = Self::parse_missav_search(&html);
        // hydration DOM 形态不可预知:主解析为空时全文番号扫描兜底
        if cards.is_empty() {
            cards = Self::scan_codes_fallback(&html);
        }
        let has_next = !cards.is_empty();
        Ok((cards, has_next))
    }

    /// missav 兜底播放地址:详情页的 packer 混淆脚本解码出 m3u8。
    /// 优先中文字幕变体,不存在回退原番号。
    async fn missav_m3u8(&self, code: &str) -> Result<(String, bool), String> {
        let lower = code.to_lowercase();
        for (path, chinese) in [(format!("{lower}-chinese-subtitle"), true), (lower, false)] {
            let url = format!("https://missav.ai/videos/{path}");
            let Ok(html) = self.missav_get(&url).await else {
                continue;
            };
            if let Some(m3u8) = extract_packed_m3u8(&html) {
                return Ok((m3u8, chinese));
            }
        }
        Err("missav 详情页未找到播放地址".into())
    }

    /// 解析列表/搜索页的卡片网格
    fn parse_cards(html: &str) -> Vec<VideoCard> {
        let doc = Html::parse_document(html);
        let (card_sel, link_sel, img_sel, sub_sel) = selectors();
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for card in doc.select(card_sel) {
            let Some(link) = card.select(link_sel).find_map(|a| {
                let href = a.value().attr("href")?;
                // 从 /videos/{code}/ 提取番号
                let code = href
                    .split("/videos/")
                    .nth(1)?
                    .trim_matches('/')
                    .to_lowercase();
                if code.is_empty() {
                    None
                } else {
                    Some((code, href.to_string()))
                }
            }) else {
                continue;
            };
            if !seen.insert(link.0.clone()) {
                continue;
            }
            // Jable 卡片标题在 h6(或 .title)内,文本含"番号 完整标题"
            let title = card
                .select(&sel("h6, .title"))
                .next()
                .map(|h| h.text().collect::<String>().trim().to_string())
                .unwrap_or_default();
            let thumbnail = card
                .select(img_sel)
                .next()
                .and_then(|img| {
                    img.value()
                        .attr("data-src")
                        .or_else(|| img.value().attr("src"))
                })
                .map(absolute_url)
                .unwrap_or_default();
            // sub-title 形如 "315 903 ❤ 1403"(svg 图标分隔的观看数与点赞数)
            let (views, likes) = card
                .select(sub_sel)
                .next()
                .map(|p| {
                    let nums: Vec<String> = p
                        .text()
                        .map(|t| t.trim().to_string())
                        .filter(|t| !t.is_empty())
                        .collect();
                    (
                        nums.first().cloned().unwrap_or_default(),
                        nums.get(1).cloned().unwrap_or_default(),
                    )
                })
                .unwrap_or_default();
            let code = link.0;
            let chinese = code.ends_with("-chinese-subtitle");
            out.push(VideoCard {
                code,
                title,
                thumbnail,
                views,
                likes,
                chinese,
            });
        }
        // 同番号的普通版与中字版只留一张,优先中字
        let mut dedup: Vec<VideoCard> = Vec::with_capacity(out.len());
        for card in out {
            let base = base_code(&card.code);
            if let Some(existing) = dedup.iter_mut().find(|c| base_code(&c.code) == base) {
                if card.chinese && !existing.chinese {
                    *existing = card;
                }
            } else {
                dedup.push(card);
            }
        }
        dedup
    }

    /// 兜底:番号形态 href 扫描(hydration 后 DOM 结构不可预知时,
    /// 番号链接仍以 href 属性形态存在)。与主解析同边界——只扫 href
    /// 属性值,避免把页面 CSS 工具类(red-500/gray-300 等)误当番号;
    /// 封面按 fourhoi 规律构造。
    fn scan_codes_fallback(html: &str) -> Vec<VideoCard> {
        let re_href = Regex::new(r#"href="([^"]+)""#).expect("static regex");
        let re_code = Regex::new(r"\b([a-z]{2,6}-\d{2,5})\b").expect("static regex");
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for cap in re_href.captures_iter(html) {
            let href = cap.get(1).map(|m| m.as_str()).unwrap_or_default();
            for m in re_code.captures_iter(href) {
                let Some(m) = m.get(1) else { continue };
                let code = m.as_str().to_lowercase();
                if seen.insert(code.clone()) {
                    out.push(VideoCard {
                        code: code.clone(),
                        title: code.to_uppercase(),
                        thumbnail: format!("https://fourhoi.com/{code}/cover-t.jpg"),
                        views: String::new(),
                        likes: String::new(),
                        chinese: false,
                    });
                }
            }
        }
        out
    }

    /// 最新上市列表,page 从 1 开始
    pub async fn list(&self, page: i32) -> Result<VideoListResult, String> {
        let html = self.get(&format!("/new-release/{page}/")).await?;
        let items = Self::parse_cards(&html);
        Ok(VideoListResult {
            pagination: PaginationInfo {
                current: page - 1,
                total: 0, // Jable 不给总页数,靠 hasNext 翻页
                has_next: items.len() >= 24,
                has_prev: page > 1,
            },
            items,
        })
    }

    /// 按关键词搜索:fs1 与 missav(直连)并发请求并合并,同基础番号
    /// 以 fs1 为准(fs1 有完整元数据);两路均失败时返回错误。
    pub async fn search(&self, keyword: &str, page: i32) -> Result<VideoListResult, String> {
        let (fs1_res, missav_res) = tokio::join!(self.search_fs1(keyword, page), async {
            self.search_missav(keyword, page)
                .await
                .unwrap_or((Vec::new(), false))
        });
        let fs1_has_next = matches!(&fs1_res, Ok(r) if r.items.len() >= 24);
        let (missav, missav_has_next) = missav_res;
        let mut items = match fs1_res {
            Ok(r) => r.items,
            Err(e) => {
                if missav.is_empty() {
                    return Err(e);
                }
                Vec::new()
            }
        };
        for card in missav {
            let base = base_code(&card.code);
            if items.iter().any(|c| base_code(&c.code) == base) {
                continue;
            }
            items.push(card);
        }
        Ok(VideoListResult {
            pagination: PaginationInfo {
                current: page - 1,
                total: 0,
                has_next: merged_has_next(fs1_has_next, missav_has_next),
                has_prev: page > 1,
            },
            items,
        })
    }

    /// fs1 侧搜索(番号/日文词;中文词在 fs1 索引中常无结果)
    async fn search_fs1(&self, keyword: &str, page: i32) -> Result<VideoListResult, String> {
        let encoded: String = url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect();
        let html = self.get(&format!("/search/{encoded}/{page}/")).await?;
        let items = Self::parse_cards(&html);
        Ok(VideoListResult {
            pagination: PaginationInfo {
                current: page - 1,
                total: 0,
                has_next: !items.is_empty(),
                has_prev: page > 1,
            },
            items,
        })
    }

    /// 解析 missav 搜索结果页:通用 href 提取(任意形态路径,join 后取
    /// 末段判定番号;SSR 与 Vue hydration 后的 DOM 均适用) +
    /// fourhoi.com/{code}/cover-t.jpg 规律封面 + 番号前缀归属标题
    fn parse_missav_search(html: &str) -> Vec<VideoCard> {
        let re_href = Regex::new(r#"href="([^"]+)""#).expect("static regex");
        let is_video_code = |code: &str| {
            Regex::new(r"^([a-z]{2,6}-\d{2,5}(-chinese-subtitle)?|fc2-ppv-\d+)$")
                .unwrap()
                .is_match(code)
        };
        let anchors: Vec<(usize, String)> = re_href
            .captures_iter(html)
            .filter_map(|c| {
                let href = c.get(1)?.as_str();
                // 相对/绝对/带语言前缀统一 join 后取末段
                let joined = url::Url::parse("https://missav.ai/")
                    .unwrap()
                    .join(href)
                    .ok()?;
                let seg = joined.path().trim_matches('/');
                let code = seg.split('/').next_back()?.to_lowercase();
                if !is_video_code(&code) {
                    return None;
                }
                Some((c.get(0)?.start(), code))
            })
            .collect();

        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        for (_index, (pos, code)) in anchors.iter().enumerate() {
            if !seen.insert(code.clone()) {
                continue;
            }
            // 标题窗口:锚点后 9000 字符内找「番号大写 」开头的文本节点
            let upper = code.to_uppercase();
            let re_t = Regex::new(&format!(
                r#">\s*{}\s([^<>]{{5,200}})\s*<"#,
                regex::escape(&upper)
            ))
            .expect("static regex");
            // 窗口终点按 UTF-8 字符边界右移(zh-Hant 页面多字节字符会落进窗口)
            let mut end = (*pos + 9000).min(html.len());
            while end < html.len() && !html.is_char_boundary(end) {
                end += 1;
            }
            let window = &html[*pos..end];
            let title = re_t
                .captures(window)
                .map(|c| {
                    c.get(1)
                        .map(|t| {
                            t.as_str()
                                .split(" - ")
                                .next()
                                .unwrap_or("")
                                .trim()
                                .to_string()
                        })
                        .unwrap_or_default()
                })
                .unwrap_or_default();
            out.push(VideoCard {
                code: code.clone(),
                title: if title.is_empty() {
                    code.to_uppercase()
                } else {
                    title
                },
                thumbnail: format!("https://fourhoi.com/{code}/cover-t.jpg"),
                views: String::new(),
                likes: String::new(),
                chinese: code.ends_with("-chinese-subtitle"),
            });
        }
        out
    }

    /// fs1 侧详情:元数据 + fs1 的 m3u8。code 为小写番号。
    /// 优先尝试中文字幕变体(/videos/{code}-chinese-subtitle/),不存在回退原路径
    async fn detail_fs1(&self, code: &str) -> Result<VideoDetail, String> {
        let code = code.to_lowercase();
        let plain = self.detail_fs1_path(&format!("{code}/"), &code).await;
        match self
            .detail_fs1_path(&format!("{code}-chinese-subtitle/"), &code)
            .await
        {
            Ok(mut cs) => {
                cs.chinese = true;
                Ok(cs)
            }
            Err(_) => plain,
        }
    }

    async fn detail_fs1_path(&self, path: &str, code: &str) -> Result<VideoDetail, String> {
        let html = self.get(&format!("/videos/{path}")).await?;
        let doc = Html::parse_document(&html);
        let title = doc
            .select(&sel("h4"))
            .next()
            .map(|h| h.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let tags: Vec<String> = doc
            .select(&sel("a.tag"))
            .map(|a| a.text().collect::<String>().trim().to_string())
            .filter(|t| !t.is_empty())
            .collect();
        // m3u8 内嵌在 hlsUrl/jwplayer 配置中
        let m3u8 = doc
            .html()
            .split("https://")
            .skip(1)
            .filter_map(|seg| {
                let end = seg.find(|c: char| c == '"' || c == '\'' || c.is_whitespace())?;
                let url = format!("https://{}", &seg[..end]);
                let is_playlist = url::Url::parse(&url)
                    .ok()
                    .is_some_and(|parsed| parsed.path().ends_with(".m3u8"));
                if is_playlist && url.contains("/hls/") {
                    Some(url)
                } else {
                    None
                }
            })
            .next();
        Ok(VideoDetail {
            code: code.to_string(),
            title,
            tags,
            m3u8: m3u8.ok_or_else(|| "未在详情页找到 m3u8".to_string())?,
            provider: "fs1".into(),
            chinese: false,
        })
    }

    /// 优先使用已验证可直连的 fs1 播放地址,失败后再以 missav 兜底。
    /// 两端均优先尝试中文字幕版本。
    pub async fn detail(&self, code: &str) -> Result<VideoDetail, String> {
        match self.detail_fs1(code).await {
            Ok(detail) => Ok(detail),
            Err(fs1_error) => match self.missav_m3u8(code).await {
                Ok((m3u8, chinese)) => Ok(VideoDetail {
                    code: code.to_lowercase(),
                    title: code.to_uppercase(),
                    tags: vec![],
                    m3u8,
                    provider: "missav".into(),
                    chinese,
                }),
                Err(missav_error) => Err(format!(
                    "fs1 播放地址不可用({fs1_error}); missav 兜底失败({missav_error})"
                )),
            },
        }
    }
}

/// 视频详情(播放地址 + 元数据),前端播放器直接消费
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoDetail {
    pub code: String,
    pub title: String,
    pub tags: Vec<String>,
    pub m3u8: String,
    /// 播放来源:"fs1"(优先)或 "missav"(兜底)
    pub provider: String,
    /// 是否中文字幕版本
    pub chinese: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_missav_search_cards() {
        // 最小卡片形态 fixture(不含敏感标题正文,占位文本代替)
        let html = r#"
          <a href="https://missav.ai/abc-123" alt="abc-123" >
            <img data-src="https://fourhoi.com/abc-123/cover-t.jpg">
          </a>
          <p class="x"> ABC-123 示例标题文本占位 - 演员名 </p>
          <a href="https://missav.ai/dm539/new">分页</a>
          <a href="https://missav.ai/fc2-ppv-5000463" alt="fc2">
        "#;
        let cards = super::VideoApi::parse_missav_search(html);
        assert_eq!(cards.len(), 2, "应解析出两个视频卡: {cards:?}");
        assert_eq!(cards[0].code, "abc-123");
        assert_eq!(
            cards[0].title, "示例标题文本占位",
            "应取番号后的标题并去演员后缀"
        );
        assert!(cards[0].thumbnail.ends_with("cover-t.jpg"));
        assert_eq!(cards[1].code, "fc2-ppv-5000463");
    }

    #[test]
    fn fourhoi_url_detection() {
        assert!(super::is_fourhoi_url(
            "https://fourhoi.com/spsc-037/cover-t.jpg"
        ));
        assert!(super::is_fourhoi_url(
            "https://cdn.fourhoi.com/x/cover.webp"
        ));
        assert!(!super::is_fourhoi_url("https://fs1.app/images/cover.jpg"));
        assert!(!super::is_fourhoi_url(
            "https://evil.example/fourhoi.com/cover.jpg"
        ));
        assert!(!super::is_fourhoi_url("不是 URL"));
    }

    #[test]
    fn fallback_scans_only_href_values() {
        // 真实样本(missav 搜索 GIGA 页)形态特征:结果卡片 href 番号 +
        // 大量 Tailwind 工具类(red-500/gray-300 等,正文里也出现同形文本)
        let html = r#"
          <div class="text-gray-300 bg-red-500 border-zinc-50 p-12">
            red-500 gray-300 zinc-50 pb-20
          </div>
          <a href="https://missav.ai/dm539/search/spsc-037?page=1">搜索</a>
          <a href="https://missav.ai/spsc-037" class="thumbnail group">
            <img data-src="https://fourhoi.com/spsc-037/cover-t.jpg">
          </a>
        "#;
        let cards = VideoApi::scan_codes_fallback(html);
        let codes: Vec<&str> = cards.iter().map(|c| c.code.as_str()).collect();
        assert_eq!(
            codes,
            ["spsc-037"],
            "只收录 href 内番号且去重,不含 CSS 类名伪番号: {codes:?}"
        );
        assert_eq!(
            cards[0].thumbnail,
            "https://fourhoi.com/spsc-037/cover-t.jpg"
        );
    }

    #[test]
    fn merged_has_next_across_sources() {
        // 缺陷场景回归:fs1 2 卡 + missav 12 卡,GIGA 搜索第一页(源站 67 页)
        assert!(merged_has_next(false, true), "missav 本页非空即可翻页");
        assert!(merged_has_next(true, false), "fs1 满页即翻页");
        assert!(!merged_has_next(false, false), "两源均无货则终止");
        assert!(merged_has_next(true, true));
    }

    #[test]
    fn normalizes_video_cover_urls() {
        assert_eq!(
            absolute_url("//cdn.example/cover.webp"),
            "https://cdn.example/cover.webp"
        );
        assert_eq!(
            absolute_url("/images/cover.jpg"),
            "https://fs1.app/images/cover.jpg"
        );
        assert_eq!(
            absolute_url("https://cdn.example/cover.jpg"),
            "https://cdn.example/cover.jpg"
        );
    }

    #[test]
    fn unpacks_packed_player_source() {
        // 真实样本:ssis-960-chinese-subtitle 详情页的 source 配置块(radix 15)
        let block = r#"eval(function(p,a,c,k,e,d){e=function(c){return c.toString(36)};if(!''.replace(/^/,String)){while(c--){d[c.toString(a)]=k[c]||c.toString(a)}k=[function(e){return d[e]}];e=function(){return'\\w+'};c=1};while(c--){if(k[c]){p=p.replace(new RegExp('\\b'+e(c)+'\\b','g'),k[c])}}return p}('e=\'8://7.6/5-4-3-2-1/d.0\';c=\'8://7.6/5-4-3-2-1/a/9.0\';b=\'8://7.6/5-4-3-2-1/a/9.0\';',15,15,'m3u8|b0b0abf841ac|83f0|4897|3c25|4a5f763b|com|surrit|https|video|720p|source1280|source842|playlist|source'.split('|'),0,{}))"#;
        let decoded = unpack_packed(block).expect("应解码 packer 块");
        assert!(
            decoded
                .contains("https://surrit.com/4a5f763b-3c25-4897-83f0-b0b0abf841ac/playlist.m3u8"),
            "解码结果应含主清单地址: {decoded}"
        );
        assert_eq!(
            extract_packed_m3u8(&format!("<html>{block}</html>")).as_deref(),
            Some("https://surrit.com/4a5f763b-3c25-4897-83f0-b0b0abf841ac/playlist.m3u8")
        );
    }

    #[test]
    fn follows_meta_refresh() {
        let html = r#"<html><head><meta charset="UTF-8" /><meta http-equiv="refresh" content="0;url='https://missav.ai/dm82/ssis-960-chinese-subtitle'" /></head></html>"#;
        let next = meta_refresh_url(html, "https://missav.ai/dm539/ssis-960-chinese-subtitle")
            .expect("应提取 meta-refresh 地址");
        assert_eq!(next, "https://missav.ai/dm82/ssis-960-chinese-subtitle");
        assert!(meta_refresh_url("<p>无跳转</p>", "https://missav.ai/").is_none());
    }

    #[test]
    #[ignore = "真实 API 联调(fs1 直连可达)"]
    fn video_live_endpoints() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(async {
                let api = VideoApi::new().expect("video client");

                let list = api.list(1).await.expect("list");
                println!(
                    "list page1: {} cards, hasNext={}",
                    list.items.len(),
                    list.pagination.has_next
                );
                assert!(!list.items.is_empty(), "list should not be empty");
                let first = list.items[0].clone();
                println!(
                    "first: code={} title={} views={} likes={}",
                    first.code,
                    first.title.chars().take(18).collect::<String>(),
                    first.views,
                    first.likes
                );
                assert!(first.code.contains('-'), "番号格式: {}", first.code);
                assert!(!first.thumbnail.is_empty(), "封面不应为空");
                assert!(!first.title.is_empty(), "卡片标题不应为空");
                let cover = api
                    .client
                    .get(&first.thumbnail)
                    .send()
                    .await
                    .expect("cover request")
                    .error_for_status()
                    .expect("cover status");
                println!("cover fetch: {}", cover.status());

                let search = api.search_fs1("ssis", 1).await.expect("search");
                println!("search_fs1[ssis]: {} cards", search.items.len());
                assert!(!search.items.is_empty());
                assert!(search.items.iter().all(|c| c.code.starts_with("ssis-")));

                let detail = api.detail_fs1(&first.code).await.expect("detail");
                println!(
                    "detail: title={}.. tags={} m3u8={}",
                    detail.title.chars().take(20).collect::<String>(),
                    detail.tags.len(),
                    &detail.m3u8[..detail.m3u8.len().min(60)]
                );
                assert!(detail.m3u8.starts_with("https://"), "m3u8 应为绝对地址");
                assert!(url::Url::parse(&detail.m3u8)
                    .unwrap()
                    .path()
                    .ends_with(".m3u8"));

                // 验证上游清单可读,且 hls_fetch 能正确改写相对地址。
                let (is_playlist, playlist, _bytes) = crate::stream_proxy::fetch_hls(&detail.m3u8)
                    .await
                    .expect("hls_fetch");
                assert!(is_playlist);
                assert!(playlist.starts_with("#EXTM3U"));
                assert!(!playlist.contains("\n1280x"), "相对地址应解析为绝对");
            });
    }
}

#[cfg(test)]
mod missav_tests {
    use super::*;

    /// missav 直连全量链路(桌面网络,需可达 missav.ai):
    /// 指纹客户端搜索 → 解析 → 详情(packer 解码 m3u8)→ 流代理拉取。
    /// CI/无头环境忽略。播放分片与快进(Range)由 stream_proxy 单测覆盖。
    #[test]
    #[ignore = "missav 全量联调:需桌面网络可达 missav.ai"]
    fn missav_full_chain() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(async {
                let api = VideoApi::new().expect("video client");

                // 1) 直连搜索(中文词兜底路径)
                let (cards, has_next) = api.search_missav("ssis", 1).await.expect("missav search");
                println!("missav search: {} cards has_next={has_next}", cards.len());
                assert!(!cards.is_empty(), "missav 搜索不应为空");

                // 2) 详情兜底:packer 解码出 m3u8(优先中字变体)
                let code = cards[0].code.clone();
                let (m3u8, chinese) = api.missav_m3u8(&code).await.expect("missav m3u8");
                println!("missav m3u8[{code}]: chinese={chinese} {m3u8}");
                assert!(m3u8.starts_with("https://"), "m3u8 应为绝对地址");
                assert!(url::Url::parse(&m3u8).unwrap().path().ends_with(".m3u8"));

                // 3) 流代理核心:surrit 清单拉取+重写(referer_for 已适配)
                let (is_playlist, rewritten, _b) = crate::stream_proxy::fetch_hls(&m3u8)
                    .await
                    .expect("hls fetch");
                assert!(is_playlist);
                assert!(rewritten.starts_with("#EXTM3U"));
                println!("m3u8 rewritten: {}B", rewritten.len());
            });
    }

    /// 完整 search() 链路真实关键词联调(桌面网络):验证 fs1→missav
    /// 兜底分叉与统一返回;并顺带验证首个结果的详情链路。
    #[test]
    #[ignore = "真实关键词联调:需桌面网络可达 fs1/missav"]
    fn search_full_chain_real_keyword() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(async {
                let api = VideoApi::new().expect("video client");
                for kw in ["giga", "中文字幕"] {
                    let fs1 = api.search_fs1(kw, 1).await;
                    let missav = api.search_missav(kw, 1).await;
                    let full = api.search(kw, 1).await.expect("search");
                    println!(
                        "[{kw}] fs1={} missav={} search()={} has_next={}",
                        fs1.as_ref().map(|r| r.items.len()).unwrap_or(999),
                        missav
                            .as_ref()
                            .map(|(v, next)| format!("{}卡 next={next}", v.len()))
                            .unwrap_or_else(|_| "FAIL".into()),
                        full.items.len(),
                        full.pagination.has_next
                    );
                    for card in full.items.iter().take(3) {
                        println!(
                            "  - {} | {}",
                            card.code,
                            card.title.chars().take(24).collect::<String>()
                        );
                    }
                }
                // 首个结果的详情链路(fs1 或 missav 兜底)
                let first = api.search("giga", 1).await.expect("search").items;
                if let Some(card) = first.first() {
                    let detail = api.detail(&card.code).await.expect("detail");
                    println!(
                        "detail[{}]: provider={} chinese={} m3u8={}",
                        detail.code,
                        detail.provider,
                        detail.chinese,
                        &detail.m3u8[..detail.m3u8.len().min(70)]
                    );
                    assert!(detail.m3u8.starts_with("https://"));
                } else {
                    panic!("giga 搜索应至少命中一路源");
                }
            });
    }
}
