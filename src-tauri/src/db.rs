//! SQLite 持久层:连接初始化(PRAGMA)、版本化迁移与各表读写。
//!
//! 约定:
//! - 全部函数接收共享连接(调用方持有 `AppState.db` 互斥锁),错误统一转 `String`;
//! - 热路径语句走 `prepare_cached`(连接级语句缓存,免重复编译 SQL);
//! - 结构变更只追加到 [`MIGRATIONS`] 末尾,且必须幂等、向后兼容(不删不改列/表)。

use crate::models::*;
use rusqlite::{params, Connection, OptionalExtension, Params, Row};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// 当前毫秒时间戳(全部 *_at 列的单位)
pub fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// rusqlite 错误统一转为 IPC 层的字符串错误
trait OrMsg<T> {
    fn or_msg(self) -> Result<T, String>;
}

impl<T> OrMsg<T> for rusqlite::Result<T> {
    fn or_msg(self) -> Result<T, String> {
        self.map_err(|e| e.to_string())
    }
}

/// 查询多行并逐行映射(语句走连接级缓存)
fn query_all<T, P: Params>(
    db: &Connection,
    sql: &str,
    params: P,
    map: impl FnMut(&Row<'_>) -> rusqlite::Result<T>,
) -> Result<Vec<T>, String> {
    let mut statement = db.prepare_cached(sql).or_msg()?;
    let rows = statement
        .query_map(params, map)
        .or_msg()?
        .collect::<rusqlite::Result<Vec<_>>>()
        .or_msg();
    rows
}

/// 执行单条写语句(语句走连接级缓存)
fn exec<P: Params>(db: &Connection, sql: &str, params: P) -> Result<(), String> {
    db.prepare_cached(sql)
        .and_then(|mut statement| statement.execute(params))
        .map(drop)
        .or_msg()
}

/// 打开(必要时创建)数据库并完成初始化与迁移
pub fn open(path: &std::path::Path) -> Result<Connection, String> {
    let db = Connection::open(path).or_msg()?;
    init(&db)?;
    Ok(db)
}

/// 连接级设置 + 迁移(与路径无关,便于内存库单测)
fn init(db: &Connection) -> Result<(), String> {
    // WAL + NORMAL:写入不再每次 fsync 主库,断电最多丢最近事务、不损坏;
    // temp_store=MEMORY:排序/临时表不落盘(Android 上无可用临时目录时也安全)
    db.execute_batch(
        "PRAGMA journal_mode=WAL;
         PRAGMA synchronous=NORMAL;
         PRAGMA temp_store=MEMORY;",
    )
    .or_msg()?;
    db.busy_timeout(Duration::from_secs(3)).or_msg()?;
    // 命令里的语句种类多于默认 16 条缓存容量,放宽避免热语句被挤出
    db.set_prepared_statement_cache_capacity(48);
    migrate(db).or_msg()?;
    // 长连接推荐的启动期统计维护(仅在有收益时才真正分析,开销极小);失败不影响使用
    let _ = db.execute_batch("PRAGMA optimize=0x10002;");
    Ok(())
}

/// 一步迁移;在事务内执行,成功后 `user_version` 记为该步序号(从 1 起)
type Migration = fn(&Connection) -> rusqlite::Result<()>;

/// 迁移序列。只能追加,不能修改或重排已发布的步骤
const MIGRATIONS: &[Migration] = &[migrate_v1_baseline, migrate_v2_order_indexes];

/// 按 `PRAGMA user_version` 执行尚未应用的迁移。
/// 更高版本(降级安装)时全部跳过,不做任何改动
fn migrate(db: &Connection) -> rusqlite::Result<()> {
    let version: i64 = db.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let applied = usize::try_from(version).unwrap_or(0);
    for (index, step) in MIGRATIONS.iter().enumerate().skip(applied) {
        let tx = db.unchecked_transaction()?;
        step(&tx)?;
        tx.pragma_update(None, "user_version", (index + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

fn has_column(db: &Connection, table: &str, column: &str) -> rusqlite::Result<bool> {
    db.query_row(
        "SELECT COUNT(*) FROM pragma_table_info(?1) WHERE name=?2",
        params![table, column],
        |r| r.get::<_, i64>(0),
    )
    .map(|count| count > 0)
}

/// v1 基线:引入 user_version 之前的全部建表与临时迁移,整理为一步。
/// 对任何历史版本的库都幂等(新装直接建最新结构,存量库按列探测补齐)
fn migrate_v1_baseline(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE TABLE IF NOT EXISTS followed_authors(author TEXT PRIMARY KEY, followed_at INTEGER NOT NULL, last_checked_at INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS follow_target(source TEXT NOT NULL, kind TEXT NOT NULL, target_id TEXT NOT NULL, display_name TEXT NOT NULL, followed_at INTEGER NOT NULL, PRIMARY KEY(source, kind, target_id));
         CREATE TABLE IF NOT EXISTS item_history(source TEXT NOT NULL, kind TEXT NOT NULL, item_id TEXT NOT NULL, title TEXT NOT NULL, viewed_at INTEGER NOT NULL, progress TEXT, thumbnail TEXT, PRIMARY KEY(source, kind, item_id));
         CREATE TABLE IF NOT EXISTS novel_blacklist(keyword TEXT PRIMARY KEY, created_at INTEGER NOT NULL);
         CREATE INDEX IF NOT EXISTS idx_item_history_viewed ON item_history(viewed_at DESC);
         CREATE TABLE IF NOT EXISTS view_history(source TEXT NOT NULL, comic_id TEXT NOT NULL, title TEXT NOT NULL, thumbnail TEXT NOT NULL, category_id INTEGER NOT NULL, author TEXT NOT NULL, viewed_at INTEGER NOT NULL, PRIMARY KEY(source, comic_id));
         CREATE TABLE IF NOT EXISTS tag_blacklist(tag TEXT PRIMARY KEY, mode TEXT NOT NULL, created_at INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS bika_account(id INTEGER PRIMARY KEY CHECK(id=1), email TEXT NOT NULL, password TEXT NOT NULL, token TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS bika_address(id INTEGER PRIMARY KEY CHECK(id=1), address TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS favorites(source TEXT NOT NULL, comic_id TEXT NOT NULL, title TEXT NOT NULL, thumbnail TEXT NOT NULL, category_id INTEGER NOT NULL, author TEXT NOT NULL, created_at INTEGER NOT NULL, PRIMARY KEY(source, comic_id));
         CREATE TABLE IF NOT EXISTS novel_series(id TEXT PRIMARY KEY, title TEXT NOT NULL, author TEXT NOT NULL, chapters TEXT NOT NULL, added_at INTEGER NOT NULL);
         CREATE TABLE IF NOT EXISTS novel_progress(series_id TEXT NOT NULL, chapter_order INTEGER NOT NULL, page INTEGER NOT NULL, updated_at INTEGER NOT NULL, PRIMARY KEY(series_id));
         CREATE TABLE IF NOT EXISTS video_watched(code TEXT PRIMARY KEY, watched_at INTEGER NOT NULL);",
    )?;
    // 旧版单源 view_history(无 source 列)迁移为复合主键表,旧行视为 niacg
    if !has_column(db, "view_history", "source")? {
        db.execute_batch(
            "CREATE TABLE view_history_migrated(source TEXT NOT NULL, comic_id TEXT NOT NULL, title TEXT NOT NULL, thumbnail TEXT NOT NULL, category_id INTEGER NOT NULL, author TEXT NOT NULL, viewed_at INTEGER NOT NULL, PRIMARY KEY(source, comic_id));
             INSERT INTO view_history_migrated SELECT 'niacg', comic_id, title, thumbnail, category_id, author, viewed_at FROM view_history;
             DROP TABLE view_history;
             ALTER TABLE view_history_migrated RENAME TO view_history;",
        )?;
    }
    // 存量 item_history 补 thumbnail 列。旧实现只在 view_history 迁移时顺带执行,
    // 早已迁移过 view_history 的库会缺列(记录视频历史报列数不符),这里按列探测补齐
    if !has_column(db, "item_history", "thumbnail")? {
        db.execute("ALTER TABLE item_history ADD COLUMN thumbnail TEXT", [])?;
    }
    // 旧 followed_authors 关注数据并入 follow_target 统一表
    db.execute(
        "INSERT OR IGNORE INTO follow_target(source,kind,target_id,display_name,followed_at)
         SELECT 'bika','author',author,author,followed_at FROM followed_authors",
        [],
    )?;
    Ok(())
}

/// v2:列表按时间倒序读取的表补排序索引(LIMIT 查询免全表排序)
fn migrate_v2_order_indexes(db: &Connection) -> rusqlite::Result<()> {
    db.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_view_history_viewed ON view_history(viewed_at DESC);
         CREATE INDEX IF NOT EXISTS idx_favorites_created ON favorites(created_at DESC);",
    )
}

// ── 哔咔作者关注(FollowedAuthor 接口,实现落在 follow_target) ──

pub fn follows(db: &Connection) -> Result<Vec<FollowedAuthor>, String> {
    // 批次 4 迁移:实现切换到 follow_target 统一表(接口 FollowedAuthor 不变;
    // last_checked_at 由 followed_at 兼容填充)
    query_all(
        db,
        "SELECT target_id,followed_at FROM follow_target
         WHERE source='bika' AND kind='author' ORDER BY followed_at DESC",
        [],
        |r| {
            let followed_at = r.get(1)?;
            Ok(FollowedAuthor {
                author: r.get(0)?,
                followed_at,
                last_checked_at: followed_at,
            })
        },
    )
}

pub fn follow(db: &Connection, author: &str) -> Result<FollowedAuthor, String> {
    let time = now();
    let tx = db.unchecked_transaction().or_msg()?;
    follow_target_upsert(&tx, "bika", "author", author, author, time)?;
    // followed_authors 双写保留(回滚安全;批次 4 收口后移除)
    exec(
        &tx,
        "INSERT INTO followed_authors(author,followed_at,last_checked_at) VALUES(?1,?2,?2)
         ON CONFLICT(author) DO UPDATE SET last_checked_at=?2",
        params![author, time],
    )?;
    tx.commit().or_msg()?;
    Ok(FollowedAuthor {
        author: author.into(),
        followed_at: time,
        last_checked_at: time,
    })
}

pub fn unfollow(db: &Connection, author: &str) -> Result<(), String> {
    let tx = db.unchecked_transaction().or_msg()?;
    follow_target_remove(&tx, "bika", "author", author)?;
    exec(
        &tx,
        "DELETE FROM followed_authors WHERE author=?1",
        [author],
    )?;
    tx.commit().or_msg()
}

// ── 漫画浏览记录(view_history) ──

pub fn history(db: &Connection, limit: i64) -> Result<Vec<ViewHistoryEntry>, String> {
    query_all(
        db,
        "SELECT source,comic_id,title,thumbnail,category_id,author,viewed_at FROM view_history
         ORDER BY viewed_at DESC LIMIT ?1",
        [limit],
        |r| {
            Ok(ViewHistoryEntry {
                source: r.get(0)?,
                comic_id: r.get(1)?,
                title: r.get(2)?,
                thumbnail: r.get(3)?,
                category_id: r.get(4)?,
                author: r.get(5)?,
                viewed_at: r.get(6)?,
            })
        },
    )
}

pub fn record(db: &Connection, v: RecordHistory) -> Result<(), String> {
    exec(
        db,
        "INSERT INTO view_history(source,comic_id,title,thumbnail,category_id,author,viewed_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(source,comic_id) DO UPDATE SET
           title=?3,thumbnail=?4,category_id=?5,author=?6,viewed_at=?7",
        params![
            v.source,
            v.comic_id,
            v.title,
            v.thumbnail,
            v.category_id,
            v.author,
            now()
        ],
    )
}

// ── 哔咔账号与分流(单行表) ──

pub struct BikaAccount {
    pub email: String,
    pub password: String,
    pub token: String,
}

pub fn bika_account(db: &Connection) -> Result<Option<BikaAccount>, String> {
    db.prepare_cached("SELECT email,password,token FROM bika_account WHERE id=1")
        .and_then(|mut statement| {
            statement
                .query_row([], |row| {
                    Ok(BikaAccount {
                        email: row.get(0)?,
                        password: row.get(1)?,
                        token: row.get(2)?,
                    })
                })
                .optional()
        })
        .or_msg()
}

pub fn save_bika_account(
    db: &Connection,
    email: &str,
    password: &str,
    token: &str,
) -> Result<(), String> {
    exec(
        db,
        "INSERT INTO bika_account(id,email,password,token) VALUES(1,?1,?2,?3)
         ON CONFLICT(id) DO UPDATE SET email=?1,password=?2,token=?3",
        params![email, password, token],
    )
}

pub fn clear_bika_account(db: &Connection) -> Result<(), String> {
    exec(db, "DELETE FROM bika_account WHERE id=1", [])
}

/// 哔咔分流地址;空字符串表示不分流(用系统 DNS)
pub fn bika_address(db: &Connection) -> Result<String, String> {
    db.query_row("SELECT address FROM bika_address WHERE id=1", [], |row| {
        row.get(0)
    })
    .optional()
    .map(Option::unwrap_or_default)
    .or_msg()
}

pub fn save_bika_address(db: &Connection, address: &str) -> Result<(), String> {
    exec(
        db,
        "INSERT INTO bika_address(id,address) VALUES(1,?1)
         ON CONFLICT(id) DO UPDATE SET address=?1",
        params![address],
    )
}

// ── 标签黑名单 ──

pub fn blacklist(db: &Connection) -> Result<Vec<BlacklistEntry>, String> {
    query_all(
        db,
        "SELECT tag,mode,created_at FROM tag_blacklist ORDER BY tag",
        [],
        |r| {
            Ok(BlacklistEntry {
                tag: r.get(0)?,
                mode: r.get(1)?,
                created_at: r.get(2)?,
            })
        },
    )
}

pub fn add_blacklist(db: &Connection, tag: &str, mode: &str) -> Result<BlacklistEntry, String> {
    let created_at = now();
    exec(
        db,
        "INSERT OR IGNORE INTO tag_blacklist(tag,mode,created_at) VALUES(?1,?2,?3)",
        params![tag, mode, created_at],
    )?;
    Ok(BlacklistEntry {
        tag: tag.into(),
        mode: mode.into(),
        created_at,
    })
}

pub fn remove_blacklist(db: &Connection, tag: &str) -> Result<(), String> {
    exec(db, "DELETE FROM tag_blacklist WHERE tag=?1", [tag])
}

pub fn update_blacklist(db: &Connection, tag: &str, mode: &str) -> Result<(), String> {
    exec(
        db,
        "UPDATE tag_blacklist SET mode=?2 WHERE tag=?1",
        params![tag, mode],
    )
}

// ── 收藏 ──

pub fn favorites(db: &Connection) -> Result<Vec<FavoriteEntry>, String> {
    query_all(
        db,
        "SELECT source,comic_id,title,thumbnail,category_id,author,created_at FROM favorites
         ORDER BY created_at DESC",
        [],
        |r| {
            Ok(FavoriteEntry {
                source: r.get(0)?,
                comic_id: r.get(1)?,
                title: r.get(2)?,
                thumbnail: r.get(3)?,
                category_id: r.get(4)?,
                author: r.get(5)?,
                created_at: r.get(6)?,
            })
        },
    )
}

/// 插入或刷新收藏(同步服务器列表时也走这里,保留已有 created_at)
pub fn add_favorite(db: &Connection, v: &RecordFavorite) -> Result<(), String> {
    exec(
        db,
        "INSERT INTO favorites(source,comic_id,title,thumbnail,category_id,author,created_at)
         VALUES(?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(source,comic_id) DO UPDATE SET title=?3,thumbnail=?4,category_id=?5,author=?6",
        params![
            v.source,
            v.comic_id,
            v.title,
            v.thumbnail,
            v.category_id,
            v.author,
            now()
        ],
    )
}

pub fn remove_favorite(db: &Connection, source: Source, comic_id: &str) -> Result<(), String> {
    exec(
        db,
        "DELETE FROM favorites WHERE source=?1 AND comic_id=?2",
        params![source, comic_id],
    )
}

pub fn is_favorite(db: &Connection, source: Source, comic_id: &str) -> Result<bool, String> {
    db.prepare_cached("SELECT 1 FROM favorites WHERE source=?1 AND comic_id=?2")
        .and_then(|mut statement| statement.exists(params![source, comic_id]))
        .or_msg()
}

// ── 已看标记(视频 / 漫画) ──

/// 视频观看记录
pub fn record_video_watched(db: &Connection, code: &str) -> Result<(), String> {
    exec(
        db,
        "INSERT INTO video_watched(code,watched_at) VALUES(?1,?2)
         ON CONFLICT(code) DO UPDATE SET watched_at=?2",
        params![code, now()],
    )
}

/// 逐个主键探测,保留输入顺序(含重复)返回存在的 key。
/// 同一预编译语句复用,每个 key 只是一次 B 树查找
fn existing_keys(
    db: &Connection,
    sql: &str,
    keys: &[String],
    mut exists: impl FnMut(&mut rusqlite::CachedStatement<'_>, &str) -> rusqlite::Result<bool>,
) -> Result<Vec<String>, String> {
    let mut statement = db.prepare_cached(sql).or_msg()?;
    let mut out = Vec::new();
    for key in keys {
        if exists(&mut statement, key.as_str()).or_msg()? {
            out.push(key.clone());
        }
    }
    Ok(out)
}

/// 批量查询哪些视频看过(返回入集中已看的 code)
pub fn video_watched_ids(db: &Connection, codes: &[String]) -> Result<Vec<String>, String> {
    existing_keys(
        db,
        "SELECT 1 FROM video_watched WHERE code=?1",
        codes,
        |statement, code| statement.exists([code]),
    )
}

/// 批量查询漫画是否看过(view_history,返回入集中已看的 id)
pub fn comic_viewed_ids(
    db: &Connection,
    source: crate::models::Source,
    ids: &[String],
) -> Result<Vec<String>, String> {
    existing_keys(
        db,
        "SELECT 1 FROM view_history WHERE source=?1 AND comic_id=?2",
        ids,
        |statement, id| statement.exists(params![source, id]),
    )
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

fn follow_target_upsert(
    conn: &Connection,
    source: &str,
    kind: &str,
    target_id: &str,
    display_name: &str,
    time: i64,
) -> Result<(), String> {
    exec(
        conn,
        "INSERT INTO follow_target(source,kind,target_id,display_name,followed_at)
         VALUES(?1,?2,?3,?4,?5)
         ON CONFLICT(source,kind,target_id) DO UPDATE SET display_name=?4, followed_at=?5",
        params![source, kind, target_id, display_name, time],
    )
}

/// 关注目标(存在则更新时间)
pub fn follow_target_add(
    conn: &Connection,
    source: &str,
    kind: &str,
    target_id: &str,
    display_name: &str,
) -> Result<(), String> {
    follow_target_upsert(conn, source, kind, target_id, display_name, now())
}

/// 取消关注目标
pub fn follow_target_remove(
    conn: &Connection,
    source: &str,
    kind: &str,
    target_id: &str,
) -> Result<(), String> {
    exec(
        conn,
        "DELETE FROM follow_target WHERE source=?1 AND kind=?2 AND target_id=?3",
        params![source, kind, target_id],
    )
}

/// 关注目标列表(按关注时间倒序)
pub fn follow_targets(
    conn: &Connection,
    source: &str,
    kind: &str,
) -> Result<Vec<FollowTargetEntry>, String> {
    query_all(
        conn,
        "SELECT source,kind,target_id,display_name,followed_at FROM follow_target
         WHERE source=?1 AND kind=?2 ORDER BY followed_at DESC",
        params![source, kind],
        |r| {
            Ok(FollowTargetEntry {
                source: r.get(0)?,
                kind: r.get(1)?,
                target_id: r.get(2)?,
                display_name: r.get(3)?,
                followed_at: r.get(4)?,
            })
        },
    )
}

// ── item_history 全局历史时间线 ──

/// 全局历史条目(批次 4;视频写入,漫画保留 view_history 存量)
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

/// 记录全局历史(存在则更新时间;进度/封面传 None 时保留已存值,时间线条目唯一)
pub fn item_history_record(
    conn: &Connection,
    source: &str,
    kind: &str,
    item_id: &str,
    title: &str,
    progress: Option<&str>,
    thumbnail: Option<&str>,
) -> Result<(), String> {
    exec(
        conn,
        "INSERT INTO item_history(source,kind,item_id,title,viewed_at,progress,thumbnail)
         VALUES(?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(source,kind,item_id) DO UPDATE SET title=?4, viewed_at=?5,
           progress=COALESCE(?6,progress), thumbnail=COALESCE(?7,thumbnail)",
        params![source, kind, item_id, title, now(), progress, thumbnail],
    )
}

/// 全局历史查询(source/kind 可选过滤,None=不过滤;按时间倒序)
pub fn item_history_fetch(
    conn: &Connection,
    source: Option<&str>,
    kind: Option<&str>,
    limit: i64,
) -> Result<Vec<ItemHistoryEntry>, String> {
    query_all(
        conn,
        "SELECT source,kind,item_id,title,viewed_at,progress FROM item_history
         WHERE (?1 IS NULL OR source=?1) AND (?2 IS NULL OR kind=?2)
         ORDER BY viewed_at DESC LIMIT ?3",
        params![source, kind, limit],
        |r| {
            Ok(ItemHistoryEntry {
                source: r.get(0)?,
                kind: r.get(1)?,
                item_id: r.get(2)?,
                title: r.get(3)?,
                viewed_at: r.get(4)?,
                progress: r.get(5)?,
            })
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> Connection {
        let db = Connection::open_in_memory().unwrap();
        init(&db).unwrap();
        db
    }

    fn user_version(db: &Connection) -> i64 {
        db.query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap()
    }

    fn index_names(db: &Connection) -> Vec<String> {
        query_all(
            db,
            "SELECT name FROM sqlite_master WHERE type='index' AND name LIKE 'idx_%' ORDER BY name",
            [],
            |r| r.get(0),
        )
        .unwrap()
    }

    fn count(db: &Connection, sql: &str) -> i64 {
        db.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    #[test]
    fn fresh_database_gets_full_schema_and_version() {
        let db = fresh();
        assert_eq!(user_version(&db), MIGRATIONS.len() as i64);
        assert!(has_column(&db, "view_history", "source").unwrap());
        assert!(has_column(&db, "item_history", "thumbnail").unwrap());
        assert_eq!(
            index_names(&db),
            [
                "idx_favorites_created",
                "idx_item_history_viewed",
                "idx_view_history_viewed"
            ]
        );
        // 重复初始化不报错、不改版本
        init(&db).unwrap();
        assert_eq!(user_version(&db), MIGRATIONS.len() as i64);
    }

    /// 最早期库:单源 view_history、无 thumbnail 的 item_history、旧作者关注表
    #[test]
    fn legacy_database_is_migrated_without_data_loss() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE followed_authors(author TEXT PRIMARY KEY, followed_at INTEGER NOT NULL, last_checked_at INTEGER NOT NULL);
             INSERT INTO followed_authors VALUES('作者甲', 10, 10);
             CREATE TABLE item_history(source TEXT NOT NULL, kind TEXT NOT NULL, item_id TEXT NOT NULL, title TEXT NOT NULL, viewed_at INTEGER NOT NULL, progress TEXT, PRIMARY KEY(source, kind, item_id));
             INSERT INTO item_history VALUES('fs1','video','ABC-1','旧视频',5,'p');
             CREATE TABLE view_history(comic_id TEXT NOT NULL, title TEXT NOT NULL, thumbnail TEXT NOT NULL, category_id INTEGER NOT NULL, author TEXT NOT NULL, viewed_at INTEGER NOT NULL);
             INSERT INTO view_history VALUES('42','旧漫画','t.jpg',3,'作者乙',7);",
        )
        .unwrap();
        init(&db).unwrap();

        let history = history(&db, 10).unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].source, Source::Niacg);
        assert_eq!(history[0].comic_id, "42");
        assert_eq!(history[0].viewed_at, 7);

        let follows = follows(&db).unwrap();
        assert_eq!(follows.len(), 1);
        assert_eq!(follows[0].author, "作者甲");
        assert_eq!(follows[0].last_checked_at, 10);

        let items = item_history_fetch(&db, None, None, 10).unwrap();
        assert_eq!(items[0].progress.as_deref(), Some("p"));
        item_history_record(&db, "fs1", "video", "ABC-2", "新视频", None, Some("c.jpg")).unwrap();
        assert_eq!(item_history_fetch(&db, None, None, 10).unwrap().len(), 2);
    }

    /// 旧实现只在 view_history 迁移时补 thumbnail 列:已迁移过 view_history 的库缺列,
    /// 记录视频历史会因列数不符失败。基线迁移应按列探测补齐
    #[test]
    fn item_history_thumbnail_is_added_even_if_view_history_already_migrated() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch(
            "CREATE TABLE view_history(source TEXT NOT NULL, comic_id TEXT NOT NULL, title TEXT NOT NULL, thumbnail TEXT NOT NULL, category_id INTEGER NOT NULL, author TEXT NOT NULL, viewed_at INTEGER NOT NULL, PRIMARY KEY(source, comic_id));
             CREATE TABLE item_history(source TEXT NOT NULL, kind TEXT NOT NULL, item_id TEXT NOT NULL, title TEXT NOT NULL, viewed_at INTEGER NOT NULL, progress TEXT, PRIMARY KEY(source, kind, item_id));",
        )
        .unwrap();
        init(&db).unwrap();
        assert!(has_column(&db, "item_history", "thumbnail").unwrap());
        item_history_record(&db, "fs1", "video", "X", "t", None, Some("c.jpg")).unwrap();
    }

    /// 文件库:WAL 生效、重复打开幂等
    #[test]
    fn file_database_uses_wal_and_reopens() {
        let dir =
            std::env::temp_dir().join(format!("ygqd-db-test-{}-{}", std::process::id(), now()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("ygqd.db");
        {
            let db = open(&path).unwrap();
            let mode: String = db
                .query_row("PRAGMA journal_mode", [], |r| r.get(0))
                .unwrap();
            assert_eq!(mode, "wal");
            record_video_watched(&db, "A").unwrap();
        }
        let db = open(&path).unwrap();
        assert_eq!(user_version(&db), MIGRATIONS.len() as i64);
        assert_eq!(video_watched_ids(&db, &["A".to_string()]).unwrap(), ["A"]);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 降级安装(库版本高于本程序)时不做任何迁移
    #[test]
    fn newer_user_version_skips_migrations() {
        let db = Connection::open_in_memory().unwrap();
        db.execute_batch("PRAGMA user_version=999;").unwrap();
        migrate(&db).unwrap();
        assert_eq!(user_version(&db), 999);
        assert!(index_names(&db).is_empty());
    }

    #[test]
    fn item_history_keeps_progress_and_thumbnail_when_omitted() {
        let db = fresh();
        item_history_record(&db, "fs1", "video", "A", "t1", Some("p1"), Some("c1")).unwrap();
        item_history_record(&db, "fs1", "video", "A", "t2", None, None).unwrap();
        item_history_record(&db, "missav", "video", "B", "t3", None, None).unwrap();
        let all = item_history_fetch(&db, None, None, 10).unwrap();
        assert_eq!(all.len(), 2);
        let a = all.iter().find(|e| e.item_id == "A").unwrap();
        assert_eq!(a.title, "t2");
        assert_eq!(a.progress.as_deref(), Some("p1"));
        let thumb: Option<String> = db
            .query_row(
                "SELECT thumbnail FROM item_history WHERE item_id='A'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(thumb.as_deref(), Some("c1"));
        let only_missav = item_history_fetch(&db, Some("missav"), Some("video"), 10).unwrap();
        assert_eq!(only_missav.len(), 1);
        assert_eq!(only_missav[0].item_id, "B");
        assert_eq!(item_history_fetch(&db, None, None, 1).unwrap().len(), 1);
    }

    #[test]
    fn watched_and_viewed_lookups_preserve_input_order() {
        let db = fresh();
        record_video_watched(&db, "B").unwrap();
        record_video_watched(&db, "A").unwrap();
        let codes: Vec<String> = ["A", "X", "B", "A"].map(String::from).to_vec();
        assert_eq!(video_watched_ids(&db, &codes).unwrap(), ["A", "B", "A"]);
        assert!(video_watched_ids(&db, &[]).unwrap().is_empty());

        let entry = |id: &str| RecordHistory {
            source: Source::Bika,
            comic_id: id.into(),
            title: "t".into(),
            thumbnail: "th".into(),
            category_id: 0,
            author: "a".into(),
        };
        record(&db, entry("1")).unwrap();
        record(&db, entry("2")).unwrap();
        record(&db, entry("1")).unwrap();
        let ids: Vec<String> = ["2", "3", "1"].map(String::from).to_vec();
        assert_eq!(
            comic_viewed_ids(&db, Source::Bika, &ids).unwrap(),
            ["2", "1"]
        );
        assert!(comic_viewed_ids(&db, Source::Niacg, &ids)
            .unwrap()
            .is_empty());
        assert_eq!(history(&db, 10).unwrap().len(), 2);
    }

    #[test]
    fn favorites_upsert_keeps_created_at() {
        let db = fresh();
        let mut fav = RecordFavorite {
            source: Source::Niacg,
            comic_id: "9".into(),
            title: "old".into(),
            thumbnail: "th".into(),
            category_id: 1,
            author: "a".into(),
        };
        add_favorite(&db, &fav).unwrap();
        db.execute("UPDATE favorites SET created_at=1", []).unwrap();
        fav.title = "new".into();
        add_favorite(&db, &fav).unwrap();
        let list = favorites(&db).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "new");
        assert_eq!(list[0].created_at, 1);
        assert!(is_favorite(&db, Source::Niacg, "9").unwrap());
        assert!(!is_favorite(&db, Source::Bika, "9").unwrap());
        remove_favorite(&db, Source::Niacg, "9").unwrap();
        assert!(!is_favorite(&db, Source::Niacg, "9").unwrap());
    }

    #[test]
    fn follow_writes_both_tables_and_unfollow_clears_them() {
        let db = fresh();
        let followed = follow(&db, "作者").unwrap();
        assert_eq!(followed.followed_at, followed.last_checked_at);
        assert_eq!(follows(&db).unwrap().len(), 1);
        assert_eq!(count(&db, "SELECT COUNT(*) FROM followed_authors"), 1);
        follow_target_add(&db, "fs1", "maker", "m1", "厂商").unwrap();
        assert_eq!(
            follow_targets(&db, "fs1", "maker").unwrap()[0].display_name,
            "厂商"
        );
        unfollow(&db, "作者").unwrap();
        assert!(follows(&db).unwrap().is_empty());
        assert_eq!(count(&db, "SELECT COUNT(*) FROM followed_authors"), 0);
        follow_target_remove(&db, "fs1", "maker", "m1").unwrap();
        assert!(follow_targets(&db, "fs1", "maker").unwrap().is_empty());
    }

    #[test]
    fn bika_account_and_address_roundtrip() {
        let db = fresh();
        assert!(bika_account(&db).unwrap().is_none());
        assert_eq!(bika_address(&db).unwrap(), "");
        save_bika_account(&db, "e", "p", "t1").unwrap();
        save_bika_account(&db, "e", "p", "t2").unwrap();
        assert_eq!(bika_account(&db).unwrap().unwrap().token, "t2");
        clear_bika_account(&db).unwrap();
        assert!(bika_account(&db).unwrap().is_none());
        save_bika_address(&db, "1.2.3.4").unwrap();
        assert_eq!(bika_address(&db).unwrap(), "1.2.3.4");
    }

    #[test]
    fn blacklist_crud() {
        let db = fresh();
        add_blacklist(&db, "b", "hide").unwrap();
        add_blacklist(&db, "a", "hide").unwrap();
        // 重复添加保持原值
        add_blacklist(&db, "a", "blur").unwrap();
        update_blacklist(&db, "b", "blur").unwrap();
        let list = blacklist(&db).unwrap();
        let tags: Vec<&str> = list.iter().map(|e| e.tag.as_str()).collect();
        assert_eq!(tags, ["a", "b"]);
        assert_eq!(list[0].mode, "hide");
        assert_eq!(list[1].mode, "blur");
        remove_blacklist(&db, "a").unwrap();
        assert_eq!(blacklist(&db).unwrap().len(), 1);
    }

    /// 关键列表查询应命中排序索引,而非全表扫描后临时排序
    #[test]
    fn list_queries_use_order_indexes() {
        let db = fresh();
        let plan = |sql: &str| -> String {
            query_all(&db, &format!("EXPLAIN QUERY PLAN {sql}"), [], |r| {
                r.get::<_, String>(3)
            })
            .unwrap()
            .join("\n")
        };
        let p = plan("SELECT * FROM view_history ORDER BY viewed_at DESC LIMIT 100");
        assert!(p.contains("idx_view_history_viewed"), "{p}");
        let p = plan("SELECT * FROM favorites ORDER BY created_at DESC");
        assert!(p.contains("idx_favorites_created"), "{p}");
        let p = plan("SELECT * FROM item_history ORDER BY viewed_at DESC LIMIT 100");
        assert!(p.contains("idx_item_history_viewed"), "{p}");
    }
}
