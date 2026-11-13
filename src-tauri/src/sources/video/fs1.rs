//! fs1 源(Jable 系,服务端渲染):卡片网格解析、搜索与详情。

use super::codes::{base_code, dedup_key};
use super::error::FS1;
use super::html::{og_title, sel};
use super::models::{VideoCard, VideoDetail, VideoListResult};
use super::{VideoApi, BASE, FS1_PAGE_SIZE};
use scraper::{ElementRef, Html, Selector};
use std::collections::{hash_map::Entry, HashMap};
use std::sync::LazyLock;

/// 列表/搜索页卡片网格的选择器(只编译一次)
struct CardSelectors {
    card: Selector,
    link: Selector,
    /// Jable 卡片标题在 h6(或 .title)内,文本含"番号 完整标题"
    title: Selector,
    img: Selector,
    /// 封面遮罩右下角的时长 label(如 "2:51:49")
    duration: Selector,
    /// sub-title 形如 "315 903 ❤ 1403"(svg 图标分隔的观看数与点赞数)
    stats: Selector,
}

static CARD: LazyLock<CardSelectors> = LazyLock::new(|| CardSelectors {
    card: sel("div.video-img-box"),
    link: sel("a[href*='/videos/']"),
    title: sel("h6, .title"),
    img: sel("img"),
    duration: sel(".absolute-bottom-right .label"),
    stats: sel("p.sub-title"),
});

static DETAIL_TITLE: LazyLock<Selector> = LazyLock::new(|| sel("h4"));
/// 只取影片自身的标签区(h5.tags);页面下方的全站标签云同为 a.tag,
/// 全选会混进上百个无关标签
static DETAIL_TAGS: LazyLock<Selector> = LazyLock::new(|| sel("h5.tags a"));

/// 封面等站内地址补全为绝对 URL(协议相对/站内绝对/站内相对)
pub(super) fn absolute_url(value: &str) -> String {
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

/// 元素的去首尾空白文本
fn trimmed_text(el: ElementRef<'_>) -> String {
    el.text().collect::<String>().trim().to_string()
}

/// 从 /videos/{code}/ 链接提取番号(小写)
fn code_from_href(href: &str) -> Option<String> {
    let code = href.split("/videos/").nth(1)?.trim_matches('/');
    (!code.is_empty()).then(|| code.to_lowercase())
}

/// 解析列表/搜索页的卡片网格。同番号的普通版与中字版只留一张
/// (优先中字),保持首现位置
pub(super) fn parse_cards(html: &str) -> Vec<VideoCard> {
    let doc = Html::parse_document(html);
    let s = &*CARD;
    let mut out: Vec<VideoCard> = Vec::new();
    let mut by_base: HashMap<String, usize> = HashMap::new();
    for card in doc.select(&s.card) {
        let Some(code) = card
            .select(&s.link)
            .find_map(|a| code_from_href(a.value().attr("href")?))
        else {
            continue;
        };
        let title = card
            .select(&s.title)
            .next()
            .map(trimmed_text)
            .unwrap_or_default();
        let thumbnail = card
            .select(&s.img)
            .next()
            .and_then(|img| {
                img.value()
                    .attr("data-src")
                    .or_else(|| img.value().attr("src"))
            })
            .map(absolute_url)
            .unwrap_or_default();
        let duration = card
            .select(&s.duration)
            .next()
            .map(trimmed_text)
            .unwrap_or_default();
        let (views, likes) = card
            .select(&s.stats)
            .next()
            .map(|p| {
                let mut nums = p.text().map(str::trim).filter(|t| !t.is_empty());
                (
                    nums.next().unwrap_or_default().to_string(),
                    nums.next().unwrap_or_default().to_string(),
                )
            })
            .unwrap_or_default();
        let chinese = code.ends_with("-chinese-subtitle");
        let item = VideoCard {
            code,
            title,
            thumbnail,
            views,
            likes,
            chinese,
            duration,
        };
        match by_base.entry(dedup_key(&item.code).to_owned()) {
            Entry::Occupied(slot) => {
                let existing = &mut out[*slot.get()];
                if item.chinese && !existing.chinese {
                    *existing = item;
                }
            }
            Entry::Vacant(slot) => {
                slot.insert(out.len());
                out.push(item);
            }
        }
    }
    out
}

/// 详情页内嵌的 HLS 主清单地址(hlsUrl/jwplayer 配置):首个路径以
/// .m3u8 结尾且含 /hls/ 的 https 地址
fn find_hls_m3u8(html: &str) -> Option<String> {
    html.split("https://").skip(1).find_map(|seg| {
        let end = seg.find(|c: char| matches!(c, '"' | '\'' | '<' | '>') || c.is_whitespace())?;
        let tail = &seg[..end];
        // 廉价预筛:路径以 .m3u8 结尾的地址必含该子串
        if !tail.contains(".m3u8") {
            return None;
        }
        let url = format!("https://{tail}");
        let is_playlist = url::Url::parse(&url)
            .ok()
            .is_some_and(|parsed| parsed.path().ends_with(".m3u8"));
        (is_playlist && url.contains("/hls/")).then_some(url)
    })
}

/// 解析 fs1 详情页(标题/标签/m3u8);元数据字段留空由 missav 补齐
fn parse_detail(html: &str, code: &str) -> Result<VideoDetail, String> {
    let m3u8 = find_hls_m3u8(html).ok_or_else(|| format!("{FS1} 详情页未找到播放地址"))?;
    let doc = Html::parse_document(html);
    let title = doc
        .select(&DETAIL_TITLE)
        .next()
        .map(trimmed_text)
        .unwrap_or_default();
    let tags: Vec<String> = doc
        .select(&DETAIL_TAGS)
        .map(trimmed_text)
        .filter(|t| !t.is_empty())
        .collect();
    let mut detail = VideoDetail::bare(code.to_string(), title, m3u8, FS1);
    detail.tags = tags;
    Ok(detail)
}

impl VideoApi {
    /// fs1 侧搜索(番号/日文词;中文词在 fs1 索引中常无结果)。
    /// sort: None=默认;Some 支持 post_date(最近更新)/video_viewed
    /// (最多观看)/most_favourited(最高收藏)/post_date_and_popularity
    /// (近期最佳)——排序时走 KVS async 块形态(返回卡片片段,无分页
    /// 导航,has_next 按卡片数推断,仅首页语义)。
    pub(super) async fn search_fs1(
        &self,
        keyword: &str,
        page: i32,
        sort: Option<&str>,
    ) -> Result<VideoListResult, String> {
        let encoded: String = url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect();
        let path = match sort {
            None => format!("/search/{encoded}/{page}/"),
            Some(s) => format!(
                "/search/{encoded}/?mode=async&function=get_block&block_id=list_videos_videos_list_search_result&sort_by={s}"
            ),
        };
        let items = parse_cards(&self.get(&path).await?);
        let has_next = !items.is_empty();
        Ok(VideoListResult::paged(items, page, has_next))
    }

    /// fs1 卡片网格页(最新上市/女优作品流),每页 24 卡,满页即有下一页
    pub(super) async fn fs1_cards_page(
        &self,
        path: &str,
        page: i32,
    ) -> Result<VideoListResult, String> {
        let html = self.get(path).await?;
        let items = parse_cards(&html);
        let has_next = items.len() >= FS1_PAGE_SIZE;
        Ok(VideoListResult::paged(items, page, has_next))
    }

    /// fs1 侧详情:元数据 + fs1 的 m3u8(番号统一小写)。
    /// 中文字幕变体(/videos/{code}-chinese-subtitle/)与原路径并发请求,
    /// 中字可用时优先,否则回退原路径的结果
    pub(super) async fn detail_fs1(&self, code: &str) -> Result<VideoDetail, String> {
        let code = code.to_lowercase();
        let chinese_path = format!("{code}-chinese-subtitle/");
        let plain_path = format!("{code}/");
        let (chinese, plain) = tokio::join!(
            self.detail_fs1_path(&chinese_path, &code),
            self.detail_fs1_path(&plain_path, &code),
        );
        match chinese {
            Ok(mut detail) => {
                detail.chinese = true;
                Ok(detail)
            }
            Err(_) => plain,
        }
    }

    async fn detail_fs1_path(&self, path: &str, code: &str) -> Result<VideoDetail, String> {
        let html = self.get(&format!("/videos/{path}")).await?;
        parse_detail(&html, code)
    }

    /// fs1 ?lang=jp 详情页的 og:title(日语原题);无此番号返回 None
    pub(super) async fn fs1_jp_title(&self, code: &str) -> Option<String> {
        let path = format!("/videos/{}/", base_code(&code.to_lowercase()));
        og_title(&self.get(&path).await.ok()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn parses_card_grid_and_prefers_chinese_variant() {
        // Jable 卡片网格最小形态(文本用占位)
        let html = r#"
          <div class="video-img-box">
            <a href="https://fs1.app/videos/ABC-001/"><img data-src="/covers/abc-001.jpg"></a>
            <div class="absolute-bottom-right"><span class="label">2:51:49</span></div>
            <h6 class="title"> ABC-001 占位标题 </h6>
            <p class="sub-title">315 903<svg></svg>1403</p>
          </div>
          <div class="video-img-box">
            <a href="https://fs1.app/videos/xyz-002/"><img src="//cdn.example/x.jpg"></a>
            <h6 class="title">XYZ-002 另一部</h6>
          </div>
          <div class="video-img-box">
            <a href="https://fs1.app/videos/abc-001-chinese-subtitle/"><img src="/c.jpg"></a>
            <h6 class="title">ABC-001 中字</h6>
          </div>
          <div class="video-img-box"><a href="https://fs1.app/models/x/">非视频</a></div>
        "#;
        let cards = parse_cards(html);
        let codes: Vec<&str> = cards.iter().map(|c| c.code.as_str()).collect();
        assert_eq!(codes, ["abc-001-chinese-subtitle", "xyz-002"]);
        assert!(cards[0].chinese);
        assert_eq!(cards[0].title, "ABC-001 中字");
        assert_eq!(cards[1].thumbnail, "https://cdn.example/x.jpg");
        assert_eq!(cards[1].views, "");

        let plain = parse_cards(&html.replace("abc-001-chinese-subtitle", "zzz-003"));
        assert_eq!(plain[0].code, "abc-001");
        assert_eq!(plain[0].thumbnail, "https://fs1.app/covers/abc-001.jpg");
        assert_eq!(plain[0].duration, "2:51:49");
        assert_eq!(
            (plain[0].views.as_str(), plain[0].likes.as_str()),
            ("315 903", "1403")
        );
    }

    #[test]
    fn parses_detail_page() {
        let html = r#"<html><body>
          <h4> ABC-001 日本語タイトル </h4>
          <h5 class="tags"><a href="/t/1">标签甲</a><a href="/t/2"> </a><a href="/t/3">标签乙</a></h5>
          <a class="tag" href="/t/9">全站标签云</a>
          <script>var x = 'https://cdn.example/preview.mp4'; var hlsUrl = 'https://cdn.example/hls/abc/index.m3u8?t=1';</script>
        </body></html>"#;
        let detail = parse_detail(html, "abc-001").expect("应解析详情");
        assert_eq!(detail.title, "ABC-001 日本語タイトル");
        assert_eq!(detail.tags, ["标签甲", "标签乙"]);
        assert_eq!(detail.m3u8, "https://cdn.example/hls/abc/index.m3u8?t=1");
        assert_eq!(detail.provider, "fs1");
        assert!(!detail.chinese);

        let err = parse_detail("<h4>无播放器</h4>", "abc-001").unwrap_err();
        assert_eq!(err, "fs1 详情页未找到播放地址");
    }
}
