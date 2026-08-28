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
      CREATE TABLE IF NOT EXISTS view_history(comic_id TEXT PRIMARY KEY, title TEXT NOT NULL, thumbnail TEXT NOT NULL, category_id INTEGER NOT NULL, author TEXT NOT NULL, viewed_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS tag_blacklist(tag TEXT PRIMARY KEY, mode TEXT NOT NULL, created_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS comic_metadata(comic_id TEXT NOT NULL, category_id INTEGER NOT NULL, author TEXT NOT NULL, published_at TEXT, PRIMARY KEY(comic_id, category_id));").map_err(|e| e.to_string())?;
    Ok(db)
}
pub fn follows(db: &Connection) -> Result<Vec<FollowedAuthor>, String> {
    let mut s=db.prepare("SELECT author,followed_at,last_checked_at FROM followed_authors ORDER BY followed_at DESC").map_err(|e|e.to_string())?;
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
    db.execute("INSERT INTO followed_authors VALUES(?1,?2,?2) ON CONFLICT(author) DO UPDATE SET last_checked_at=?2",params![author,time]).map_err(|e|e.to_string())?;
    Ok(FollowedAuthor {
        author: author.into(),
        followed_at: time,
        last_checked_at: time,
    })
}
pub fn unfollow(db: &Connection, author: &str) -> Result<(), String> {
    db.execute("DELETE FROM followed_authors WHERE author=?1", [author])
        .map_err(|e| e.to_string())?;
    Ok(())
}
pub fn history(db: &Connection, limit: i64) -> Result<Vec<ViewHistoryEntry>, String> {
    let mut s=db.prepare("SELECT comic_id,title,thumbnail,category_id,author,viewed_at FROM view_history ORDER BY viewed_at DESC LIMIT ?1").map_err(|e|e.to_string())?;
    let rows = s
        .query_map([limit], |r| {
            Ok(ViewHistoryEntry {
                comic_id: r.get(0)?,
                title: r.get(1)?,
                thumbnail: r.get(2)?,
                category_id: r.get(3)?,
                author: r.get(4)?,
                viewed_at: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}
pub fn record(db: &Connection, v: RecordHistory) -> Result<(), String> {
    db.execute("INSERT INTO view_history VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(comic_id) DO UPDATE SET title=?2,thumbnail=?3,category_id=?4,author=?5,viewed_at=?6",params![v.comic_id,v.title,v.thumbnail,v.category_id,v.author,now()]).map_err(|e|e.to_string())?;
    Ok(())
}
pub fn comic_metadata(
    db: &Connection,
    category_id: i32,
    id: &str,
) -> Result<Option<ComicMetadata>, String> {
    let mut statement = db
        .prepare("SELECT comic_id,category_id,author,published_at FROM comic_metadata WHERE comic_id=?1 AND category_id=?2")
        .map_err(|e| e.to_string())?;
    let mut rows = statement
        .query(params![id, category_id])
        .map_err(|e| e.to_string())?;
    rows.next()
        .map_err(|e| e.to_string())?
        .map(|row| {
            Ok(ComicMetadata {
                id: row.get(0)?,
                category_id: row.get(1)?,
                author: row.get(2)?,
                published_at: row.get(3)?,
            })
        })
        .transpose()
        .map_err(|e: rusqlite::Error| e.to_string())
}
pub fn save_comic_metadata(db: &Connection, value: &ComicMetadata) -> Result<(), String> {
    db.execute(
        "INSERT INTO comic_metadata VALUES(?1,?2,?3,?4) ON CONFLICT(comic_id,category_id) DO UPDATE SET author=?3,published_at=?4",
        params![value.id, value.category_id, value.author, value.published_at],
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
