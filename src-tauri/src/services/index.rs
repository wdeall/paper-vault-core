//! 索引服务：维护 papers_fts + index_status。
//!
//! P3 起 fulltext_index 表已废弃 (migration 0003 删除),搜索切到
//! papers_fts (metadata-only)。本模块只保留 reindex_paper /
//! reindex_all / status_summary 三个入口,实际 FTS 写入由
//! `services::search::sync_papers_fts` 完成。

use crate::db;
use crate::error::{AppError, AppResult};
use crate::types::IndexStatusSummary;
use rusqlite::params;
use std::path::Path;

/// 重建单篇论文的 papers_fts 行 + 更新 index_status。
///
/// P3 起 fulltext_index 表已删除,本函数只做:
///   1. 写 index_status = "索引中"
///   2. 调 `services::search::sync_papers_fts` 同步 papers_fts
///   3. 写 index_status = "已索引"
pub fn reindex_paper(vault: &Path, paper_id: &str) -> AppResult<()> {
    let conn = db::open(vault)?;
    // 写 status = 索引中
    let now = chrono::Local::now().timestamp_millis();
    conn.execute(
        "INSERT INTO index_status (paper_id, status, indexed_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(paper_id) DO UPDATE SET status = excluded.status, indexed_at = excluded.indexed_at",
        params![paper_id, "索引中", now],
    )?;

    // 校验 paper 存在
    let exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM papers WHERE id = ?1",
            params![paper_id],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if exists == 0 {
        return Err(AppError::NotFound(format!("论文 {paper_id} 不存在")));
    }

    // 同步 papers_fts
    if let Err(e) = crate::services::search::sync_papers_fts(&conn, paper_id) {
        let _ = conn.execute(
            "UPDATE index_status SET status = ?2, error = ?3 WHERE paper_id = ?1",
            params![paper_id, "索引失败", e.to_string()],
        );
        return Err(e);
    }

    // 写 status = 已索引
    conn.execute(
        "UPDATE index_status SET status = ?2, indexed_at = ?3, error = NULL WHERE paper_id = ?1",
        params![paper_id, "已索引", chrono::Local::now().timestamp_millis()],
    )?;
    Ok(())
}

/// 遍历所有 paper_id 调 reindex_paper。
pub fn reindex_all(vault: &Path) -> AppResult<()> {
    let conn = db::open(vault)?;
    let mut stmt = conn.prepare("SELECT id FROM papers")?;
    let ids: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    for id in ids {
        if let Err(e) = reindex_paper(vault, &id) {
            log::warn!("重新索引 {id} 失败: {e}");
            let conn2 = db::open(vault)?;
            conn2.execute(
                "INSERT INTO index_status (paper_id, status, error) VALUES (?1, ?2, ?3)
                 ON CONFLICT(paper_id) DO UPDATE SET status = excluded.status, error = excluded.error",
                params![id, "索引失败", e.to_string()],
            )?;
        }
    }
    Ok(())
}

/// 索引状态汇总。indexed = papers_fts 行数。
pub fn status_summary(vault: &Path) -> AppResult<IndexStatusSummary> {
    let conn = db::open(vault)?;
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM papers", [], |r| r.get(0))?;
    // indexed = papers_fts 行数 (与 papers 表 JOIN 防止 FTS 残留)
    let indexed: i64 = conn.query_row(
        "SELECT COUNT(*) FROM papers WHERE id IN (SELECT paper_id FROM papers_fts)",
        [],
        |r| r.get(0),
    )?;
    let mut sum = IndexStatusSummary {
        total,
        indexed,
        ..Default::default()
    };
    let mut stmt = conn.prepare("SELECT status, COUNT(*) FROM index_status GROUP BY status")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for r in rows {
        let (s, n) = r?;
        match s.as_str() {
            "已索引" => {}
            "索引中" => sum.indexing = n,
            "索引失败" => sum.failed = n,
            _ => sum.pending += n,
        }
    }
    sum.pending = sum.total - sum.indexed - sum.indexing - sum.failed;
    Ok(sum)
}
