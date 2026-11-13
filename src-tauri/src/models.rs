//! IPC 共享数据模型(与前端 `src/types/index.ts` 一一对应)。
//!
//! 字段经 `rename_all = "camelCase"` 序列化,线上字段名即契约,不可改名。

use serde::{Deserialize, Serialize};

/// 漫画源标识。IPC 与 SQLite 中均以小写字符串("niacg"/"bika")表示
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Niacg,
    Bika,
}

impl Source {
    /// 稳定的小写标识(IPC、SQLite 与 Display 共用)
    pub const fn as_str(self) -> &'static str {
        match self {
            Source::Niacg => "niacg",
            Source::Bika => "bika",
        }
    }
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl rusqlite::ToSql for Source {
    fn to_sql(&self) -> rusqlite::Result<rusqlite::types::ToSqlOutput<'_>> {
        // 静态串借用绑定,免每次绑定参数分配 String
        Ok(rusqlite::types::ToSqlOutput::Borrowed(
            rusqlite::types::ValueRef::Text(self.as_str().as_bytes()),
        ))
    }
}

impl rusqlite::types::FromSql for Source {
    fn column_result(value: rusqlite::types::ValueRef<'_>) -> rusqlite::types::FromSqlResult<Self> {
        match value.as_str()? {
            "niacg" => Ok(Source::Niacg),
            "bika" => Ok(Source::Bika),
            other => Err(rusqlite::types::FromSqlError::Other(
                format!("未知的漫画源 {other:?}").into(),
            )),
        }
    }
}

/// 列表/搜索结果中的一部漫画(卡片数据)
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

/// 漫画详情页数据
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

/// 漫画章节
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub order: i32,
    pub title: String,
}

/// 一页章节图片及其分页
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComicImagesResult {
    pub images: Vec<String>,
    pub pagination: PaginationInfo,
}

/// 分页信息
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginationInfo {
    pub current: i32,
    pub total: i32,
    pub has_next: bool,
    pub has_prev: bool,
}

/// 搜索结果页
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub items: Vec<ComicItem>,
    pub pagination: PaginationInfo,
    pub page_url_template: Option<String>,
}

/// 分类列表页
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryListResult {
    pub items: Vec<ComicItem>,
    pub pagination: PaginationInfo,
}

/// 已关注的哔咔作者(存于 follow_target;last_checked_at 与 followed_at 同值,兼容旧接口)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowedAuthor {
    pub author: String,
    pub followed_at: i64,
    pub last_checked_at: i64,
}

/// 漫画浏览记录(view_history 行)
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

/// 标签黑名单条目(mode 区分屏蔽方式)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlacklistEntry {
    pub tag: String,
    pub mode: String,
    pub created_at: i64,
}

/// 浏览记录写入参数(时间戳由后端补全)
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

/// 哔咔登录状态(不含密码与 token)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BikaAccountStatus {
    pub logged_in: bool,
    pub email: String,
}

/// 本地收藏条目(favorites 行)
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
    use rusqlite::{types::FromSql, ToSql};

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

    /// SQLite 绑定/读取与序列化使用同一组字符串
    #[test]
    fn source_sql_roundtrip_matches_wire_strings() {
        for source in [Source::Niacg, Source::Bika] {
            let out = source.to_sql().unwrap();
            let rusqlite::types::ToSqlOutput::Borrowed(value) = out else {
                panic!("expected borrowed text");
            };
            assert_eq!(value.as_str().unwrap(), source.as_str());
            assert_eq!(Source::column_result(value).unwrap(), source);
        }
        assert!(Source::column_result(rusqlite::types::ValueRef::Text(b"x")).is_err());
    }

    /// 线上字段名是前端契约(src/types/index.ts),逐字锁定
    #[test]
    fn wire_field_names_are_camel_case() {
        let entry = FavoriteEntry {
            source: Source::Bika,
            comic_id: "c".into(),
            title: "t".into(),
            thumbnail: "th".into(),
            category_id: 1,
            author: "a".into(),
            created_at: 2,
        };
        assert_eq!(
            serde_json::to_string(&entry).unwrap(),
            r#"{"source":"bika","comicId":"c","title":"t","thumbnail":"th","categoryId":1,"author":"a","createdAt":2}"#
        );
        let pagination = PaginationInfo {
            current: 1,
            total: 3,
            has_next: true,
            has_prev: false,
        };
        assert_eq!(
            serde_json::to_string(&pagination).unwrap(),
            r#"{"current":1,"total":3,"hasNext":true,"hasPrev":false}"#
        );
        let followed = FollowedAuthor {
            author: "a".into(),
            followed_at: 1,
            last_checked_at: 1,
        };
        assert_eq!(
            serde_json::to_string(&followed).unwrap(),
            r#"{"author":"a","followedAt":1,"lastCheckedAt":1}"#
        );
    }
}
