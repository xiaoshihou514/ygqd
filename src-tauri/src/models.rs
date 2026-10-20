use serde::{Deserialize, Serialize};

/// 漫画源标识。IPC 与 SQLite 中均以小写字符串("niacg"/"bika")表示
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Niacg,
    Bika,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Source::Niacg => "niacg",
            Source::Bika => "bika",
        })
    }
}

impl rusqlite::ToSql for Source {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        Ok(rusqlite::types::ToSqlOutput::Owned(
            rusqlite::types::Value::Text(self.to_string()),
        ))
    }
}

impl rusqlite::types::FromSql for Source {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        let text = value.as_str()?;
        match text {
            "niacg" => Ok(Source::Niacg),
            "bika" => Ok(Source::Bika),
            other => Err(rusqlite::types::FromSqlError::Other(
                format!("未知的漫画源 {other:?}").into(),
            )),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComicItem {
    pub source: Source,
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub author: String,
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
    pub source: Source,
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
    /// 当前账号是否已点赞/收藏(哔咔服务器状态;niacg 恒 false)
    #[serde(default)]
    pub is_liked: bool,
    #[serde(default)]
    pub is_favourite: bool,
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
    pub source: Source,
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
    pub source: Source,
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteEntry {
    pub source: Source,
    pub comic_id: String,
    pub title: String,
    pub thumbnail: String,
    pub category_id: i32,
    pub author: String,
    pub created_at: i64,
}

/// 收藏写入参数(时间戳由后端补全)
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordFavorite {
    pub source: Source,
    pub comic_id: String,
    pub title: String,
    pub thumbnail: String,
    pub category_id: i32,
    pub author: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// IPC 与 SQLite 都以既有的小写串表示,序列化值不可变动
    #[test]
    fn source_serializes_to_stable_lowercase_strings() {
        assert_eq!(serde_json::to_string(&Source::Niacg).unwrap(), "\"niacg\"");
        assert_eq!(serde_json::to_string(&Source::Bika).unwrap(), "\"bika\"");
        assert_eq!(
            serde_json::from_str::<Source>("\"niacg\"").unwrap(),
            Source::Niacg
        );
        assert_eq!(
            serde_json::from_str::<Source>("\"bika\"").unwrap(),
            Source::Bika
        );
        assert!(serde_json::from_str::<Source>("\"xxx\"").is_err());
        assert_eq!(Source::Bika.to_string(), "bika");
    }
}
