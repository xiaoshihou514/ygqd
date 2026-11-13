//! 视频板块:fs1.app(Jable 系,服务端渲染,reqwest 直连可达)。
//! 列表/搜索/详情解析;m3u8 直接内嵌在详情页 HTML。
//! missav.ai 按 TLS 指纹拦截通用客户端(403):用 wreq 模拟 Chrome 指纹
//! 直连(搜索/兜底播放地址均不再经 WebView 网关);播放地址在详情页的
//! Dean Edwards packer 混淆脚本内,由 packer 模块解码提取。
//!
//! 模块划分(本文件只放 VideoApi、HTTP 层与 list/search/detail 入口):
//! - `models`:IPC 模型(VideoCard/VideoListResult/VideoDetail/…)
//! - `fs1`:fs1 卡片网格、搜索与详情
//! - `missav`:missav 搜索、兜底播放地址、元数据区、meta-refresh
//! - `follow`:关注目标列表与作品流
//! - `codes`:番号形态校验与同番号变体归并
//! - `packer`:Dean Edwards packer 解码
//! - `html`:两源共用的 HTML 文本小工具
//! - `error`:上游错误 → 简短中文(细节进日志)

mod codes;
mod error;
mod follow;
mod fs1;
mod html;
mod missav;
mod models;
mod packer;

pub use missav::is_fourhoi_url;
pub use models::{FollowTargetItem, VideoDetail, VideoListResult};

use codes::{dedupe_by_base, merged_has_next};
use error::{net_error, status_message, FS1, MISSAV};
use missav::{apply_missav_meta, meta_refresh_url};
use reqwest::header::{HeaderMap, HeaderValue, REFERER, USER_AGENT};
use std::time::Duration;

pub(crate) const BASE: &str = "https://fs1.app";
pub(crate) const MISSAV_BASE: &str = "https://missav.ai";

/// fs1(Jable 系)列表每页卡数:满页即认为还有下一页
const FS1_PAGE_SIZE: usize = 24;
/// 可选补充请求(fs1 线路的 missav 元数据、missav 线路的 fs1 日语标题)
/// 的时间预算:超时即放弃补充,不拖慢播放地址返回
const ENRICH_TIMEOUT: Duration = Duration::from_secs(12);
/// missav 间歇 403(CF 对连续请求的拦截)的退避时长
const MISSAV_403_BACKOFF: Duration = Duration::from_secs(3);
/// missav meta-refresh 跳转(含 403 退避重试)的请求预算
const MISSAV_MAX_HOPS: usize = 3;

pub struct VideoApi {
    pub client: reqwest::Client,
    /// missav 专用:Chrome TLS 指纹(wreq 默认不跟随重定向,构建时已设跟随)
    pub missav: wreq::Client,
}

impl VideoApi {
    pub fn new() -> Result<Self, String> {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(crate::sources::shared::FS1_UA),
        );
        headers.insert(REFERER, HeaderValue::from_static(BASE));
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("视频客户端初始化失败: {e}"))?;
        let missav = wreq::Client::builder()
            .emulation(wreq_util::Emulation::Chrome131)
            .redirect(wreq::redirect::Policy::limited(5))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|e| format!("missav 客户端初始化失败: {e}"))?;
        Ok(Self { client, missav })
    }

    /// fs1 GET(站内路径或绝对地址),返回 HTML;错误为简短中文
    async fn get(&self, path: &str) -> Result<String, String> {
        let url = if path.starts_with("http") {
            path.to_string()
        } else {
            // 标题统一日语(问题 7 用户要求):fs1 支持 ?lang=jp 返回
            // 日语原始标题(取证:?lang=jp 时 og:title 由繁中翻译变为
            // 日语原文);列表与详情全站统一
            let joiner = if path.contains('?') { '&' } else { '?' };
            format!("{BASE}{path}{joiner}lang=jp")
        };
        let fail = |e: reqwest::Error| net_error(FS1, &url, &e);
        self.client
            .get(&url)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(fail)?
            .text()
            .await
            .map_err(fail)
    }

    /// missav 直连 GET:HTTP 重定向由客户端跟随,HTML meta-refresh
    /// (dm 前缀轮换产生的跳转页)在此手动跟进,返回最终 HTML
    async fn missav_get(&self, url: &str) -> Result<String, String> {
        // 站内相对路径("/makers"、"/{code}")补全为绝对地址(wreq 不接受相对 URL)
        let mut current = if url.starts_with('/') {
            format!("{MISSAV_BASE}{url}")
        } else {
            url.to_string()
        };
        for hop in 0..MISSAV_MAX_HOPS {
            let fail = |e: wreq::Error| net_error(MISSAV, &current, &e);
            let resp = self
                .missav
                .get(&current)
                .header(REFERER, "https://missav.ai/")
                .send()
                .await
                .map_err(fail)?;
            let status = resp.status();
            let body = resp.text().await.map_err(fail)?;
            if !status.is_success() {
                // 问题 4:403 为 CF 对连续请求的间歇拦截,退避后由外层循环
                // 重试一次(hop 预算内);IP 级封禁时仍 403,如实返回错误
                // 由前端重试按钮兜底
                if status.as_u16() == 403 && hop == 0 {
                    tokio::time::sleep(MISSAV_403_BACKOFF).await;
                    continue;
                }
                error::log(MISSAV, &current, &status);
                return Err(status_message(MISSAV, status.as_u16()));
            }
            match meta_refresh_url(&body, &current) {
                Some(next) => current = next,
                None => return Ok(body),
            }
        }
        error::log(MISSAV, &current, &"meta-refresh hop budget exhausted");
        Err(format!("{MISSAV} 跳转次数过多"))
    }

    /// fs1 最新上市
    pub async fn list(&self, page: i32) -> Result<VideoListResult, String> {
        self.fs1_cards_page(&format!("/new-release/{page}/"), page)
            .await
    }

    /// 按关键词搜索:fs1 与 missav(直连)并发请求并合并,同基础番号
    /// 以 fs1 为准(fs1 有完整元数据);两路均失败时返回 fs1 的错误。
    pub async fn search(
        &self,
        keyword: &str,
        page: i32,
        sort: Option<&str>,
    ) -> Result<VideoListResult, String> {
        let (fs1_res, missav_res) = tokio::join!(
            self.search_fs1(keyword, page, sort),
            self.search_missav(keyword, page, sort)
        );
        let (missav, missav_has_next) = missav_res.unwrap_or_default();
        let fs1_has_next = matches!(&fs1_res, Ok(r) if r.items.len() >= FS1_PAGE_SIZE);
        let mut all = match fs1_res {
            Ok(r) => r.items,
            Err(e) if missav.is_empty() => return Err(e),
            Err(_) => Vec::new(),
        };
        all.extend(missav);
        // 需求 12:同番号变体去重(中字>有码>无码>英字取一)
        let items = dedupe_by_base(all);
        Ok(VideoListResult::paged(
            items,
            page,
            merged_has_next(fs1_has_next, missav_has_next),
        ))
    }

    /// 优先使用已验证可直连的 fs1 播放地址,失败后再以 missav 兜底。
    /// 两端均优先尝试中文字幕版本。
    ///
    /// fs1 详情页无厂商/女优/系列等元数据(取证:极简结构),从 missav
    /// 同番号详情页补齐;该请求与 fs1 并发发出(限时 ENRICH_TIMEOUT,
    /// 失败/超时只缺元数据),fs1 失败时随即取消,转 missav 兜底。
    pub async fn detail(&self, code: &str) -> Result<VideoDetail, String> {
        let lower = code.to_lowercase();
        let fs1 = self.detail_fs1(&lower);
        let meta = tokio::time::timeout(ENRICH_TIMEOUT, self.missav_meta(&lower));
        tokio::pin!(fs1, meta);
        let mut meta_early = None;
        let fs1_res = tokio::select! {
            biased;
            r = &mut fs1 => r,
            m = &mut meta => {
                meta_early = Some(m);
                (&mut fs1).await
            }
        };
        let fs1_error = match fs1_res {
            Ok(mut detail) => {
                // 标题保持 fs1 的日语标题,只补元数据
                let meta = match meta_early {
                    Some(m) => m,
                    None => meta.await,
                };
                if let Ok(Ok(meta)) = meta {
                    apply_missav_meta(&mut detail, &meta);
                }
                return Ok(detail);
            }
            Err(e) => e,
        };
        self.detail_missav(code)
            .await
            .map_err(|missav_error| format!("播放地址获取失败:{fs1_error};{missav_error}"))
    }
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
                let cover = api
                    .client
                    .get(&first.thumbnail)
                    .send()
                    .await
                    .expect("cover request")
                    .error_for_status()
                    .expect("cover status");
                println!("cover fetch: {}", cover.status());

                let search = api.search_fs1("ssis", 1, None).await.expect("search");
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

                // 验证上游清单可读,且 fetch_hls 能正确改写相对地址。
                let (is_playlist, playlist, _bytes) = crate::stream_proxy::fetch_hls(&detail.m3u8)
                    .await
                    .expect("fetch_hls");
                assert!(is_playlist);
                assert!(playlist.starts_with("#EXTM3U"));
                assert!(!playlist.contains("\n1280x"), "相对地址应解析为绝对");
            });
    }

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
                let (cards, has_next) = api
                    .search_missav("ssis", 1, None)
                    .await
                    .expect("missav search");
                println!("missav search: {} cards has_next={has_next}", cards.len());
                assert!(!cards.is_empty(), "missav 搜索不应为空");

                // 2) 详情兜底:packer 解码出 m3u8(优先中字变体)
                let code = cards[0].code.clone();
                let (m3u8, chinese, _html) = api.missav_m3u8(&code).await.expect("missav m3u8");
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
                    let fs1 = api.search_fs1(kw, 1, None).await;
                    let missav = api.search_missav(kw, 1, None).await;
                    let full = api.search(kw, 1, None).await.expect("search");
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
                let first = api.search("giga", 1, None).await.expect("search").items;
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
