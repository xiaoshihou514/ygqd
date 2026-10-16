use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComicItem {
    pub source: String,
    pub id: String,
    pub title: String,
    pub thumbnail: String,
    pub category: String,
    pub category_id: i32,
    pub tags: Vec<String>,
    pub likes: String,
    pub link: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComicDetail {
    pub source: String,
    pub id: String,
    pub title: String,
    pub thumbnail: String,
    pub category: String,
    pub category_id: i32,
    pub author: String,
    pub works: Vec<String>,
    pub characters: Vec<String>,
    pub tags: Vec<String>,
    pub likes: String,
    pub episodes: Vec<Episode>,
    pub images: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub order: i32,
    pub title: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComicImagesResult {
    pub images: Vec<String>,
    pub pagination: PaginationInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationInfo {
    pub current: i32,
    pub total: i32,
    pub has_next: bool,
    pub has_prev: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub items: Vec<ComicItem>,
    pub pagination: PaginationInfo,
    pub page_url_template: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryListResult {
    pub items: Vec<ComicItem>,
    pub pagination: PaginationInfo,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowedAuthor {
    pub author: String,
    pub followed_at: i64,
    pub last_checked_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewHistoryEntry {
    pub source: String,
    pub comic_id: String,
    pub title: String,
    pub thumbnail: String,
    pub category_id: i32,
    pub author: String,
    pub viewed_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlacklistEntry {
    pub tag: String,
    pub mode: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordHistory {
    pub source: String,
    pub comic_id: String,
    pub title: String,
    pub thumbnail: String,
    pub category_id: i32,
    pub author: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BikaAccountStatus {
    pub logged_in: bool,
    pub email: String,
}
