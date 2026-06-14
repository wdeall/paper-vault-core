//! 数据库：SQLite + 迁移 + FTS5。
//!
//! 通过 `tauri-plugin-sql` 在前端直接查询，复杂操作通过自写命令走
//! `services/*`。本模块提供纯 Rust 侧的连接与迁移函数。

use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use std::path::Path;
use std::sync::Mutex;

const INIT_SQL: &str = include_str!("migrations/0001_init.sql");

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
pub fn migrate(vault: &Path) -> AppResult<()> {
    let mut conn = open(vault)?;
    let tx = conn.transaction()?;
    tx.execute_batch(INIT_SQL)?;
    tx.commit()?;
    Ok(())
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
pub fn exec_sql(vault: &Path, sql: &str) -> AppResult<()> {
    let conn = open(vault)?;
    conn.execute_batch(sql)?;
    Ok(())
}

pub mod migrations {
    //! 迁移文件位置常量
    pub const INIT: &str = include_str!("migrations/0001_init.sql");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn migrate_is_idempotent() {
        let dir = tempdir().unwrap();
        crate::vault::init_at(dir.path()).unwrap();
        migrate(dir.path()).unwrap();
        // 二次跑必须成功（CREATE TABLE IF NOT EXISTS / 不重复）
        migrate(dir.path()).unwrap();
        let conn = open(dir.path()).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM papers", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
