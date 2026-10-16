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
      CREATE TABLE IF NOT EXISTS view_history(comic_id TEXT NOT NULL, title TEXT NOT NULL, thumbnail TEXT NOT NULL, category_id INTEGER NOT NULL, author TEXT NOT NULL, viewed_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS tag_blacklist(tag TEXT PRIMARY KEY, mode TEXT NOT NULL, created_at INTEGER NOT NULL);
      CREATE TABLE IF NOT EXISTS bika_account(id INTEGER PRIMARY KEY CHECK(id=1), email TEXT NOT NULL, password TEXT NOT NULL, token TEXT NOT NULL);").map_err(|e| e.to_string())?;
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
    Ok(())
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
