//! 视频板块:fs1.app(Jable 系,服务端渲染,reqwest 直连可达)。
//! 列表/搜索/详情解析;m3u8 直接内嵌在详情页 HTML。
//! missav.ai 按 TLS 指纹拦截 reqwest(403),按番号直达的播放
//! 增强后续走 WebView 方案,当前播放统一用 fs1 的 m3u8。

use crate::models::*;
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use scraper::{Html, Selector};
use std::sync::OnceLock;
use tauri::Manager as _;

const BASE: &str = "https://fs1.app";

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
                .unwrap_or_default()
                .to_string();
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

    /// 按关键词(番号/标题)搜索
    pub async fn search(&self, keyword: &str, page: i32) -> Result<VideoListResult, String> {
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
                if url.ends_with(".m3u8") && url.contains("/hls/") {
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

    /// 详情(双源):元数据走 fs1(服务端渲染,稳定);播放地址优先
    /// missav(经网关解码 eval packer,番号直达),失败回退 fs1。
    /// 两端均优先 {code}-chinese-subtitle 中字变体。
    /// missav 网关不可用时静默降级,不影响可看性。
    pub async fn detail(&self, app: &tauri::AppHandle, code: &str) -> Result<VideoDetail, String> {
        let mut detail = self.detail_fs1(code).await;
        let manager = app.state::<crate::GatewayManager>();
        let lower = code.to_lowercase();
        // missav 优先中字变体,404/无流则回退普通番号页
        let missav_cs = crate::gateway::gateway_fetch_extract_m3u8(
            app,
            &manager,
            "missav",
            &format!("https://missav.ai/videos/{lower}-chinese-subtitle"),
        )
        .await;
        let missav_plain = if missav_cs.is_ok() {
            missav_cs.clone()
        } else {
            crate::gateway::gateway_fetch_extract_m3u8(
                app,
                &manager,
                "missav",
                &format!("https://missav.ai/videos/{lower}"),
            )
            .await
        };
        let chinese = missav_cs.is_ok();
        if let Ok(m3u8) = missav_plain {
            if !m3u8.is_empty() {
                if let Ok(d) = detail.as_mut() {
                    d.m3u8 = m3u8;
                    d.provider = "missav".into();
                    d.chinese = d.chinese || chinese;
                } else {
                    // fs1 元数据也失败:仅番号的纯播放模式
                    detail = Ok(VideoDetail {
                        code: lower,
                        title: code.to_uppercase(),
                        tags: vec![],
                        m3u8,
                        provider: "missav".into(),
                        chinese,
                    });
                }
            }
        }
        detail
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
    /// 播放来源:"missav"(优先)或 "fs1"(回退)
    pub provider: String,
    /// 是否中文字幕版本
    pub chinese: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

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

                let search = api.search("ssis", 1).await.expect("search");
                println!("search[ssis]: {} cards", search.items.len());
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
                assert!(detail.m3u8.ends_with(".m3u8"));

                // m3u8 实际可达(CDN 不做 CF 挑战)
                let resp = api
                    .client
                    .get(&detail.m3u8)
                    .send()
                    .await
                    .expect("m3u8 request");
                println!("m3u8 fetch: {}", resp.status());
                assert!(resp.status().is_success(), "m3u8 应可下载");
            });
    }
}
