//! 数据库：SQLite + 迁移 + FTS5。
//!
//! 通过 `tauri-plugin-sql` 在前端直接查询，复杂操作通过自写命令走
//! `services/*`。本模块提供纯 Rust 侧的连接与迁移函数。
//!
//! v2.0 PLAN §3.4：顺序执行 `0001_init.sql` → `0002_zotero_alignment.sql`，
//! 然后调用 `migrate_v2::run_migration` 把旧 JSON 数据搬运到新结构化表。
//! `migrate()` 整体幂等：二次执行 no-op。

pub mod migrate_v2;

use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::path::Path;

/// 获取库目录对应的 SQLite 连接。
/// 不缓存连接（每次调用 new_connection），由调用方管理。
pub fn open(vault: &Path) -> AppResult<Connection> {
    let db_path = vault.join(crate::vault::DB_FILE);
    if let Some(p) = db_path.parent() {
        std::fs::create_dir_all(p)?;
    }
    let conn = Connection::open(&db_path)?;
    // WAL + busy_timeout
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.pragma_update(None, "busy_timeout", 5000)?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    Ok(conn)
}

/// 在库上跑迁移（幂等）。
///
/// 返回 `(current_version, migrated_v2_data)`：
///   - `current_version`：当前 schema 版本号（v2 = P0 完成态）。
///   - `migrated_v2_data`：本次是否实际执行了 P0 数据搬运（仅在
///     旧库升级时为 true；二次 / 新建库均为 false）。
pub fn migrate(vault: &Path) -> AppResult<(i32, bool)> {
    use std::fs;

    let migrations_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("db")
        .join("migrations");
    let mut entries: Vec<_> = fs::read_dir(&migrations_dir)
        .map_err(|e| AppError::Io(format!("读取 {} 失败: {e}", migrations_dir.display())))?
        .filter_map(|res| res.ok())
        .filter_map(|e| {
            let p = e.path();
            let is_sql = p.extension().and_then(|s| s.to_str()) == Some("sql");
            if is_sql {
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.to_string());
                name.map(|n| (n, p))
            } else {
                None
            }
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut conn = open(vault)?;
    let tx = conn.transaction()?;
    for (name, path) in &entries {
        let sql = fs::read_to_string(path)
            .map_err(|e| AppError::Io(format!("读取 {} 失败: {e}", path.display())))?;
        log::info!("应用迁移: {name}");
        tx.execute_batch(&sql)
            .map_err(|e| AppError::Db(format!("迁移 {name} 失败: {e}")))?;
    }
    tx.commit()?;

    // P0 数据搬运（独立事务，已在内部处理回滚）。
    let migrated = migrate_v2::run_migration(vault, &mut conn)?;

    // 写版本号。write_schema_version 自身用 CREATE TABLE IF NOT EXISTS
    // + INSERT ON CONFLICT 保证幂等，且会把 schema_version 表创建好。
    let version = migrate_v2::read_schema_version(&conn).unwrap_or(1);
    if migrated || version < migrate_v2::TARGET_SCHEMA_VERSION {
        migrate_v2::write_schema_version(&conn, migrate_v2::TARGET_SCHEMA_VERSION)?;
    }
    Ok((migrate_v2::TARGET_SCHEMA_VERSION, migrated))
}

pub fn count_papers(vault: &Path) -> AppResult<i64> {
    let conn = open(vault)?;
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM papers", [], |r| r.get(0))?;
    Ok(n)
}

pub fn count_indexed(vault: &Path) -> AppResult<i64> {
    let conn = open(vault)?;
    let n: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM index_status WHERE status='已索引'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    Ok(n)
}

// 简单包装：执行一个 SQL 文件（多次语句）。
#[allow(dead_code)]
pub fn exec_sql(vault: &Path, sql: &str) -> AppResult<()> {
    let conn = open(vault)?;
    conn.execute_batch(sql)?;
    Ok(())
}

pub mod migrations {
    //! 迁移文件位置常量
    // P0 引入的第二个迁移文件，inline 在 `migrate()` 中按文件名顺序
    // 自动执行；这里保留常量以便后续脚本 / 测试按需使用。
    #[allow(dead_code)]
    pub const INIT: &str = include_str!("migrations/0001_init.sql");
    #[allow(dead_code)]
    pub const V2_ALIGNMENT: &str = include_str!("migrations/0002_zotero_alignment.sql");
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::tempdir;

    #[test]
    fn migrate_is_idempotent() {
        let dir = tempdir().unwrap();
        crate::vault::init_at(dir.path()).unwrap();
        let (v1, _m1) = migrate(dir.path()).unwrap();
        assert_eq!(v1, 2);
        // 二次跑必须成功且 m=false（已搬运过）。
        let (_v2, m2) = migrate(dir.path()).unwrap();
        assert!(!m2);
        let conn = open(dir.path()).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM papers", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn legacy_json_gets_migrated_to_new_tables() {
        let dir = tempdir().unwrap();
        crate::vault::init_at(dir.path()).unwrap();

        // 1. 用 0001_init 手动建表 + 写入一行“旧 schema”数据。
        let conn = open(dir.path()).unwrap();
        conn.execute_batch(include_str!("migrations/0001_init.sql"))
            .unwrap();
        let now = chrono::Local::now().timestamp_millis();
        conn.execute(
            "INSERT INTO papers
                (id, title, year, venue, doi, status, authors, keywords, tags,
                 pdf_path, note_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                "p1",
                "Hello",
                2024_i32,
                "Nature",
                "10.1109/foo",
                "未读",
                r#"["Alice Smith","Bob"]"#,
                r#"["ml","rl"]"#,
                r#"["hot"]"#,
                "pdfs/2024/p1.pdf",
                "notes/papers/p1.md",
                now,
                now,
            ],
        )
        .unwrap();
        drop(conn);

        // 2. 跑 P0 迁移。
        let (_v, migrated) = migrate(dir.path()).unwrap();
        assert!(migrated, "应当检测到旧 JSON 并搬运");

        // 3. 断言：papers.status 正规化 + 结构化表有数据。
        let conn = open(dir.path()).unwrap();
        let status: String = conn
            .query_row("SELECT status FROM papers WHERE id='p1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(status, "unread");

        let creators: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM paper_creators WHERE paper_id='p1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(creators, 2);

        let kws: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM paper_keywords WHERE paper_id='p1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kws, 2);

        let atts: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM attachments WHERE paper_id='p1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(atts, 2, "应有一条 PDF + 一条 note");

        let fts: i64 = conn
            .query_row("SELECT COUNT(*) FROM papers_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fts, 1);

        // 4. 备份文件应存在。
        let backup_dir = dir.path().join("backups");
        assert!(backup_dir.exists(), "backups/ 目录应已创建");
        let any_backup = std::fs::read_dir(&backup_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("vault-")
            });
        assert!(any_backup, "backups/ 中应至少存在一个 vault-*.db");
    }
}
