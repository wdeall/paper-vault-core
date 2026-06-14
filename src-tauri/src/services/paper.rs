//! 论文 CRUD 服务

use crate::db;
use crate::error::{AppError, AppResult};
use crate::types::{Paper, PaperDetail, ReadingProgress};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

fn row_to_paper(row: &rusqlite::Row<'_>) -> rusqlite::Result<Paper> {
    let authors_json: String = row.get("authors")?;
    let keywords_json: String = row.get("keywords")?;
    let tags_json: String = row.get("tags")?;
    Ok(Paper {
        id: row.get("id")?,
        title: row.get("title")?,
        authors: serde_json::from_str(&authors_json).unwrap_or_default(),
        year: row.get("year")?,
        venue: row.get("venue")?,
        doi: row.get("doi")?,
        abstract_text: row.get("abstract_text")?,
        keywords: serde_json::from_str(&keywords_json).unwrap_or_default(),
        tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        status: row.get("status")?,
        rating: row.get("rating")?,
        pdf_path: row.get("pdf_path")?,
        note_path: row.get("note_path")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn insert(vault: &Path, p: &Paper) -> AppResult<()> {
    let conn = db::open(vault)?;
    conn.execute(
        "INSERT OR REPLACE INTO papers
         (id, title, authors, year, venue, doi, abstract_text, keywords, tags, status, rating, pdf_path, note_path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            p.id, p.title,
            serde_json::to_string(&p.authors)?,
            p.year, p.venue, crate::duplicates::normalize_doi(&p.doi), p.abstract_text,
            serde_json::to_string(&p.keywords)?,
            serde_json::to_string(&p.tags)?,
            p.status, p.rating, p.pdf_path, p.note_path,
            p.created_at, p.updated_at,
        ],
    )?;
    Ok(())
}

pub fn list(
    vault: &Path,
    status: Option<&str>,
    collection_id: Option<&str>,
    tag: Option<&str>,
) -> AppResult<Vec<Paper>> {
    let conn = db::open(vault)?;
    let mut sql = String::from(
        "SELECT DISTINCT p.* FROM papers p
         LEFT JOIN paper_collections pc ON pc.paper_id = p.id
         WHERE 1=1",
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    if let Some(s) = status {
        sql.push_str(" AND p.status = ?");
        args.push(Box::new(s.to_string()));
    }
    if let Some(c) = collection_id {
        sql.push_str(" AND pc.collection_id = ?");
        args.push(Box::new(c.to_string()));
    }
    if let Some(t) = tag {
        sql.push_str(" AND EXISTS (SELECT 1 FROM json_each(p.tags) WHERE value = ?)");
        args.push(Box::new(t.to_string()));
    }
    sql.push_str(" ORDER BY p.updated_at DESC");

    let mut stmt = conn.prepare(&sql)?;
    let arg_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(&arg_refs[..], row_to_paper)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn get(vault: &Path, id: &str) -> AppResult<PaperDetail> {
    let conn = db::open(vault)?;
    let paper = conn
        .query_row(
            "SELECT * FROM papers WHERE id = ?1",
            params![id],
            row_to_paper,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("论文 {id} 不存在")))?;

    let progress: Option<ReadingProgress> = conn
        .query_row(
            "SELECT paper_id, current_page, total_pages, progress_percent, last_read_at
             FROM reading_progress WHERE paper_id = ?1",
            params![id],
            |r| {
                Ok(ReadingProgress {
                    paper_id: r.get(0)?,
                    current_page: r.get(1)?,
                    total_pages: r.get(2)?,
                    progress_percent: r.get(3)?,
                    last_read_at: r.get(4)?,
                })
            },
        )
        .optional()?;

    let index_status: String = conn
        .query_row(
            "SELECT status FROM index_status WHERE paper_id = ?1",
            params![id],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "未索引".into());

    let mut stmt =
        conn.prepare("SELECT collection_id FROM paper_collections WHERE paper_id = ?1")?;
    let collections: Vec<String> = stmt
        .query_map(params![id], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();

    Ok(PaperDetail {
        paper,
        reading_progress: progress,
        index_status,
        collections,
    })
}

pub fn update(vault: &Path, id: &str, p: &Paper) -> AppResult<Paper> {
    let now = chrono::Local::now().timestamp_millis();
    let conn = db::open(vault)?;
    let n = conn.execute(
        "UPDATE papers SET
           title = ?2, authors = ?3, year = ?4, venue = ?5, doi = ?6,
           abstract_text = ?7, keywords = ?8, tags = ?9, status = ?10,
           rating = ?11, pdf_path = ?12, note_path = ?13,
           updated_at = ?14
         WHERE id = ?1",
        params![
            id, p.title,
            serde_json::to_string(&p.authors)?,
            p.year, p.venue, crate::duplicates::normalize_doi(&p.doi), p.abstract_text,
            serde_json::to_string(&p.keywords)?,
            serde_json::to_string(&p.tags)?,
            p.status, p.rating, p.pdf_path, p.note_path,
            now,
        ],
    )?;
    if n == 0 {
        return Err(AppError::NotFound(format!("论文 {id} 不存在")));
    }
    get(vault, id).map(|d| d.paper)
}

pub fn update_progress(
    vault: &Path,
    id: &str,
    current_page: i32,
    total_pages: Option<i32>,
) -> AppResult<ReadingProgress> {
    let now = chrono::Local::now().timestamp_millis();
    let conn = db::open(vault)?;
    // 确认论文存在
    let exists: i64 = conn
        .query_row("SELECT 1 FROM papers WHERE id = ?1", params![id], |r| {
            r.get(0)
        })
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("论文 {id} 不存在")))?;

    let total: i32 = if let Some(t) = total_pages {
        t
    } else {
        conn.query_row(
            "SELECT total_pages FROM reading_progress WHERE paper_id = ?1",
            params![id],
            |r| r.get::<_, i32>(0),
        )
        .unwrap_or(0)
    };
    let percent = if total > 0 {
        (current_page as f32 / total as f32) * 100.0
    } else {
        0.0
    };
    conn.execute(
        "INSERT INTO reading_progress (paper_id, current_page, total_pages, progress_percent, last_read_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(paper_id) DO UPDATE SET
           current_page = excluded.current_page,
           total_pages = excluded.total_pages,
           progress_percent = excluded.progress_percent,
           last_read_at = excluded.last_read_at",
        params![id, current_page, total, percent, now],
    )?;
    Ok(ReadingProgress {
        paper_id: id.into(),
        current_page,
        total_pages: total,
        progress_percent: percent,
        last_read_at: Some(now),
    })
}

pub fn delete(vault: &Path, id: &str, mode: &str) -> AppResult<()> {
    let conn = db::open(vault)?;
    // 取出 pdf_path / note_path
    let paper: Option<Paper> = conn
        .query_row("SELECT * FROM papers WHERE id = ?1", params![id], row_to_paper)
        .optional()?;
    let paper = paper.ok_or_else(|| AppError::NotFound(format!("论文 {id} 不存在")))?;

    conn.execute("DELETE FROM papers WHERE id = ?1", params![id])?;

    if mode == "entry+pdf" || mode == "entry+pdf+note" {
        if !paper.pdf_path.is_empty() {
            let p = std::path::Path::new(&paper.pdf_path);
            if p.exists() {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    if mode == "entry+pdf+note" {
        if !paper.note_path.is_empty() {
            let p = std::path::Path::new(&paper.note_path);
            if p.exists() {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    Ok(())
}
