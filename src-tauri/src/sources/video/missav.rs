//! missav 源:搜索结果解析、兜底播放地址(packer 解码)、详情元数据区
//! 与 meta-refresh 跳转。

use super::codes::{base_code, dedupe_by_base, is_video_code, VIDEO_CODE};
use super::error::MISSAV;
use super::html::{og_title, strip_tags, HREF};
use super::models::{MetaLink, VideoCard, VideoDetail};
use super::packer::extract_packed_m3u8;
use super::{VideoApi, MISSAV_BASE};
use crate::sources::shared::percent_decode_simple;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// 站点根地址(相对 href 的 join 基准)
static ROOT: LazyLock<url::Url> =
    LazyLock::new(|| url::Url::parse("https://missav.ai/").expect("static url"));
/// 卡片封面链接内的右下角时长角标:`<a href=".../{番号}"><span class="absolute
/// bottom-1 right-1 …">2:08:22</span>`(missav 列表页实测,每卡一个)
static DURATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"<a[^>]*href="([^"]+)"[^>]*>\s*<span[^>]*class="[^"]*absolute[^"]*bottom-1[^"]*right-1[^"]*"[^>]*>\s*(\d{1,2}:\d{2}(?::\d{2})?)\s*</span>"#,
    )
    .expect("static regex")
});
/// 文本节点:`>` 与其后最近的 `<` 之间
static TEXT_NODE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r">([^<>]*)<").expect("static regex"));
static META_LABEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"<span>\s*([^<:：]{1,8})\s*[:：]\s*</span>").expect("static regex")
});
static META_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<a[^>]*href="([^"]+)"[^>]*>([\s\S]{0,200}?)</a>"#).expect("static regex")
});
static META_SPAN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<span[^>]*>([\s\S]{0,400}?)</span>").expect("static regex"));
static REFRESH_TAG: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<meta[^>]*http-equiv\s*=\s*['"]?refresh[^>]*>"#).expect("static regex")
});
static REFRESH_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)url\s*=\s*['"]?([^'">\s]+)"#).expect("static regex"));

/// 卡片标题窗口:锚点后多少字节内找标题文本
const TITLE_WINDOW: usize = 9000;

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

/// 元数据区标签名(繁/简)→ 条目 kind
fn meta_kind_for_label(label: &str) -> Option<&'static str> {
    Some(match label {
        "女優" | "女优" => "actress",
        "男優" | "男优" => "actor",
        "類型" | "类型" => "genre",
        "系列" => "series",
        "發行商" | "发行商" => "maker",
        "導演" | "导演" => "director",
        "標籤" | "标签" => "label",
        _ => return None,
    })
}

/// missav 条目列表页地址:/ja/{路由}/{id}(id 按路径段编码),第 2 页起 ?page=N。
/// kind 兼容关注表里的单数形态(maker/actress)与元数据条目的 kind
pub(super) fn missav_list_url(kind: &str, target_id: &str, page: i32) -> Result<String, String> {
    let route = missav_route_for_kind(kind).ok_or_else(|| format!("不支持的条目类型 {kind}"))?;
    let id = percent_decode_simple(target_id);
    if id.is_empty() {
        return Err("条目 id 为空".into());
    }
    let mut url = ROOT.clone();
    // ja 语言前缀:列表卡片标题为日语原题(与 fs1 ?lang=jp、详情页「標題」一致)
    url.path_segments_mut()
        .map_err(|_| "无效地址".to_string())?
        .clear()
        .push("ja")
        .push(route)
        .push(&id);
    if page > 1 {
        url.set_query(Some(&format!("page={page}")));
    }
    Ok(url.to_string())
}

/// 条目 kind → missav 列表路由
fn missav_route_for_kind(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "maker" | "makers" => "makers",
        "actress" | "actresses" => "actresses",
        "actor" | "actors" => "actors",
        "series" => "series",
        "director" | "directors" => "directors",
        "label" | "labels" => "labels",
        "genre" | "genres" => "genres",
        _ => return None,
    })
}

/// 提取 meta-refresh 跳转地址并解析为绝对 URL(无则返回 None)。
/// missav 详情路径会先返回形如
/// `<meta http-equiv="refresh" content="0;url='https://missav.ai/dm82/…'">` 的跳转页
pub(super) fn meta_refresh_url(html: &str, base_url: &str) -> Option<String> {
    let tag = REFRESH_TAG.find(html)?.as_str();
    let target = REFRESH_URL.captures(tag)?.get(1)?.as_str();
    let base = url::Url::parse(base_url).ok()?;
    base.join(&target.replace("&amp;", "&"))
        .ok()
        .map(|u| u.to_string())
}

/// 播放路径:中字变体优先,原番号回退。只剥 -chinese-subtitle(保留
/// -uncensored-leak 等后缀)——missav 播放页按完整变体路径分发
fn missav_video_paths(code: &str) -> [(String, bool); 2] {
    let lower = code.to_lowercase();
    let bare = lower.strip_suffix("-chinese-subtitle").unwrap_or(&lower);
    [
        (format!("{bare}-chinese-subtitle"), true),
        (bare.to_string(), false),
    ]
}

/// href 末段番号(相对/绝对/带语言前缀统一 join 后取末段),非番号形态返回 None
fn code_from_href(href: &str) -> Option<String> {
    let joined = ROOT.join(href).ok()?;
    let code = joined
        .path()
        .trim_matches('/')
        .split('/')
        .next_back()?
        .to_lowercase();
    is_video_code(&code).then_some(code)
}

/// 标题文本:窗口内首个以「基础番号大写 + 空白」开头的文本节点的余下
/// 部分(5-200 字符,尾随空白不计)
fn find_title<'a>(window: &'a str, base_upper: &str) -> Option<&'a str> {
    TEXT_NODE.captures_iter(window).find_map(|c| {
        let node = c.get(1)?.as_str().trim_start();
        let after = node.strip_prefix(base_upper)?;
        let sep = after.chars().next().filter(|c| c.is_whitespace())?;
        let rest = &after[sep.len_utf8()..];
        (rest.chars().count() >= 5 && rest.trim_end().chars().count() <= 200).then_some(rest)
    })
}

/// 解析 missav 搜索结果/列表页:通用 href 提取(SSR 与 Vue hydration 后的
/// DOM 均适用)+ fourhoi.com/{code}/cover-t.jpg 规律封面 + 番号前缀归属标题
pub(super) fn parse_missav_search(html: &str) -> Vec<VideoCard> {
    // 番号形态校验用公共 is_video_code(与兜底扫描同一模式定义)。
    // 页面标题文本以基础番号(无后缀)大写开头,后缀只存在于 href。
    let anchors: Vec<(usize, String)> = HREF
        .captures_iter(html)
        .filter_map(|c| Some((c.get(0)?.start(), code_from_href(c.get(1)?.as_str())?)))
        .collect();

    let durations: HashMap<String, &str> = DURATION
        .captures_iter(html)
        .filter_map(|c| {
            let code = c
                .get(1)?
                .as_str()
                .trim_end_matches('/')
                .rsplit('/')
                .next()?
                .split(['?', '#'])
                .next()?
                .to_lowercase();
            Some((code, c.get(2)?.as_str()))
        })
        .collect();

    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for (pos, code) in anchors {
        if !seen.insert(code.clone()) {
            continue;
        }
        // 页面标题不含后缀(FTHTD-242 而非 FTHTD-242-UNCENSORED-LEAK),
        // 故剥掉 leak/中字后缀再匹配
        let base_upper = code
            .strip_suffix("-uncensored-leak-chinese-subtitle")
            .or_else(|| code.strip_suffix("-uncensored-leak"))
            .or_else(|| code.strip_suffix("-chinese-subtitle"))
            .unwrap_or(&code)
            .to_uppercase();
        // 窗口终点按 UTF-8 字符边界右移(zh-Hant 页面多字节字符会落进窗口)
        let mut end = (pos + TITLE_WINDOW).min(html.len());
        while !html.is_char_boundary(end) {
            end += 1;
        }
        let title = find_title(&html[pos..end], &base_upper)
            .and_then(|t| t.split(" - ").next())
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map_or_else(|| code.to_uppercase(), str::to_string);
        let duration = durations
            .get(&code)
            .copied()
            .unwrap_or_default()
            .to_string();
        let mut card = VideoCard::from_code(code, title);
        card.duration = duration;
        out.push(card);
    }
    // 需求 12 归并语义贯彻页内:同番号变体按优先级取一
    // (中字>有码>无码>英字),与 search 合并层行为一致
    dedupe_by_base(out)
}

/// 兜底:番号形态 href 扫描(hydration 后 DOM 结构不可预知时,
/// 番号链接仍以 href 属性形态存在)。与主解析同边界——只扫 href
/// 属性值,避免把页面 CSS 工具类(red-500/gray-300 等)误当番号;
/// 番号模式与主解析共用 VIDEO_CODE(leak/中字/组合形态完整收录,
/// 不丢后缀);封面按 fourhoi 规律构造。
pub(super) fn scan_codes_fallback(html: &str) -> Vec<VideoCard> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for cap in HREF.captures_iter(html) {
        let href = cap.get(1).map_or("", |m| m.as_str());
        for m in VIDEO_CODE.find_iter(href) {
            let code = m.as_str().to_lowercase();
            if seen.insert(code.clone()) {
                let title = code.to_uppercase();
                out.push(VideoCard::from_code(code, title));
            }
        }
    }
    out
}

/// missav 详情页元数据区解析结果
#[derive(Clone, Debug, Default)]
pub(super) struct MissavMeta {
    /// og:title(繁中翻译标题)
    pub(super) og_title: Option<String>,
    /// 元数据区「標題」(日语原题)
    pub(super) title_jp: Option<String>,
    release_date: Option<String>,
    links: Vec<MetaLink>,
}

impl MissavMeta {
    fn of(&self, kind: &str) -> Vec<MetaLink> {
        self.links
            .iter()
            .filter(|l| l.kind == kind)
            .cloned()
            .collect()
    }
}

/// missav 详情页元数据区解析。区块形如
/// `<div class="text-secondary"><span>女優:</span><a href=".../dm41/actresses/{id}">名</a>…</div>`,
/// 逐个标签块取链接(只在块内取,不会误收页面别处的推荐/排行链接)。
/// 「標題」块是日语原题(og:title 为繁中翻译)。
pub(super) fn parse_missav_meta(html: &str) -> MissavMeta {
    let mut meta = MissavMeta {
        og_title: og_title(html),
        ..Default::default()
    };
    let mut seen = HashSet::new();
    for cap in META_LABEL.captures_iter(html) {
        let label = cap[1].trim();
        let start = cap.get(0).map_or(0, |m| m.end());
        let rest = &html[start..];
        let block = &rest[..rest.find("</div>").unwrap_or(rest.len()).min(4000)];
        match label {
            "標題" | "标题" => {
                if let Some(t) = META_SPAN.captures(block) {
                    let t = strip_tags(&t[1]);
                    if !t.is_empty() {
                        meta.title_jp = Some(t);
                    }
                }
                continue;
            }
            "發行日期" | "发行日期" => {
                let t = strip_tags(block);
                if !t.is_empty() {
                    meta.release_date = Some(t);
                }
                continue;
            }
            _ => {}
        }
        let Some(kind) = meta_kind_for_label(label) else {
            continue;
        };
        let Some(route) = missav_route_for_kind(kind) else {
            continue;
        };
        // 路由末段为 id:…/dm41/actresses/{id}、…/series/{id}
        let marker = format!("/{route}/");
        for link in META_LINK.captures_iter(block) {
            let href = &link[1];
            let Some(pos) = href.find(&marker) else {
                continue;
            };
            let id_raw = href[pos + marker.len()..]
                .split(['?', '#', '/'])
                .next()
                .unwrap_or("");
            let id = percent_decode_simple(id_raw);
            let name = strip_tags(&link[2]);
            if id.is_empty() || name.is_empty() || !seen.insert((kind, id.clone())) {
                continue;
            }
            meta.links.push(MetaLink {
                kind: kind.to_string(),
                id,
                name,
            });
        }
    }
    meta
}

/// 把 missav 元数据灌进详情(厂商/女优/男优/系列/导演/标签 + 兼容字段)
pub(super) fn apply_missav_meta(detail: &mut VideoDetail, meta: &MissavMeta) {
    detail.makers = meta.of("maker");
    detail.actresses = meta.of("actress");
    detail.male_actors = meta.of("actor");
    detail.series = meta.of("series");
    detail.directors = meta.of("director");
    detail.labels = meta.of("label");
    if detail.release_date.is_none() {
        detail.release_date = meta.release_date.clone();
    }
    // 兼容字段:studio = 首个厂商名,actors = 女优名
    detail.studio = detail.makers.first().map(|m| m.name.clone());
    detail.actors = detail.actresses.iter().map(|a| a.name.clone()).collect();
    if detail.tags.is_empty() {
        detail.tags = meta.of("genre").into_iter().map(|g| g.name).collect();
    }
}

impl VideoApi {
    /// missav 搜索(中文词兜底):dm539 搜索页为服务端渲染,直连可达。
    /// 返回结果卡片与「本页有货」标志(dm539 每页仅 12 卡,不能沿用
    /// fs1 的每页卡数阈值推断下一页)。
    pub(super) async fn search_missav(
        &self,
        keyword: &str,
        page: i32,
        sort: Option<&str>,
    ) -> Result<(Vec<VideoCard>, bool), String> {
        let encoded: String = url::form_urlencoded::byte_serialize(keyword.as_bytes()).collect();
        let sort_seg = sort.map(|s| format!("&sort={s}")).unwrap_or_default();
        let html = self
            .missav_get(&format!(
                "{MISSAV_BASE}/dm539/ja/search/{encoded}?page={page}{sort_seg}"
            ))
            .await?;
        let mut cards = parse_missav_search(&html);
        // hydration DOM 形态不可预知:主解析为空时全文番号扫描兜底
        if cards.is_empty() {
            cards = scan_codes_fallback(&html);
        }
        let has_next = !cards.is_empty();
        Ok((cards, has_next))
    }

    /// missav 兜底播放地址:详情页的 packer 混淆脚本解码出 m3u8。
    /// 优先中文字幕变体,不存在回退原番号。同页 HTML 一并返回供元数据
    /// 区解析复用,不再二次抓取
    pub(super) async fn missav_m3u8(&self, code: &str) -> Result<(String, bool, String), String> {
        let mut fetch_error = None;
        let mut page_fetched = false;
        for (path, chinese) in missav_video_paths(code) {
            match self
                .missav_get(&format!("{MISSAV_BASE}/videos/{path}"))
                .await
            {
                Ok(html) => {
                    if let Some(m3u8) = extract_packed_m3u8(&html) {
                        return Ok((m3u8, chinese, html));
                    }
                    page_fetched = true;
                }
                Err(e) => fetch_error = Some(e),
            }
        }
        // 页面拿到了却没有播放器脚本 → 「未找到」;全部请求失败 → 末次请求错误
        Err(match fetch_error {
            Some(e) if !page_fetched => e,
            _ => format!("{MISSAV} 未找到播放地址"),
        })
    }

    /// missav 兜底详情(fs1 无此番号或不可用时):播放地址 + 元数据区
    pub(super) async fn detail_missav(&self, code: &str) -> Result<VideoDetail, String> {
        let (m3u8, chinese, html) = self.missav_m3u8(code).await?;
        let meta = parse_missav_meta(&html);
        let lower = code.to_lowercase();
        // 标题统一日语:元数据区「標題」即日语原题;缺失时用 fs1 ?lang=jp
        // 反查(限时,可选),再退回 missav 繁中 og:title
        let title = match meta.title_jp.clone() {
            Some(t) => Some(t),
            None => tokio::time::timeout(super::ENRICH_TIMEOUT, self.fs1_jp_title(&lower))
                .await
                .ok()
                .flatten(),
        }
        .or_else(|| meta.og_title.clone())
        .unwrap_or_else(|| base_code(&lower).to_uppercase());
        let mut detail = VideoDetail::bare(lower, title, m3u8, MISSAV);
        detail.chinese = chinese;
        apply_missav_meta(&mut detail, &meta);
        Ok(detail)
    }

    /// 同番号 missav 详情页的元数据(fs1 线路补齐厂商/女优等)
    pub(super) async fn missav_meta(&self, code: &str) -> Result<MissavMeta, String> {
        let html = self.missav_get(&format!("/{}", base_code(code))).await?;
        Ok(parse_missav_meta(&html))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_missav_meta_blocks() {
        // 元数据区结构取证(2026-11 样本,人名/片名以占位文本代替)
        let html = r#"<meta property="og:title" content="繁中译名 占位">
          <div class="text-secondary"><span>標題:</span> <span class="font-medium">日本語タイトル 占位</span></div>
          <div class="text-secondary"><span>發行日期:</span> <time>2021-02-19</time></div>
          <div class="text-secondary"><span>女優:</span>
            <a href="https://missav.ai/dm180/actresses/%E7%94%B2" class="text-nord13 font-medium">女优甲</a>,
            <a href="https://missav.ai/dm41/actresses/%E4%B9%99" class="text-nord13 font-medium">女优乙</a></div>
          <div class="text-secondary"><span>男優:</span> <a href="https://missav.ai/dm2/actors/%E4%B8%99">男优丙</a></div>
          <div class="text-secondary"><span>類型:</span> <a href="https://missav.ai/chinese-subtitle">中文字幕</a>,
            <a href="https://missav.ai/dm218/genres/%E4%B8%81">类型丁</a></div>
          <div class="text-secondary"><span>系列:</span> <a href="https://missav.ai/ja/series/abc-def">系列戊</a></div>
          <div class="text-secondary"><span>發行商:</span> <a href="https://missav.ai/dm191/makers/s1">S1</a></div>
          <div class="text-secondary"><span>導演:</span> <a href="https://missav.ai/dm6/directors/%E5%B7%B1">导演己</a></div>
          <div class="text-secondary"><span>標籤:</span> <a href="https://missav.ai/dm39/labels/%E5%BA%9A">标签庚</a></div>
          <aside><a href="https://missav.ai/dm9/actresses/%E5%A4%96">推荐区女优</a></aside>"#;
        let meta = parse_missav_meta(html);
        assert_eq!(meta.title_jp.as_deref(), Some("日本語タイトル 占位"));
        assert_eq!(meta.og_title.as_deref(), Some("繁中译名 占位"));
        assert_eq!(meta.release_date.as_deref(), Some("2021-02-19"));
        let names = |k: &str| meta.of(k).into_iter().map(|l| l.name).collect::<Vec<_>>();
        assert_eq!(names("actress"), ["女优甲", "女优乙"], "块外推荐区不得混入");
        assert_eq!(meta.of("actress")[0].id, "甲", "id 应 percent 解码");
        assert_eq!(names("actor"), ["男优丙"]);
        assert_eq!(
            names("genre"),
            ["类型丁"],
            "非路由链接(中文字幕)不算类型条目"
        );
        assert_eq!(meta.of("series")[0].id, "abc-def");
        assert_eq!(names("maker"), ["S1"]);
        assert_eq!(names("director"), ["导演己"]);
        assert_eq!(names("label"), ["标签庚"]);

        let mut detail = VideoDetail::bare("abc-001".into(), String::new(), String::new(), "fs1");
        apply_missav_meta(&mut detail, &meta);
        assert_eq!(detail.studio.as_deref(), Some("S1"));
        assert_eq!(detail.actors, ["女优甲", "女优乙"]);
        assert_eq!(detail.tags, ["类型丁"]);
        assert_eq!(detail.series.len(), 1);
    }

    #[test]
    fn builds_missav_list_urls() {
        assert_eq!(
            missav_list_url("maker", "s1", 1).unwrap(),
            "https://missav.ai/ja/makers/s1"
        );
        assert_eq!(
            missav_list_url("actress", "%E7%94%B2", 2).unwrap(),
            "https://missav.ai/ja/actresses/%E7%94%B2?page=2"
        );
        assert_eq!(
            missav_list_url("series", "abc-def", 1).unwrap(),
            "https://missav.ai/ja/series/abc-def"
        );
        assert!(missav_list_url("unknown", "x", 1).is_err());
        assert!(missav_list_url("maker", "", 1).is_err());
    }

    #[test]
    fn play_paths_keep_leak_suffix() {
        // 播放路径只剥中字后缀:leak 码推导路径仍带 -uncensored-leak
        // (中字优先尝试,有码回退)
        assert_eq!(
            missav_video_paths("FTHTD-242-uncensored-leak"),
            [
                (
                    "fthtd-242-uncensored-leak-chinese-subtitle".to_string(),
                    true
                ),
                ("fthtd-242-uncensored-leak".to_string(), false)
            ]
        );
        assert_eq!(
            missav_video_paths("ssis-960-chinese-subtitle")[1].0,
            "ssis-960"
        );
    }

    #[test]
    fn parses_uncensored_leak_and_short_prefix_codes() {
        // 真实样本(部屋搜索页)特征:无码流出变体每页 3-4 个、组合后缀、
        // 单字母前缀;页面标题以基础番号大写开头(后缀只存在于 href)
        let html = r#"
          <a href="https://missav.ai/fthtd-242-uncensored-leak">
            <p class="x"> FTHTD-242 示例标题一 - 演员甲 </p>
          </a>
          <a href="https://missav.ai/c-2992">
            <p class="x"> C-2992 示例标题二 - 演员乙 </p>
          </a>
          <a href="https://missav.ai/oftr-017-uncensored-leak-chinese-subtitle">
            <p class="x"> OFTR-017 示例标题三 - 演员丙 </p>
          </a>
        "#;
        let cards = parse_missav_search(html);
        let codes: Vec<&str> = cards.iter().map(|c| c.code.as_str()).collect();
        assert_eq!(
            codes,
            [
                "fthtd-242-uncensored-leak",
                "c-2992",
                "oftr-017-uncensored-leak-chinese-subtitle"
            ],
            "leak/组合/单字母前缀形态均应收录: {codes:?}"
        );
        // 标题按基础番号大写匹配(不含后缀)——顺带修复中字卡标题为空的存量问题
        assert_eq!(cards[0].title, "示例标题一");
        assert_eq!(cards[1].title, "示例标题二");
        assert_eq!(cards[2].title, "示例标题三");
        assert!(cards[2].chinese, "组合形态的中字标记应为 true");
        assert!(!cards[0].chinese);
        // 封面按完整 code 构造(missav 图床按完整路径收录)
        assert_eq!(
            cards[0].thumbnail,
            "https://fourhoi.com/fthtd-242-uncensored-leak/cover-t.jpg"
        );
    }

    #[test]
    fn title_matching_edge_cases() {
        // 番号后须紧跟空白;标题少于 5 字符或超 200 字符不收,回退大写番号
        let short = r#"<a href="https://missav.ai/abc-001"></a><p>ABC-001 短</p>"#;
        assert_eq!(parse_missav_search(short)[0].title, "ABC-001");
        let glued = r#"<a href="https://missav.ai/abc-001"></a><p>ABC-0012 示例标题文本</p>"#;
        assert_eq!(parse_missav_search(glued)[0].title, "ABC-001");
        let long = format!(
            r#"<a href="https://missav.ai/abc-001"></a><p>ABC-001 {}</p><p>ABC-001 后备标题文本</p>"#,
            "长".repeat(201)
        );
        assert_eq!(parse_missav_search(&long)[0].title, "后备标题文本");
        // 尾随空白不计入 200 字符上限
        let padded = format!(
            "<a href=\"https://missav.ai/abc-001\"></a><p>ABC-001 {}{}</p>",
            "题".repeat(200),
            " ".repeat(30)
        );
        assert_eq!(parse_missav_search(&padded)[0].title, "题".repeat(200));
    }

    #[test]
    fn variant_dedup_merges_by_priority() {
        // 去重语义(需求 12 用户要求,覆盖更早的「独立保留」裁决):同番号
        // 变体按优先级归并取一(中字>有码>无码>英字);无中字时有码优先于
        // 无码流出
        let html = r#"
          <a href="https://missav.ai/snos-359"><p class="x"> SNOS-359 标题A </p></a>
          <a href="https://missav.ai/snos-359-uncensored-leak"><p class="x"> SNOS-359 标题B </p></a>
          <a href="https://missav.ai/oftr-017-uncensored-leak"><p class="x"> OFTR-017 标题C </p></a>
          <a href="https://missav.ai/oftr-017-uncensored-leak-chinese-subtitle"><p class="x"> OFTR-017 标题D </p></a>
        "#;
        let cards = parse_missav_search(html);
        let codes: Vec<&str> = cards.iter().map(|c| c.code.as_str()).collect();
        assert_eq!(
            codes,
            ["snos-359", "oftr-017-uncensored-leak-chinese-subtitle"],
            "同番号归并取优:有码胜过无码流出,中字胜过一切: {codes:?}"
        );
        assert!(!cards[0].chinese);
        assert!(cards[1].chinese);
    }

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
        let cards = parse_missav_search(html);
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
    fn missav_cards_carry_duration() {
        // 列表页实测结构(文本用占位):封面链接内右下角时长角标
        let html = r#"
          <a href="https://missav.ai/dm63/ja/ssis-960-uncensored-leak" alt="x"><span class="absolute bottom-1 right-1 rounded-lg px-2 py-1 text-xs">2:08:22</span></a>
          <a href="https://missav.ai/dm63/ja/ssis-960-uncensored-leak"><p> SSIS-960 占位标题甲 </p></a>
          <a href="https://missav.ai/ja/snos-486" alt="y"><span class="absolute bottom-1 right-1 rounded-lg">58:07</span></a>
          <a href="https://missav.ai/ja/snos-486"><p> SNOS-486 占位标题乙 </p></a>
          <a href="https://missav.ai/ja/abc-001"><p> ABC-001 占位标题丙 </p></a>
        "#;
        let cards = parse_missav_search(html);
        let dur = |c: &str| {
            cards
                .iter()
                .find(|v| v.code == c)
                .map(|v| v.duration.clone())
        };
        assert_eq!(dur("ssis-960-uncensored-leak").as_deref(), Some("2:08:22"));
        assert_eq!(dur("snos-486").as_deref(), Some("58:07"));
        assert_eq!(
            dur("abc-001").as_deref(),
            Some(""),
            "无角标的卡片时长为空(前端隐藏)"
        );
    }

    #[test]
    fn fallback_captures_leak_codes_whole() {
        // 旧兜底正则只抓基础子串(fthtd-242),丢 -uncensored-leak 后缀
        // 会导致兜底卡点详情找不到 leak 内容;对齐后须收录完整形态
        let html = r#"
          <a href="https://missav.ai/fthtd-242-uncensored-leak">x</a>
          <a href="https://missav.ai/c-2992">y</a>
          <a href="https://missav.ai/dm539/search/spsc-037?page=1">z</a>
        "#;
        let cards = scan_codes_fallback(html);
        let codes: Vec<&str> = cards.iter().map(|c| c.code.as_str()).collect();
        assert_eq!(
            codes,
            ["fthtd-242-uncensored-leak", "c-2992", "spsc-037"],
            "兜底须与主解析同能力集(完整形态、含后缀): {codes:?}"
        );
    }

    #[test]
    fn fallback_and_validator_never_drift() {
        // 漂移防护:任意 href 末段,凡 is_video_code 认可的形态,
        // 兜底扫描都必须能完整抓到(两处共用同一模式定义)
        let samples = [
            "ssis-960",
            "ssis-960-chinese-subtitle",
            "fthtd-242-uncensored-leak",
            "oftr-017-uncensored-leak-chinese-subtitle",
            "c-2992",
            "fc2-ppv-5001211",
        ];
        for code in samples {
            assert!(is_video_code(code), "样本应为合法番号: {code}");
            let html = format!(r#"<a href="https://missav.ai/{code}">x</a>"#);
            let codes: Vec<String> = scan_codes_fallback(&html)
                .into_iter()
                .map(|c| c.code)
                .collect();
            assert_eq!(
                codes,
                [code.to_string()],
                "兜底应完整收录校验器认可的形态: {code}"
            );
        }
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
        let cards = scan_codes_fallback(html);
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
    fn fourhoi_url_detection() {
        assert!(is_fourhoi_url("https://fourhoi.com/spsc-037/cover-t.jpg"));
        assert!(is_fourhoi_url("https://cdn.fourhoi.com/x/cover.webp"));
        assert!(!is_fourhoi_url("https://fs1.app/images/cover.jpg"));
        assert!(!is_fourhoi_url(
            "https://evil.example/fourhoi.com/cover.jpg"
        ));
        assert!(!is_fourhoi_url("不是 URL"));
    }

    #[test]
    fn follows_meta_refresh() {
        let html = r#"<html><head><meta charset="UTF-8" /><meta http-equiv="refresh" content="0;url='https://missav.ai/dm82/ssis-960-chinese-subtitle'" /></head></html>"#;
        let next = meta_refresh_url(html, "https://missav.ai/dm539/ssis-960-chinese-subtitle")
            .expect("应提取 meta-refresh 地址");
        assert_eq!(next, "https://missav.ai/dm82/ssis-960-chinese-subtitle");
        assert!(meta_refresh_url("<p>无跳转</p>", "https://missav.ai/").is_none());
    }
}
