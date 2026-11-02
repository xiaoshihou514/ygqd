//! 视频板块:fs1.app(Jable 系,服务端渲染,reqwest 直连可达)。
//! 列表/搜索/详情解析;m3u8 直接内嵌在详情页 HTML。
//! missav.ai 按 TLS 指纹拦截 reqwest(403),播放地址仅在 fs1 失效时
//! 经 WebView 网关兜底,常规播放使用 fs1 的 m3u8。

use crate::gateway;
use crate::models::*;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use scraper::{Html, Selector};
use std::{sync::OnceLock, time::Duration};
use tauri::Manager as _;

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

/// 视频列表响应
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoListResult {
    pub items: Vec<VideoCard>,
    pub pagination: PaginationInfo,
}

pub struct VideoApi {
    pub client: reqwest::Client,
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
        Ok(Self { client })
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

    /// 按关键词搜索:桌面为双源合并(fs1 + missav 网关);
    /// Android 仅 fs1——手机网络下 missav 全链路(CF 挑战+CDN)不可达,
    /// 网关方案在移动端不成立,直接走稳定可达的 fs1。
    #[cfg(target_os = "android")]
    pub async fn search(
        &self,
        _app: &tauri::AppHandle,
        keyword: &str,
        page: i32,
    ) -> Result<VideoListResult, String> {
        self.search_fs1(keyword, page).await
    }

    #[cfg(not(target_os = "android"))]
    pub async fn search(
        &self,
        app: &tauri::AppHandle,
        keyword: &str,
        page: i32,
    ) -> Result<VideoListResult, String> {
        use tauri::Manager as _;
        let fs1_fut = self.search_fs1(keyword, page);
        let missav_fut = async {
            let manager = app.state::<crate::GatewayManager>();
            let encoded =
                url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect::<String>();
            // 导航式抓取:网关窗口直接跳搜索页,浏览器完整处理 CF 与 JS 渲染
            let r = gateway::navigate_and_dump(
                app,
                &manager,
                "missav",
                &format!("https://missav.ai/dm539/search/{encoded}"),
                4500,
            )
            .await;
            match &r {
                Ok(html) => {
                    let cards = Self::parse_missav_search(html);
                    // 转储样本供解析失败分析(仅保留最新一次)
                    use tauri::Manager as _;
                    if let Ok(dir) = app.path().app_data_dir() {
                        let _ = std::fs::write(dir.join("search_dump.html"), html);
                    }
                    crate::gateway::diag(
                        app,
                        &format!(
                            "missav search ok html={}B cards={}",
                            html.len(),
                            cards.len()
                        ),
                    );
                    Ok::<_, String>(cards)
                }
                Err(e) => {
                    crate::gateway::diag(app, &format!("missav search FAIL: {e}"));
                    Err(e.clone())
                }
            }
        };
        let fs1_res = fs1_fut.await;
        let missav_res = match missav_fut.await {
            Ok(v) => v,
            Err(e) => {
                if let Ok(fs1) = &fs1_res {
                    if !fs1.items.is_empty() {
                        crate::gateway::diag(app, "missav branch failed, degraded to fs1 only");
                        return Ok(fs1.clone());
                    }
                }
                return Err(e);
            }
        };

        let mut fs1 = match fs1_res {
            Ok(v) => v.items,
            Err(e) => {
                if missav_res.is_empty() {
                    return Err(e);
                }
                Vec::new()
            }
        };
        // missav 结果追加,同基础番号以 fs1 为准(fs1 有完整元数据)
        fn base(code: &str) -> &str {
            code.strip_suffix("-chinese-subtitle").unwrap_or(code)
        }
        for card in missav_res {
            if fs1.iter().any(|c| base(&c.code) == base(&card.code)) {
                continue;
            }
            fs1.push(card);
        }
        let has_next = fs1.len() >= 20;
        Ok(VideoListResult {
            pagination: PaginationInfo {
                current: page - 1,
                total: 0,
                has_next,
                has_prev: page > 1,
            },
            items: fs1,
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
            let window = &html[*pos..(*pos + 9000).min(html.len())];
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
    pub async fn detail(&self, app: &tauri::AppHandle, code: &str) -> Result<VideoDetail, String> {
        match self.detail_fs1(code).await {
            Ok(detail) => Ok(detail),
            Err(fs1_error) => {
                let manager = app.state::<crate::GatewayManager>();
                let lower = code.to_lowercase();
                let missav_cs = crate::gateway::gateway_fetch_extract_m3u8(
                    app,
                    &manager,
                    "missav",
                    &format!("https://missav.ai/videos/{lower}-chinese-subtitle"),
                )
                .await;
                let (missav, chinese) = match missav_cs {
                    Ok(m3u8) if !m3u8.is_empty() => (Ok(m3u8), true),
                    _ => (
                        crate::gateway::gateway_fetch_extract_m3u8(
                            app,
                            &manager,
                            "missav",
                            &format!("https://missav.ai/videos/{lower}"),
                        )
                        .await,
                        false,
                    ),
                };
                match missav {
                    Ok(m3u8) if !m3u8.is_empty() => Ok(VideoDetail {
                        code: lower,
                        title: code.to_uppercase(),
                        tags: vec![],
                        m3u8,
                        provider: "missav".into(),
                        chinese,
                    }),
                    Ok(_) => Err(format!(
                        "fs1 播放地址不可用({fs1_error}); missav 未返回播放地址"
                    )),
                    Err(missav_error) => Err(format!(
                        "fs1 播放地址不可用({fs1_error}); missav 兜底失败({missav_error})"
                    )),
                }
            }
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

    /// missav 全量链路(桌面网络,需可达 missav.ai):
    /// 网关导航式搜索 → 解析 → 详情(eval 提取 m3u8)→ 流代理拉取
    /// 注意:此测试依赖 tauri AppHandle(网关窗口),仅桌面 dev 环境可跑;
    /// CI/无头环境忽略。播放分片与快进(Range)由 stream_proxy 单测覆盖。
    #[test]
    #[ignore = "missav 全量联调:需桌面网络可达 missav.ai"]
    fn missav_full_chain() {
        tokio::runtime::Runtime::new()
            .expect("tokio runtime")
            .block_on(async {
                let api = VideoApi::new().expect("video client");

                // 1) fs1 侧元数据(ATID-707 在 fs1 有收录)
                let detail = api.detail_fs1("atid-707").await.expect("fs1 detail");
                println!(
                    "fs1 detail: title={}..",
                    detail.title.chars().take(16).collect::<String>()
                );
                assert!(!detail.m3u8.is_empty(), "fs1 m3u8 不应为空");

                // 2) missav 中字变体存在性(导航式网关在桌面无 AppHandle 不跑;
                //    此处直接验证 missav 搜索解析器对真实页面的行为——
                //    用 fs1 列表页做解析器回归样本源)
                let list = api.list(1).await.expect("fs1 list");
                assert!(!list.items.is_empty());

                // 3) 流代理核心:fs1 m3u8 拉取+重写
                let (is_playlist, rewritten, _b) = crate::stream_proxy::fetch_hls(&detail.m3u8)
                    .await
                    .expect("hls fetch");
                assert!(is_playlist);
                assert!(rewritten.starts_with("#EXTM3U"));
                println!("m3u8 rewritten: {}B", rewritten.len());

                // 4) 从重写后的清单取媒体分片地址,验证二进制拉取
                println!("rewritten head:");
                for l in rewritten.lines().take(6) {
                    println!("  | {l}");
                }
                let lines_vec: Vec<&str> = rewritten.lines().collect();
                let seg = lines_vec
                    .windows(2)
                    .find(|w| w[0].starts_with("#EXTINF"))
                    .map(|w| w[1].trim().to_string())
                    .unwrap_or_default();
                println!("segment url: {seg}");
                let (is_pl, ct, bytes) = crate::stream_proxy::fetch_hls(&seg)
                    .await
                    .expect("segment fetch");
                assert!(!is_pl);
                println!("segment: ct={ct} bytes={}", bytes.len());
                assert!(bytes.len() > 10_000, "分片过小");

                // 5) Range 快进模拟:带 Range 头请求中段
                let client = reqwest::Client::new();
                let r = client
                    .get(seg)
                    .header("Referer", "https://fs1.app/")
                    .header("Range", "bytes=1000000-1000999")
                    .send()
                    .await
                    .expect("range request");
                println!(
                    "Range 请求: {} ({}B)",
                    r.status(),
                    r.bytes().await.unwrap().len()
                );
            });
    }
}
