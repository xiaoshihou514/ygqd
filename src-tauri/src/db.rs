use crate::models::*;
use rusqlite::{params, Connection};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
pub fn open(path: &std::path::Path) -> Result<Connection, String> {
    let db = Connection::open(path).map_err(|e| e.to_string())?;
    db.execute_batch("PRAGMA journal_mode=WAL;
      CREATE TABLE IF NOT EXISTS followed_authors(author TEXT PRIMARY KEY, followed_at INTEGER NOT NULL, last_checked_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS follow_target(source TEXT NOT NULL, kind TEXT NOT NULL, target_id TEXT NOT NULL, display_name TEXT NOT NULL, followed_at INTEGER NOT NULL, PRIMARY KEY(source, kind, target_id));
      CREATE TABLE IF NOT EXISTS item_history(source TEXT NOT NULL, kind TEXT NOT NULL, item_id TEXT NOT NULL, title TEXT NOT NULL, viewed_at INTEGER NOT NULL, progress TEXT, PRIMARY KEY(source, kind, item_id));
      CREATE TABLE IF NOT EXISTS novel_blacklist(keyword TEXT PRIMARY KEY, created_at INTEGER NOT NULL);
      CREATE INDEX IF NOT EXISTS idx_item_history_viewed ON item_history(viewed_at DESC);
      INSERT OR IGNORE INTO follow_target(source,kind,target_id,display_name,followed_at)
        SELECT 'bika','author',author,author,followed_at FROM followed_authors;
      CREATE TABLE IF NOT EXISTS view_history(comic_id TEXT NOT NULL, title TEXT NOT NULL, thumbnail TEXT NOT NULL, category_id INTEGER NOT NULL, author TEXT NOT NULL, viewed_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS tag_blacklist(tag TEXT PRIMARY KEY, mode TEXT NOT NULL, created_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS bika_account(id INTEGER PRIMARY KEY CHECK(id=1), email TEXT NOT NULL, password TEXT NOT NULL, token TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS bika_address(id INTEGER PRIMARY KEY CHECK(id=1), address TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS favorites(source TEXT NOT NULL, comic_id TEXT NOT NULL, title TEXT NOT NULL, thumbnail TEXT NOT NULL, category_id INTEGER NOT NULL, author TEXT NOT NULL, created_at INTEGER NOT NULL, PRIMARY KEY(source, comic_id));
      CREATE TABLE IF NOT EXISTS novel_series(id TEXT PRIMARY KEY, title TEXT NOT NULL, author TEXT NOT NULL, chapters TEXT NOT NULL, added_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS novel_progress(series_id TEXT NOT NULL, chapter_order INTEGER NOT NULL, page INTEGER NOT NULL, updated_at INTEGER NOT NULL, PRIMARY KEY(series_id));
      CREATE TABLE IF NOT EXISTS video_watched(code TEXT PRIMARY KEY, watched_at INTEGER NOT NULL);").map_err(|e| e.to_string())?;
    migrate_view_history(&db)?;
    Ok(db)
}

/// 旧版单源 view_history 迁移为带 source 列的复合主键表,旧行视为 niacg
fn migrate_view_history(db: &Connection) -> Result<(), String> {
    let has_source: bool = {
        let mut statement = db
            .prepare("SELECT COUNT(*) FROM pragma_table_info('view_history') WHERE name='source'")
            .map_err(|e| e.to_string())?;
        let count: i64 = statement
            .query_row([], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        count > 0
    };
    if has_source {
        return Ok(());
    }
    db.execute_batch(
        "BEGIN;
        CREATE TABLE view_history_migrated(source TEXT NOT NULL, comic_id TEXT NOT NULL, title TEXT NOT NULL, thumbnail TEXT NOT NULL, category_id INTEGER NOT NULL, author TEXT NOT NULL, viewed_at INTEGER NOT NULL, PRIMARY KEY(source, comic_id));
        INSERT INTO view_history_migrated SELECT 'niacg', comic_id, title, thumbnail, category_id, author, viewed_at FROM view_history;
        DROP TABLE view_history;
        ALTER TABLE view_history_migrated RENAME TO view_history;
        COMMIT;",
    )
    .map_err(|e| e.to_string())?;
    // 存量 item_history 表补 thumbnail 列(契约扩展;SQLite ALTER 非幂等,
    // 重复执行的 duplicate column 错误静默忽略)
    let _ = db.execute("ALTER TABLE item_history ADD COLUMN thumbnail TEXT", []);
    Ok(())
}

pub fn follows(db: &Connection) -> Result<Vec<FollowedAuthor>, String> {
    // 批次 4 迁移:实现切换到 follow_target 统一表(接口 FollowedAuthor 不变;
    // last_checked_at 由 followed_at 兼容填充)
    let mut s=db.prepare("SELECT target_id,followed_at,followed_at FROM follow_target WHERE source='bika' AND kind='author' ORDER BY followed_at DESC").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(FollowedAuthor {
                author: r.get(0)?,
                followed_at: r.get(1)?,
                last_checked_at: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}
pub fn follow(db: &Connection, author: &str) -> Result<FollowedAuthor, String> {
    let time = now();
    follow_target_add(db, "bika", "author", author, author)?;
    // followed_authors 双写保留(回滚安全;批次 4 收口后移除)
    db.execute("INSERT INTO followed_authors VALUES(?1,?2,?2) ON CONFLICT(author) DO UPDATE SET last_checked_at=?2",params![author,time]).map_err(|e|e.to_string())?;
    Ok(FollowedAuthor {
        author: author.into(),
        followed_at: time,
        last_checked_at: time,
    })
}
pub fn unfollow(db: &Connection, author: &str) -> Result<(), String> {
    follow_target_remove(db, "bika", "author", author)?;
    db.execute("DELETE FROM followed_authors WHERE author=?1", [author])
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn history(db: &Connection, limit: i64) -> Result<Vec<ViewHistoryEntry>, String> {
    let mut s=db.prepare("SELECT source,comic_id,title,thumbnail,category_id,author,viewed_at FROM view_history ORDER BY viewed_at DESC LIMIT ?1").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([limit], |r| {
            Ok(ViewHistoryEntry {
                source: r.get(0)?,
                comic_id: r.get(1)?,
                title: r.get(2)?,
                thumbnail: r.get(3)?,
                category_id: r.get(4)?,
                author: r.get(5)?,
                viewed_at: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}
pub fn record(db: &Connection, v: RecordHistory) -> Result<(), String> {
    db.execute("INSERT INTO view_history VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(source,comic_id) DO UPDATE SET title=?3,thumbnail=?4,category_id=?5,author=?6,viewed_at=?7",params![v.source,v.comic_id,v.title,v.thumbnail,v.category_id,v.author,now()]).map_err(|e|e.to_string())?;
    Ok(())
}
pub struct BikaAccount {
    pub email: String,
    pub password: String,
    pub token: String,
}
pub fn bika_account(db: &Connection) -> Result<Option<BikaAccount>, String> {
    let mut statement = db
        .prepare("SELECT email,password,token FROM bika_account WHERE id=1")
        .map_err(|e| e.to_string())?;
    let mut rows = statement.query([]).map_err(|e| e.to_string())?;
    rows.next()
        .map_err(|e| e.to_string())?
        .map(|row| {
            Ok(BikaAccount {
                email: row.get(0)?,
                password: row.get(1)?,
                token: row.get(2)?,
            })
        })
        .transpose()
        .map_err(|e: rusqlite::Error| e.to_string())
}
pub fn save_bika_account(
    db: &Connection,
    email: &str,
    password: &str,
    token: &str,
) -> Result<(), String> {
    db.execute(
        "INSERT INTO bika_account VALUES(1,?1,?2,?3) ON CONFLICT(id) DO UPDATE SET email=?1,password=?2,token=?3",
        params![email, password, token],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn clear_bika_account(db: &Connection) -> Result<(), String> {
    db.execute("DELETE FROM bika_account WHERE id=1", [])
        .map_err(|e| e.to_string())?;
    Ok(())
}
/// 哔咔分流地址;空字符串表示不分流(用系统 DNS)
pub fn bika_address(db: &Connection) -> Result<String, String> {
    db.query_row("SELECT address FROM bika_address WHERE id=1", [], |row| {
        row.get(0)
    })
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(String::new()),
        other => Err(other),
    })
    .map_err(|e: rusqlite::Error| e.to_string())
}
pub fn save_bika_address(db: &Connection, address: &str) -> Result<(), String> {
    db.execute(
        "INSERT INTO bika_address VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET address=?1",
        params![address],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn blacklist(db: &Connection) -> Result<Vec<BlacklistEntry>, String> {
    let mut s = db
        .prepare("SELECT tag,mode,created_at FROM tag_blacklist ORDER BY tag")
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(BlacklistEntry {
                tag: r.get(0)?,
                mode: r.get(1)?,
                created_at: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}
pub fn add_blacklist(db: &Connection, tag: &str, mode: &str) -> Result<BlacklistEntry, String> {
    let created_at = now();
    db.execute(
        "INSERT OR IGNORE INTO tag_blacklist VALUES(?1,?2,?3)",
        params![tag, mode, created_at],
    )
    .map_err(|e| e.to_string())?;
    Ok(BlacklistEntry {
        tag: tag.into(),
        mode: mode.into(),
        created_at,
    })
}
pub fn remove_blacklist(db: &Connection, tag: &str) -> Result<(), String> {
    db.execute("DELETE FROM tag_blacklist WHERE tag=?1", [tag])
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn update_blacklist(db: &Connection, tag: &str, mode: &str) -> Result<(), String> {
    db.execute(
        "UPDATE tag_blacklist SET mode=?2 WHERE tag=?1",
        params![tag, mode],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn favorites(db: &Connection) -> Result<Vec<FavoriteEntry>, String> {
    let mut s = db
        .prepare("SELECT source,comic_id,title,thumbnail,category_id,author,created_at FROM favorites ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(FavoriteEntry {
                source: r.get(0)?,
                comic_id: r.get(1)?,
                title: r.get(2)?,
                thumbnail: r.get(3)?,
                category_id: r.get(4)?,
                author: r.get(5)?,
                created_at: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// 插入或刷新收藏(同步服务器列表时也走这里,保留已有 created_at)
pub fn add_favorite(db: &Connection, v: &RecordFavorite) -> Result<(), String> {
    db.execute(
        "INSERT INTO favorites VALUES(?1,?2,?3,?4,?5,?6,?7) ON CONFLICT(source,comic_id) DO UPDATE SET title=?3,thumbnail=?4,category_id=?5,author=?6",
        params![v.source, v.comic_id, v.title, v.thumbnail, v.category_id, v.author, now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn remove_favorite(db: &Connection, source: Source, comic_id: &str) -> Result<(), String> {
    db.execute(
        "DELETE FROM favorites WHERE source=?1 AND comic_id=?2",
        params![source, comic_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn is_favorite(db: &Connection, source: Source, comic_id: &str) -> Result<bool, String> {
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM favorites WHERE source=?1 AND comic_id=?2",
            params![source, comic_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(count > 0)
}

pub fn novel_shelf(db: &Connection) -> Result<Vec<crate::models::NovelSeries>, String> {
    let mut s = db
        .prepare(
            "SELECT id,title,author,chapters,added_at FROM novel_series ORDER BY added_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(crate::models::NovelSeries {
                id: r.get(0)?,
                title: r.get(1)?,
                author_id: String::new(),
                author: r.get(2)?,
                // cover/introduction 暂不入库(hlib 站点不提供封面;简介在
                // 详情页实时获取),书架恢复时为空值
                cover: String::new(),
                introduction: String::new(),
                chapters: serde_json::from_str(&r.get::<_, String>(3)?).unwrap_or_default(),
                added_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

pub fn novel_upsert(db: &Connection, series: &crate::models::NovelSeries) -> Result<(), String> {
    let chapters = serde_json::to_string(&series.chapters).map_err(|e| e.to_string())?;
    db.execute(
        "INSERT INTO novel_series VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET title=?2,author=?3,chapters=?4",
        params![series.id, series.title, series.author, chapters, now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn novel_remove(db: &Connection, id: &str) -> Result<(), String> {
    db.execute("DELETE FROM novel_series WHERE id=?1", [id])
        .map_err(|e| e.to_string())?;
    db.execute("DELETE FROM novel_progress WHERE series_id=?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn novel_progress(db: &Connection, series_id: &str) -> Result<Option<(i64, i64)>, String> {
    db.query_row(
        "SELECT chapter_order,page FROM novel_progress WHERE series_id=?1",
        [series_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .map(Some)
    .or_else(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => Ok(None),
        other => Err(other),
    })
    .map_err(|e: rusqlite::Error| e.to_string())
}

/// 系列标题查询(时间线条目展示用;书架未收录返回 None)
pub fn novel_series_title(db: &Connection, series_id: &str) -> Option<String> {
    db.query_row(
        "SELECT title FROM novel_series WHERE id=?1",
        params![series_id],
        |r| r.get::<_, String>(0),
    )
    .ok()
}

pub fn novel_set_progress(
    db: &Connection,
    series_id: &str,
    chapter_order: i64,
    page: i64,
    title_hint: Option<&str>,
) -> Result<(), String> {
    db.execute(
        "INSERT INTO novel_progress VALUES(?1,?2,?3,?4) ON CONFLICT(series_id) DO UPDATE SET chapter_order=?2,page=?3,updated_at=?4",
        params![series_id, chapter_order, page, now()],
    )
    .map_err(|e| e.to_string())?;
    // 双写全局时间线(批次 4):progress 表保精确续读,item_history 保
    // 时间线;item_id 用系列,progress JSON 记章节与页码
    let progress_json = format!("{{\"chapter\":{},\"page\":{}}}", chapter_order, page);
    // 标题:B2 修复——前端传入(前端持有 series.title)优先,查表兜底,
    // 避免「系列 12345678」占位标题进时间线
    let title = title_hint
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .or_else(|| novel_series_title(db, series_id))
        .unwrap_or_else(|| format!("系列 {series_id}"));
    item_history_record(
        db,
        "hlib",
        "novel",
        series_id,
        &title,
        Some(&progress_json),
        None,
    )?;
    Ok(())
}

/// 视频观看记录
pub fn record_video_watched(db: &Connection, code: &str) -> Result<(), String> {
    db.execute(
        "INSERT INTO video_watched VALUES(?1,?2) ON CONFLICT(code) DO UPDATE SET watched_at=?2",
        params![code, now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 批量查询哪些视频看过(返回入集中已看的 code)
pub fn video_watched_ids(db: &Connection, codes: &[String]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for code in codes {
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM video_watched WHERE code=?1",
                params![code],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if count > 0 {
            out.push(code.clone());
        }
    }
    Ok(out)
}

/// 批量查询漫画是否看过(view_history,返回入集中已看的 id)
pub fn comic_viewed_ids(
    db: &Connection,
    source: crate::models::Source,
    ids: &[String],
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for id in ids {
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM view_history WHERE source=?1 AND comic_id=?2",
                params![source, id],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        if count > 0 {
            out.push(id.clone());
        }
    }
    Ok(out)
}

// ── follow_target 统一关注表(批次 3 视频关注首个用例;后续跨源扩展) ──

/// 关注目标条目
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FollowTargetEntry {
    pub source: String,
    pub kind: String,
    pub target_id: String,
    pub display_name: String,
    pub followed_at: i64,
}

/// 关注目标(存在则更新时间)
pub fn follow_target_add(
    conn: &Connection,
    source: &str,
    kind: &str,
    target_id: &str,
    display_name: &str,
) -> Result<(), String> {
    let time = now();
    conn.execute(
        "INSERT INTO follow_target VALUES(?1,?2,?3,?4,?5)
         ON CONFLICT(source,kind,target_id) DO UPDATE SET display_name=?4, followed_at=?5",
        params![source, kind, target_id, display_name, time],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 取消关注目标
pub fn follow_target_remove(
    conn: &Connection,
    source: &str,
    kind: &str,
    target_id: &str,
) -> Result<(), String> {
    conn.execute(
        "DELETE FROM follow_target WHERE source=?1 AND kind=?2 AND target_id=?3",
        params![source, kind, target_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 关注目标列表(按关注时间倒序)
pub fn follow_targets(
    conn: &Connection,
    source: &str,
    kind: &str,
) -> Result<Vec<FollowTargetEntry>, String> {
    let mut s = conn
        .prepare(
            "SELECT source,kind,target_id,display_name,followed_at FROM follow_target
             WHERE source=?1 AND kind=?2 ORDER BY followed_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map(params![source, kind], |r| {
            Ok(FollowTargetEntry {
                source: r.get(0)?,
                kind: r.get(1)?,
                target_id: r.get(2)?,
                display_name: r.get(3)?,
                followed_at: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// 全局历史条目(批次 4;视频/小说写入,漫画保留 view_history 存量)
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemHistoryEntry {
    pub source: String,
    pub kind: String,
    pub item_id: String,
    pub title: String,
    pub viewed_at: i64,
    pub progress: Option<String>,
}

/// 记录全局历史(存在则更新时间与进度,时间线条目唯一)
pub fn item_history_record(
    conn: &Connection,
    source: &str,
    kind: &str,
    item_id: &str,
    title: &str,
    progress: Option<&str>,
    thumbnail: Option<&str>,
) -> Result<(), String> {
    let time = now();
    conn.execute(
        "INSERT INTO item_history VALUES(?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(source,kind,item_id) DO UPDATE SET title=?4, viewed_at=?5,
           progress=COALESCE(?6,progress), thumbnail=COALESCE(?7,thumbnail)",
        params![source, kind, item_id, title, time, progress, thumbnail],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 全局历史查询(source/kind 可选过滤,None=不过滤;按时间倒序)
pub fn item_history_fetch(
    conn: &Connection,
    source: Option<&str>,
    kind: Option<&str>,
    limit: i64,
) -> Result<Vec<ItemHistoryEntry>, String> {
    let mut s = conn
        .prepare(
            "SELECT source,kind,item_id,title,viewed_at,progress FROM item_history
             WHERE (?1 IS NULL OR source=?1) AND (?2 IS NULL OR kind=?2)
             ORDER BY viewed_at DESC LIMIT ?3",
        )
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map(params![source, kind, limit], |r| {
            Ok(ItemHistoryEntry {
                source: r.get(0)?,
                kind: r.get(1)?,
                item_id: r.get(2)?,
                title: r.get(3)?,
                viewed_at: r.get(4)?,
                progress: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// 小说关键词黑名单条目
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NovelBlacklistEntry {
    pub keyword: String,
    pub created_at: i64,
}

/// 黑名单列表
pub fn novel_blacklist_list(conn: &Connection) -> Result<Vec<NovelBlacklistEntry>, String> {
    let mut s = conn
        .prepare("SELECT keyword,created_at FROM novel_blacklist ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
    let rows = s
        .query_map([], |r| {
            Ok(NovelBlacklistEntry {
                keyword: r.get(0)?,
                created_at: r.get(1)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// 添加黑名单关键词(子串匹配语义,与漫画 tag 精确匹配不同故独立表)
pub fn novel_blacklist_add(conn: &Connection, keyword: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO novel_blacklist VALUES(?1,?2)",
        params![keyword, now()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// 移除黑名单关键词
pub fn novel_blacklist_remove(conn: &Connection, keyword: &str) -> Result<(), String> {
    conn.execute(
        "DELETE FROM novel_blacklist WHERE keyword=?1",
        params![keyword],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}
