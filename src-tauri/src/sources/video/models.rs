//! 视频板块 IPC 模型(序列化 camelCase,与 `src/types/index.ts` 一致)。

use crate::models::PaginationInfo;

/// 影片元数据中的一个可跳转条目(厂商/女优/男优/系列/导演/标签/类型)。
/// kind + id 即 missav 列表页 /{路由}/{id},用于「相似影片」与关注
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MetaLink {
    /// maker / actress / actor / series / director / label / genre
    pub kind: String,
    /// 路由末段(已 percent 解码)
    pub id: String,
    pub name: String,
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
    /// 视频时长(fs1 卡片右下遮罩 label 如 "2:51:49";missav 卡片无此
    /// 数据为空字符串)
    #[serde(default)]
    pub duration: String,
}

impl VideoCard {
    /// 仅有番号的卡片(missav 搜索/兜底扫描):封面按 fourhoi 图床
    /// 规律 `fourhoi.com/{code}/cover-t.jpg` 构造,无观看/点赞/时长
    pub(super) fn from_code(code: String, title: String) -> Self {
        Self {
            thumbnail: format!("https://fourhoi.com/{code}/cover-t.jpg"),
            chinese: code.ends_with("-chinese-subtitle"),
            code,
            title,
            views: String::new(),
            likes: String::new(),
            duration: String::new(),
        }
    }
}

/// 关注目标的可选条目(列表页提取;target_id 用于 follow/works)
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowTargetItem {
    pub target_id: String,
    pub display_name: String,
    /// 作品数(站点有的话;可选)
    pub work_count: Option<i64>,
}

/// 视频列表响应
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoListResult {
    pub items: Vec<VideoCard>,
    pub pagination: PaginationInfo,
}

impl VideoListResult {
    /// 分页包装:`page` 从 1 开始(IPC 约定),`current` 为 0 基;
    /// 站点均不给总页数(total 恒 0),前端靠 hasNext 翻页
    pub(super) fn paged(items: Vec<VideoCard>, page: i32, has_next: bool) -> Self {
        Self {
            items,
            pagination: PaginationInfo {
                current: page - 1,
                total: 0,
                has_next,
                has_prev: page > 1,
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
    /// 发行厂商(missav 详情页 makers 链接;fs1 详情页无此元数据为 None)
    #[serde(default)]
    pub studio: Option<String>,
    /// 女优名列表(兼容字段,与 actresses 同源)
    #[serde(default)]
    pub actors: Vec<String>,
    /// 以下来自 missav 元数据区(fs1 线路按同番号从 missav 补齐)
    #[serde(default)]
    pub makers: Vec<MetaLink>,
    #[serde(default)]
    pub actresses: Vec<MetaLink>,
    /// 男优
    #[serde(default)]
    pub male_actors: Vec<MetaLink>,
    #[serde(default)]
    pub series: Vec<MetaLink>,
    #[serde(default)]
    pub directors: Vec<MetaLink>,
    #[serde(default)]
    pub labels: Vec<MetaLink>,
    #[serde(default)]
    pub release_date: Option<String>,
}

impl VideoDetail {
    /// 只有基础字段、元数据留空的详情(元数据随后由 missav 补齐)
    pub(super) fn bare(code: String, title: String, m3u8: String, provider: &str) -> Self {
        Self {
            code,
            title,
            tags: Vec::new(),
            m3u8,
            provider: provider.to_string(),
            chinese: false,
            studio: None,
            actors: Vec::new(),
            makers: Vec::new(),
            actresses: Vec::new(),
            male_actors: Vec::new(),
            series: Vec::new(),
            directors: Vec::new(),
            labels: Vec::new(),
            release_date: None,
        }
    }
}
