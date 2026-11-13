//! 番号形态与同番号变体归并(fs1/missav 两源共用)。

use super::models::VideoCard;
use regex::Regex;
use std::collections::{hash_map::Entry, HashMap};
use std::sync::LazyLock;

/// 去重键:剥离全部已知变体后缀(中字/英字/无码/流出),循环剥离
/// 应对叠加形态(如 -uncensored-leak-chinese-subtitle)。
/// 仅用于同番号变体归并(dedupe_by_base/parse_missav_search 去重);
/// 播放路径推导用 play_paths(保留 leak 后缀),详情/标题回退用 base_code。
pub(super) fn dedup_key(code: &str) -> &str {
    let mut key = code;
    loop {
        // N1 修正:站点真实后缀是 -leak(样本统计 62:0,无 -leaked 形态)
        let stripped = key
            .strip_suffix("-chinese-subtitle")
            .or_else(|| key.strip_suffix("-english-subtitle"))
            .or_else(|| key.strip_suffix("-uncensored-leak"))
            .or_else(|| key.strip_suffix("-uncensored"))
            .or_else(|| key.strip_suffix("-leak"));
        match stripped {
            Some(s) if !s.is_empty() => key = s,
            _ => break,
        }
    }
    key
}

/// 基础番号:剥离全部变体后缀(详情页 URL/标题回退/日语反查三处
/// 的番号归一用途)
pub(super) fn base_code(code: &str) -> &str {
    dedup_key(code)
}

/// 变体优先级(需求 12 用户要求):中文字幕 > 有码 > 无码 > 英文字幕。
/// 数值小者优先。
fn variant_priority(card: &VideoCard) -> u8 {
    let c = &card.code;
    if c.contains("chinese-subtitle") {
        0
    } else if c.contains("english-subtitle") {
        3
    } else if c.contains("uncensored") {
        2
    } else {
        1
    }
}

/// 同番号变体去重:按基础番号分组,组内取优先级最高者(同级保留首现);
/// 输出保持基础番号的首次出现顺序。
pub(super) fn dedupe_by_base(cards: Vec<VideoCard>) -> Vec<VideoCard> {
    let mut slots: Vec<(u8, VideoCard)> = Vec::with_capacity(cards.len());
    let mut index: HashMap<String, usize> = HashMap::with_capacity(cards.len());
    for card in cards {
        let pri = variant_priority(&card);
        match index.entry(dedup_key(&card.code).to_owned()) {
            Entry::Occupied(slot) => {
                let best = &mut slots[*slot.get()];
                if pri < best.0 {
                    *best = (pri, card);
                }
            }
            Entry::Vacant(slot) => {
                slot.insert(slots.len());
                slots.push((pri, card));
            }
        }
    }
    slots.into_iter().map(|(_, card)| card).collect()
}

/// 多源合并后的 has_next:任一源本页还有货即 true。
/// fs1 按满页判定(每页 24 卡);missav dm539 每页仅 12 卡且末页可能
/// 不满,按「本页非空」判定——末页有余卡时会多一次空翻页,可接受。
pub(super) fn merged_has_next(fs1_full_page: bool, missav_nonempty: bool) -> bool {
    fs1_full_page || missav_nonempty
}

/// 番号模式(主解析校验与兜底扫描共用的唯一定义,避免两处能力漂移):
/// 1-6 字母前缀 + 2-5 位数字,可选无码流出(-uncensored-leak)与中字
/// (-chinese-subtitle)后缀含组合;另收 fc2 短码形态。无锚定义——
/// 完整匹配用于校验(is_video_code),子串匹配用于兜底 href 扫描。
pub(super) static VIDEO_CODE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"[a-z]{1,6}-\d{2,5}",
        r"(-uncensored-leak(-chinese-subtitle)?|-chinese-subtitle)?",
        r"|fc2-ppv-\d+"
    ))
    .expect("static regex")
});

/// 番号形态完整匹配(等价 ^模式$):匹配须从串首到串尾
pub(super) fn is_video_code(code: &str) -> bool {
    VIDEO_CODE
        .find(code)
        .is_some_and(|m| m.start() == 0 && m.end() == code.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(code: &str) -> VideoCard {
        VideoCard::from_code(code.into(), String::new())
    }

    #[test]
    fn base_code_strips_all_variants() {
        assert_eq!(base_code("mida-829"), "mida-829");
        assert_eq!(base_code("mida-829-chinese-subtitle"), "mida-829");
        assert_eq!(base_code("mida-829-uncensored"), "mida-829");
        assert_eq!(base_code("mida-829-english-subtitle"), "mida-829");
        assert_eq!(base_code("mida-829-uncensored-leak"), "mida-829");
        assert_eq!(base_code("mida-829-leak"), "mida-829");
    }

    #[test]
    fn base_code_dedup_semantics_unchanged() {
        // N1 修正后语义:剥全部变体后缀(含 -leak,样本统计 62:0 为站点
        // 真实形态)——同番号的无码/流出/中字变体在去重时归并为一组
        assert_eq!(base_code("fthtd-242"), "fthtd-242");
        assert_eq!(base_code("fthtd-242-uncensored-leak"), "fthtd-242");
        assert_eq!(base_code("ssis-960-chinese-subtitle"), "ssis-960");
        assert_eq!(
            base_code("oftr-017-uncensored-leak-chinese-subtitle"),
            "oftr-017",
            "叠加形态(无码流出+中字)循环剥离应归并到基础番号"
        );
    }

    #[test]
    fn dedupe_prefers_chinese_then_default() {
        // 中字优先于 fs1 首现的有码
        let out = dedupe_by_base(vec![card("mida-829"), card("mida-829-chinese-subtitle")]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].code, "mida-829-chinese-subtitle");
        // 有码优先于无码与英字
        let out = dedupe_by_base(vec![
            card("ssis-001-uncensored"),
            card("ssis-001-english-subtitle"),
            card("ssis-001"),
        ]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].code, "ssis-001");
        // 不同番号不去重,保持首现顺序(组内替换不改变组的位置)
        let out = dedupe_by_base(vec![
            card("aaa-001"),
            card("bbb-002"),
            card("aaa-001-chinese-subtitle"),
        ]);
        let codes: Vec<&str> = out.iter().map(|c| c.code.as_str()).collect();
        assert_eq!(codes, ["aaa-001-chinese-subtitle", "bbb-002"]);
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
    fn video_code_validation_is_anchored() {
        assert!(is_video_code("ssis-960"));
        assert!(is_video_code("oftr-017-uncensored-leak-chinese-subtitle"));
        assert!(is_video_code("fc2-ppv-5001211"));
        assert!(!is_video_code("dm539"));
        assert!(!is_video_code("search-spsc-037"), "须从串首匹配");
        assert!(!is_video_code("ssis-960-english-subtitle"), "须匹配到串尾");
    }
}
