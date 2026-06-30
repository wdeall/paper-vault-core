//! Paper 服务：所有 CRUD 走结构化表（creators / paper_creators /
//! identifiers / keywords / paper_keywords / attachments）。
//!
//! v2.0 P0：app 层只读写新结构化表。`Paper` 是从多表 JOIN 出来的
//! "合并视图"；写操作拆成对各子表的增删改。

use crate::db;
use crate::error::{AppError, AppResult};
use crate::types::{Paper, PaperDetail, PaperStatus};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

// ============================================================
// 读：合并视图
// ============================================================

/// 从 DB 取一条 paper 的"合并视图"（不含 reading_progress / index_status / collections）。
///
/// 供其它内部服务（ai_svc / note / index）按需取 Paper；UI 层用
/// `get()` 拿完整 `PaperDetail`。
pub fn load_paper(
    vault: &Path,
    paper_id: &str,
) -> AppResult<Option<Paper>> {
    let conn = db::open(vault)?;
    row_to_paper(&conn, paper_id)
}

/// 从 DB 取一条 paper，附 JOIN 出 authors / keywords。
///
/// 实现细节：先读 `papers` 行 + status 字符串解析为枚举，再用
/// 两次 `GROUP_CONCAT` 子查询拉 authors / keywords。SQLite 没有
/// struct 投影，这种"两阶段读"比动态拼 JOIN 字符串更稳。
fn row_to_paper(
    conn: &Connection,
    paper_id: &str,
) -> AppResult<Option<Paper>> {
    let row = conn
        .query_row(
            "SELECT id, title, year, venue, doi, abstract_text, status, rating,
                    pdf_path, note_path, created_at, updated_at
             FROM papers WHERE id = ?1",
            params![paper_id],
            |r| {
                Ok((
                    r.get::<_, String>("id")?,
                    r.get::<_, String>("title")?,
                    r.get::<_, Option<i32>>("year")?,
                    r.get::<_, String>("venue")?,
                    r.get::<_, String>("doi")?,
                    r.get::<_, String>("abstract_text")?,
                    r.get::<_, String>("status")?,
                    r.get::<_, Option<i32>>("rating")?,
                    r.get::<_, String>("pdf_path")?,
                    r.get::<_, String>("note_path")?,
                    r.get::<_, i64>("created_at")?,
                    r.get::<_, i64>("updated_at")?,
                ))
            },
        )
        .optional()?;

    let (id, title, year, venue, doi, abstract_text, status, rating, pdf_path, note_path, created_at, updated_at) =
        match row {
            Some(t) => t,
            None => return Ok(None),
        };

    // authors via creators JOIN
    let authors: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT c.display_name
             FROM paper_creators pc
             JOIN creators c ON c.id = pc.creator_id
             WHERE pc.paper_id = ?1 AND pc.role = 'author'
             ORDER BY pc.position",
        )?;
        let rows = stmt.query_map(params![paper_id], |r| r.get::<_, String>(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    // keywords
    let keywords: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT k.name
             FROM paper_keywords pk
             JOIN keywords k ON k.id = pk.keyword_id
             WHERE pk.paper_id = ?1
             ORDER BY k.name",
        )?;
        let rows = stmt.query_map(params![paper_id], |r| r.get::<_, String>(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    Ok(Some(Paper {
        id,
        title,
        authors,
        year,
        venue,
        doi,
        abstract_text,
        keywords,
        status: PaperStatus::parse(&status),
        rating,
        pdf_path,
        note_path,
        created_at,
        updated_at,
    }))
}

pub fn get(vault: &Path, id: &str) -> AppResult<PaperDetail> {
    let conn = db::open(vault)?;
    let paper = row_to_paper(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("论文 {id} 不存在")))?;

    // reading_progress
    let progress = conn
        .query_row(
            "SELECT current_page, total_pages, last_read_at
             FROM reading_progress WHERE paper_id = ?1",
            params![id],
            |r| {
                let cur = r.get::<_, i32>(0)?;
                let total = r.get::<_, i32>(1)?;
                let last = r.get::<_, i64>(2)?;
                let pct = if total > 0 {
                    (cur as f32 / total as f32) * 100.0
                } else {
                    0.0
                };
                Ok(crate::types::ReadingProgress {
                    current_page: cur,
                    total_pages: total,
                    progress_percent: pct,
                    last_read_at: last,
                })
            },
        )
        .optional()?;

    // index_status
    let index_status: String = conn
        .query_row(
            "SELECT status FROM index_status WHERE paper_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "未索引".into());

    // collections
    let collections: Vec<String> = {
        let mut stmt = conn.prepare(
            "SELECT c.name FROM paper_collections pc
             JOIN collections c ON c.id = pc.collection_id
             WHERE pc.paper_id = ?1 ORDER BY c.name",
        )?;
        let rows = stmt.query_map(params![id], |r| r.get::<_, String>(0))?;
        rows.filter_map(|r| r.ok()).collect()
    };

    Ok(PaperDetail {
        paper,
        reading_progress: progress,
        index_status,
        collections,
    })
}

/// 列表查询（无 JOIN 开销，按 status / 集合过滤）。
pub fn list(
    vault: &Path,
    status: Option<PaperStatus>,
    collection_id: Option<&str>,
    smart_view: Option<&str>,
) -> AppResult<Vec<Paper>> {
    let conn = db::open(vault)?;
    // 拼 SQL — 用参数化避免注入。
    let mut sql = String::from(
        "SELECT p.id FROM papers p WHERE 1=1",
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(s) = status {
        sql.push_str(" AND p.status = ?");
        args.push(Box::new(s.as_str().to_string()));
    }
    if let Some(cid) = collection_id {
        sql.push_str(
            " AND p.id IN (SELECT paper_id FROM paper_collections WHERE collection_id = ?)",
        );
        args.push(Box::new(cid.to_string()));
    }
    // smart_view 预留：现阶段当 "all" 处理；后续 P1 再加实现。
    let _ = smart_view;

    sql.push_str(" ORDER BY p.updated_at DESC");

    let mut stmt = conn.prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| &**b).collect();
    let rows = stmt.query_map(params_refs.as_slice(), |r| r.get::<_, String>(0))?;
    let ids: Vec<String> = rows.filter_map(|r| r.ok()).collect();
    drop(stmt);

    let mut out = Vec::with_capacity(ids.len());
    for id in &ids {
        if let Some(p) = row_to_paper(&conn, id)? {
            out.push(p);
        }
    }
    Ok(out)
}

// ============================================================
// 写：单表 / 拆表
// ============================================================

/// 生成稳定主键（与 migrate_v2 同样的 djb2 思路）。
///
/// 公开：供 commands 层复用，保证 metadata-based paper id 与 services 层一致。
pub fn stable_id(prefix: &str, raw: &str) -> String {
    let mut hash: u64 = 5381;
    for b in raw.as_bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(u64::from(*b));
    }
    format!("{prefix}-{:08x}{:08x}", (hash >> 32) as u32, hash as u32)
}

/// 给定名字字符串，尝试拆为 (family, given)。
/// 规则：
///   - 含 "," → "," 前 = family，后 = given
///   - 否则取最后一个空格前的部分 = given
///   - 单字 → family = 整串
fn split_name(s: &str) -> (String, String) {
    let t = s.trim();
    if t.is_empty() {
        return (String::new(), String::new());
    }
    if let Some((f, g)) = t.split_once(',') {
        return (f.trim().to_string(), g.trim().to_string());
    }
    if let Some((g, f)) = t.rsplit_once(' ') {
        return (f.trim().to_string(), g.trim().to_string());
    }
    (t.to_string(), String::new())
}

pub fn insert(vault: &Path, paper: &Paper) -> AppResult<()> {
    let mut conn = db::open(vault)?;
    let tx = conn.transaction()?;

    let now = chrono::Local::now().timestamp_millis();
    tx.execute(
        "INSERT INTO papers
            (id, title, year, venue, doi, abstract_text, status, rating,
             pdf_path, note_path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            paper.id,
            paper.title,
            paper.year,
            paper.venue,
            paper.doi,
            paper.abstract_text,
            paper.status.as_str(),
            paper.rating,
            paper.pdf_path,
            paper.note_path,
            now,
            now,
        ],
    )?;

    // authors
    for (pos, name) in paper.authors.iter().enumerate() {
        if name.trim().is_empty() {
            continue;
        }
        let (family, given) = split_name(name);
        let cid = stable_id("cr", &format!("{family}|{given}|{name}").to_lowercase());
        tx.execute(
            "INSERT OR IGNORE INTO creators (id, family_name, given_name, display_name, raw)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![cid, family, given, name.trim(), name.trim()],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO paper_creators (paper_id, creator_id, position, role)
             VALUES (?1, ?2, ?3, 'author')",
            params![paper.id, cid, pos as i32],
        )?;
    }

    // keywords
    for kw in &paper.keywords {
        if kw.trim().is_empty() {
            continue;
        }
        let kid = stable_id("kw", &kw.trim().to_lowercase());
        tx.execute(
            "INSERT OR IGNORE INTO keywords (id, name, source) VALUES (?1, ?2, 'manual')",
            params![kid, kw.trim()],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO paper_keywords (paper_id, keyword_id) VALUES (?1, ?2)",
            params![paper.id, kid],
        )?;
    }

    // identifiers (DOI)
    if !paper.doi.trim().is_empty() {
        tx.execute(
            "INSERT OR IGNORE INTO identifiers (paper_id, type, value, is_primary)
             VALUES (?1, 'doi', ?2, 1)",
            params![paper.id, paper.doi.trim()],
        )?;
    }

    // attachments
    if !paper.pdf_path.trim().is_empty() {
        let aid = stable_id("att-pdf", &format!("{}|{}", paper.id, paper.pdf_path));
        tx.execute(
            "INSERT OR IGNORE INTO attachments
                (id, paper_id, kind, rel_path, title, status)
             VALUES (?1, ?2, 'pdf', ?3, 'main', 'active')",
            params![aid, paper.id, paper.pdf_path],
        )?;
    }
    if !paper.note_path.trim().is_empty() {
        let aid = stable_id("att-note", &format!("{}|{}", paper.id, paper.note_path));
        tx.execute(
            "INSERT OR IGNORE INTO attachments
                (id, paper_id, kind, rel_path, title, status)
             VALUES (?1, ?2, 'note', ?3, 'main', 'active')",
            params![aid, paper.id, paper.note_path],
        )?;
    }

    // FTS
    rebuild_fts_for_paper(&tx, &paper.id)?;

    tx.commit()?;
    Ok(())
}

pub fn update(vault: &Path, id: &str, paper: &Paper) -> AppResult<Paper> {
    let mut conn = db::open(vault)?;
    let tx = conn.transaction()?;

    let now = chrono::Local::now().timestamp_millis();
    tx.execute(
        "UPDATE papers
            SET title = ?1, year = ?2, venue = ?3, doi = ?4,
                abstract_text = ?5, status = ?6, rating = ?7,
                pdf_path = ?8, note_path = ?9, updated_at = ?10
         WHERE id = ?11",
        params![
            paper.title,
            paper.year,
            paper.venue,
            paper.doi,
            paper.abstract_text,
            paper.status.as_str(),
            paper.rating,
            paper.pdf_path,
            paper.note_path,
            now,
            id,
        ],
    )?;

    // 重建 creators / keywords / identifiers（保持幂等）。
    tx.execute("DELETE FROM paper_creators WHERE paper_id = ?1", params![id])?;
    for (pos, name) in paper.authors.iter().enumerate() {
        if name.trim().is_empty() {
            continue;
        }
        let (family, given) = split_name(name);
        let cid = stable_id("cr", &format!("{family}|{given}|{name}").to_lowercase());
        tx.execute(
            "INSERT OR IGNORE INTO creators (id, family_name, given_name, display_name, raw)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![cid, family, given, name.trim(), name.trim()],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO paper_creators (paper_id, creator_id, position, role)
             VALUES (?1, ?2, ?3, 'author')",
            params![id, cid, pos as i32],
        )?;
    }

    tx.execute("DELETE FROM paper_keywords WHERE paper_id = ?1", params![id])?;
    for kw in &paper.keywords {
        if kw.trim().is_empty() {
            continue;
        }
        let kid = stable_id("kw", &kw.trim().to_lowercase());
        tx.execute(
            "INSERT OR IGNORE INTO keywords (id, name, source) VALUES (?1, ?2, 'manual')",
            params![kid, kw.trim()],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO paper_keywords (paper_id, keyword_id) VALUES (?1, ?2)",
            params![id, kid],
        )?;
    }

    tx.execute("DELETE FROM identifiers WHERE paper_id = ?1", params![id])?;
    if !paper.doi.trim().is_empty() {
        tx.execute(
            "INSERT OR IGNORE INTO identifiers (paper_id, type, value, is_primary)
             VALUES (?1, 'doi', ?2, 1)",
            params![id, paper.doi.trim()],
        )?;
    }

    // attachments
    tx.execute("DELETE FROM attachments WHERE paper_id = ?1", params![id])?;
    if !paper.pdf_path.trim().is_empty() {
        let aid = stable_id("att-pdf", &format!("{id}|{}", paper.pdf_path));
        tx.execute(
            "INSERT OR IGNORE INTO attachments
                (id, paper_id, kind, rel_path, title, status)
             VALUES (?1, ?2, 'pdf', ?3, 'main', 'active')",
            params![aid, id, paper.pdf_path],
        )?;
    }
    if !paper.note_path.trim().is_empty() {
        let aid = stable_id("att-note", &format!("{id}|{}", paper.note_path));
        tx.execute(
            "INSERT OR IGNORE INTO attachments
                (id, paper_id, kind, rel_path, title, status)
             VALUES (?1, ?2, 'note', ?3, 'main', 'active')",
            params![aid, id, paper.note_path],
        )?;
    }

    rebuild_fts_for_paper(&tx, id)?;
    tx.commit()?;

    // 返回最新值。
    let p = row_to_paper(&conn, id)?
        .ok_or_else(|| AppError::NotFound(format!("论文 {id} 不存在")))?;
    Ok(p)
}

pub fn delete(vault: &Path, id: &str) -> AppResult<()> {
    let conn = db::open(vault)?;
    // FK ON DELETE CASCADE 会自动清理 paper_creators / paper_keywords /
    // attachments / annotations / paper_collections / identifiers /
    // paper_relations / merge_log。
    conn.execute("DELETE FROM papers WHERE id = ?1", params![id])?;
    conn.execute("DELETE FROM papers_fts WHERE paper_id = ?1", params![id])?;
    Ok(())
}

// ============================================================
// P1：从 PaperMetadata 插入（带完整 identifier 列表）
// ============================================================

/// 用 `PaperMetadata` + 稳定主键前缀建 paper。`paper_id_prefix`
/// 推荐 `"meta-doi" / "meta-arxiv" / "meta-pmid" / "meta-isbn"` 之类，
/// 保证同一来源多次导入能复用同一 paper id（idempotent）。
pub fn insert_from_metadata(
    vault: &Path,
    paper_id: &str,
    meta: &crate::types::PaperMetadata,
) -> AppResult<()> {
    let now = chrono::Local::now().timestamp_millis();
    let paper = Paper {
        id: paper_id.to_string(),
        title: meta.title.clone(),
        authors: meta.authors.clone(),
        year: meta.year,
        venue: meta.venue.clone(),
        doi: crate::duplicates::normalize_doi(&meta.doi),
        abstract_text: meta.abstract_text.clone(),
        keywords: meta.keywords.clone(),
        status: PaperStatus::Unread,
        rating: None,
        pdf_path: String::new(),
        note_path: String::new(),
        created_at: now,
        updated_at: now,
    };
    insert(vault, &paper)?;

    // insert() 只插了 DOI 到 identifiers（若有）。这里补其余 scheme。
    let conn = db::open(vault)?;
    for (scheme, val) in &meta.identifiers {
        let s = scheme.trim().to_ascii_lowercase();
        let v = val.trim();
        if v.is_empty() {
            continue;
        }
        // DOI 已经在 insert() 里写过（is_primary=1），避免重复。
        if s == "doi" {
            continue;
        }
        conn.execute(
            "INSERT OR IGNORE INTO identifiers (paper_id, type, value, is_primary)
             VALUES (?1, ?2, ?3, 0)",
            params![paper_id, s, v],
        )?;
    }
    Ok(())
}

/// 更新阅读进度。
pub fn update_progress(
    vault: &Path,
    paper_id: &str,
    current_page: i32,
    total_pages: Option<i32>,
) -> AppResult<crate::types::ReadingProgress> {
    let conn = db::open(vault)?;
    let now = chrono::Local::now().timestamp_millis();
    // 如果传了 total_pages，先获取 metadata（缺省时从 PDF 推断）。
    let resolved_total = if let Some(t) = total_pages {
        t
    } else {
        conn.query_row(
            "SELECT total_pages FROM reading_progress WHERE paper_id = ?1",
            params![paper_id],
            |r| r.get::<_, i32>(0),
        )
        .optional()?
        .unwrap_or(0)
    };
    conn.execute(
        "INSERT INTO reading_progress (paper_id, current_page, total_pages, last_read_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(paper_id) DO UPDATE SET
           current_page = excluded.current_page,
           total_pages = excluded.total_pages,
           last_read_at = excluded.last_read_at",
        params![paper_id, current_page, resolved_total, now],
    )?;
    let pct = if resolved_total > 0 {
        (current_page as f32 / resolved_total as f32) * 100.0
    } else {
        0.0
    };
    Ok(crate::types::ReadingProgress {
        current_page,
        total_pages: resolved_total,
        progress_percent: pct,
        last_read_at: now,
    })
}

// ============================================================
// FTS
// ============================================================

pub fn rebuild_fts_for_paper(conn: &Connection, paper_id: &str) -> AppResult<()> {
    conn.execute(
        "DELETE FROM papers_fts WHERE paper_id = ?1",
        params![paper_id],
    )?;
    conn.execute(
        "INSERT INTO papers_fts
            (paper_id, title, abstract, authors, keywords, venue, doi)
         VALUES (
            ?1,
            (SELECT title FROM papers WHERE id = ?1),
            (SELECT abstract_text FROM papers WHERE id = ?1),
            (SELECT COALESCE(GROUP_CONCAT(c.display_name, ' '), '')
             FROM paper_creators pc JOIN creators c ON c.id = pc.creator_id
             WHERE pc.paper_id = ?1 ORDER BY pc.position),
            (SELECT COALESCE(GROUP_CONCAT(k.name, ' '), '')
             FROM paper_keywords pk JOIN keywords k ON k.id = pk.keyword_id
             WHERE pk.paper_id = ?1),
            (SELECT venue FROM papers WHERE id = ?1),
            (SELECT doi FROM papers WHERE id = ?1)
         )",
        params![paper_id],
    )?;
    Ok(())
}

// ============================================================
// 列表：keywords 全局聚合
// ============================================================

pub fn list_keywords(vault: &Path) -> AppResult<Vec<(String, i64)>> {
    let conn = db::open(vault)?;
    let mut stmt = conn.prepare(
        "SELECT k.name, COUNT(pk.paper_id) AS n
         FROM keywords k
         LEFT JOIN paper_keywords pk ON pk.keyword_id = k.id
         GROUP BY k.id
         ORDER BY n DESC, k.name ASC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
    })?;
    let out: Vec<_> = rows.filter_map(|r| r.ok()).collect();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault;

    fn fresh() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        vault::init_at(dir.path()).unwrap();
        crate::db::migrate(dir.path()).unwrap();
        dir
    }

    fn sample_paper(id: &str) -> Paper {
        Paper {
            id: id.into(),
            title: "Hello".into(),
            authors: vec!["Alice Smith".into(), "Bob".into()],
            year: Some(2024),
            venue: "Nature".into(),
            doi: "10.1109/foo".into(),
            abstract_text: "abs".into(),
            keywords: vec!["ml".into(), "rl".into()],
            status: PaperStatus::Unread,
            rating: Some(4),
            pdf_path: "pdfs/2024/x.pdf".into(),
            note_path: "notes/papers/x.md".into(),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn insert_get_update_delete_roundtrip() {
        let dir = fresh();
        let p = sample_paper("p1");
        insert(dir.path(), &p).unwrap();

        let got = get(dir.path(), "p1").unwrap();
        assert_eq!(got.paper.title, "Hello");
        assert_eq!(got.paper.authors, vec!["Alice Smith", "Bob"]);
        assert_eq!(got.paper.keywords, vec!["ml", "rl"]);
        assert_eq!(got.paper.status, PaperStatus::Unread);
        assert_eq!(got.collections.len(), 0);
        assert_eq!(got.reading_progress.as_ref().map(|p| p.current_page), None);

        // update
        let mut p2 = got.paper.clone();
        p2.status = PaperStatus::Reading;
        p2.keywords = vec!["graph".into()];
        let updated = update(dir.path(), "p1", &p2).unwrap();
        assert_eq!(updated.status, PaperStatus::Reading);
        assert_eq!(updated.keywords, vec!["graph"]);

        // fts row
        let conn = db::open(dir.path()).unwrap();
        let fts_n: i64 = conn
            .query_row("SELECT COUNT(*) FROM papers_fts WHERE paper_id='p1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fts_n, 1);

        delete(dir.path(), "p1").unwrap();
        let conn = db::open(dir.path()).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM papers WHERE id='p1'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn list_filter_by_status() {
        let dir = fresh();
        let mut p = sample_paper("p1");
        insert(dir.path(), &p).unwrap();
        p.id = "p2".into();
        p.status = PaperStatus::Reading;
        insert(dir.path(), &p).unwrap();

        let r = list(dir.path(), Some(PaperStatus::Unread), None, None).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "p1");
    }

    #[test]
    fn list_keywords_aggregates() {
        let dir = fresh();
        let mut p = sample_paper("p1");
        p.keywords = vec!["ml".into()];
        insert(dir.path(), &p).unwrap();
        p.id = "p2".into();
        p.keywords = vec!["ml".into(), "rl".into()];
        insert(dir.path(), &p).unwrap();
        let kws = list_keywords(dir.path()).unwrap();
        let map: std::collections::HashMap<String, i64> = kws.into_iter().collect();
        assert_eq!(map.get("ml").copied(), Some(2));
        assert_eq!(map.get("rl").copied(), Some(1));
    }
}
