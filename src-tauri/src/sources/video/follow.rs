//! 视频关注:可关注目标列表(fs1 女优 / missav 厂商 / missav 女优排行)
//! 与关注对象的作品流。

use super::codes::dedupe_by_base;
use super::html::strip_tags;
use super::missav::{missav_list_url, parse_missav_search};
use super::models::{FollowTargetItem, VideoListResult};
use super::VideoApi;
use crate::sources::shared::percent_decode_simple;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

/// fs1 女优卡片块:链接 hash slug + 块内 h6.title 名字
static FS1_MODEL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"<a[^>]*href="https://fs1\.app/models/([a-f0-9]+)/"[\s\S]*?<h6 class="title">([^<]+)</h6>"#,
    )
    .expect("static regex")
});
/// missav 厂商:相对链接 slug + 文本名
static MISSAV_MAKER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"<a[^>]*href="(/makers/[^"]+)"[^>]*>([\s\S]{0,120}?)</a>"#).expect("static regex")
});
/// missav 女优:dm{N}/actresses/{percent-name},卡片文本 text-nord13
static MISSAV_ACTRESS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"<a[^>]*href="[^"]*/actresses/([^"]+)"[^>]*class="text-nord13"[^>]*>([\s\S]{0,120}?)</a>"#,
    )
    .expect("static regex")
});

/// missav 列表每视频双链接(封面+标题),每页 12 链接去重后实为 6 部
const MISSAV_FOLLOW_PAGE_SIZE: usize = 6;

/// 按 target_id 去重收集(空 id/空名跳过)
fn collect_targets(pairs: impl Iterator<Item = (String, String)>) -> Vec<FollowTargetItem> {
    let mut seen = HashSet::new();
    pairs
        .filter(|(id, name)| !id.is_empty() && !name.is_empty() && seen.insert(id.clone()))
        .map(|(target_id, display_name)| FollowTargetItem {
            target_id,
            display_name,
            work_count: None,
        })
        .collect()
}

fn parse_fs1_models(html: &str) -> Vec<FollowTargetItem> {
    collect_targets(
        FS1_MODEL
            .captures_iter(html)
            .map(|c| (c[1].to_string(), c[2].trim().to_string())),
    )
}

fn parse_missav_makers(html: &str) -> Vec<FollowTargetItem> {
    collect_targets(MISSAV_MAKER.captures_iter(html).map(|c| {
        (
            c[1].trim_start_matches("/makers/").to_string(),
            strip_tags(&c[2]),
        )
    }))
}

/// target_id 存 percent 解码后的名字(直连 /actresses/{name} 可达)
fn parse_missav_actresses(html: &str) -> Vec<FollowTargetItem> {
    collect_targets(
        MISSAV_ACTRESS
            .captures_iter(html)
            .map(|c| (percent_decode_simple(&c[1]), strip_tags(&c[2]))),
    )
}

impl VideoApi {
    /// fs1 女优列表:/models/ 页卡片(链接 hash slug + h6.title 名字)
    pub async fn fs1_models_list(&self) -> Result<Vec<FollowTargetItem>, String> {
        Ok(parse_fs1_models(&self.get("/models/").await?))
    }

    /// fs1 女优作品流:/models/{hash}/ 的视频卡片,复用卡片网格解析;
    /// Jable 系分页 /{path}/{page}/
    pub async fn fs1_model_works(&self, slug: &str, page: i32) -> Result<VideoListResult, String> {
        let path = if page <= 1 {
            format!("/models/{slug}/")
        } else {
            format!("/models/{slug}/{page}/")
        };
        self.fs1_cards_page(&path, page).await
    }

    /// missav 厂商列表:/makers 页(相对链接 slug + 文本名)
    pub async fn missav_makers_list(&self) -> Result<Vec<FollowTargetItem>, String> {
        Ok(parse_missav_makers(&self.missav_get("/makers").await?))
    }

    /// missav 女优列表:/actresses/ranking 页(dm 前缀路由 + percent 名字)
    pub async fn missav_actresses_list(&self) -> Result<Vec<FollowTargetItem>, String> {
        Ok(parse_missav_actresses(
            &self.missav_get("/actresses/ranking").await?,
        ))
    }

    /// missav 厂商/女优等条目作品流:/ja/{路由}/{id},复用搜索页解析;
    /// 分页 ?page=N
    pub async fn missav_follow_works(
        &self,
        kind: &str,
        target_id: &str,
        page: i32,
    ) -> Result<VideoListResult, String> {
        let url = missav_list_url(kind, target_id, page)?;
        let html = self.missav_get(&url).await?;
        // 自查补漏(问题 2):作品列表与搜索同构,变体去重一并生效
        let items = dedupe_by_base(parse_missav_search(&html));
        // 问题 4 修复:阈值按去重后部数判满页(此前按链接数 12 判,
        // has_next 恒 false →「只有 6 部」)
        let has_next = items.len() >= MISSAV_FOLLOW_PAGE_SIZE;
        Ok(VideoListResult::paged(items, page, has_next))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_follow_target_lists() {
        let fs1 = r#"
          <a href="https://fs1.app/models/abc123/"><img></a><h6 class="title"> 女优甲 </h6>
          <a href="https://fs1.app/models/abc123/"><img></a><h6 class="title">重复</h6>
          <a href="https://fs1.app/models/def456/"><img></a><h6 class="title">女优乙</h6>"#;
        let models = parse_fs1_models(fs1);
        let ids: Vec<(&str, &str)> = models
            .iter()
            .map(|m| (m.target_id.as_str(), m.display_name.as_str()))
            .collect();
        assert_eq!(ids, [("abc123", "女优甲"), ("def456", "女优乙")]);

        let makers = parse_missav_makers(
            r#"<a href="/makers/s1"><span> S1 </span>NO.1</a><a href="/makers/empty"> </a>"#,
        );
        assert_eq!(makers.len(), 1);
        assert_eq!(makers[0].target_id, "s1");
        assert_eq!(makers[0].display_name, "S1 NO.1");

        let actresses = parse_missav_actresses(
            r#"<a href="https://missav.ai/dm41/actresses/%E7%94%B2" class="text-nord13">女优甲</a>"#,
        );
        assert_eq!(actresses[0].target_id, "甲");
        assert_eq!(actresses[0].display_name, "女优甲");
    }
}
