//! FTS5 全文搜索服务

use crate::db;
use crate::error::{AppError, AppResult};
use crate::pdf;
use crate::types::{IndexStatusSummary, SearchHit, Paper};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub fn reindex_paper(vault: &Path, paper_id: &str) -> AppResult<()> {
    let conn = db::open(vault)?;
    // 删除旧索引行
    conn.execute(
        "DELETE FROM fulltext_index WHERE paper_id = ?1",
        params![paper_id],
    )?;

    // 写 status = 索引中
    let now = chrono::Local::now().timestamp_millis();
    conn.execute(
        "INSERT INTO index_status (paper_id, status, indexed_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(paper_id) DO UPDATE SET status = excluded.status, indexed_at = excluded.indexed_at",
        params![paper_id, "索引中", now],
    )?;

    // 读 paper
    let paper: Paper = conn
        .query_row(
            "SELECT * FROM papers WHERE id = ?1",
            params![paper_id],
            |r| {
                let authors_json: String = r.get("authors")?;
                let keywords_json: String = r.get("keywords")?;
                let tags_json: String = r.get("tags")?;
                Ok(Paper {
                    id: r.get("id")?,
                    title: r.get("title")?,
                    authors: serde_json::from_str(&authors_json).unwrap_or_default(),
                    year: r.get("year")?,
                    venue: r.get("venue")?,
                    doi: r.get("doi")?,
                    abstract_text: r.get("abstract_text")?,
                    keywords: serde_json::from_str(&keywords_json).unwrap_or_default(),
                    tags: serde_json::from_str(&tags_json).unwrap_or_default(),
                    status: r.get("status")?,
                    rating: r.get("rating")?,
                    pdf_path: r.get("pdf_path")?,
                    note_path: r.get("note_path")?,
                    created_at: r.get("created_at")?,
                    updated_at: r.get("updated_at")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("论文 {paper_id} 不存在")))?;

    // title / authors / abstract / keywords
    if !paper.title.is_empty() {
        conn.execute(
            "INSERT INTO fulltext_index (paper_id, source_type, content, page) VALUES (?1, ?2, ?3, ?4)",
            params![paper_id, "title", paper.title, Option::<i32>::None],
        )?;
    }
    if !paper.authors.is_empty() {
        conn.execute(
            "INSERT INTO fulltext_index (paper_id, source_type, content, page) VALUES (?1, ?2, ?3, ?4)",
            params![paper_id, "authors", paper.authors.join(" "), Option::<i32>::None],
        )?;
    }
    if !paper.abstract_text.is_empty() {
        conn.execute(
            "INSERT INTO fulltext_index (paper_id, source_type, content, page) VALUES (?1, ?2, ?3, ?4)",
            params![paper_id, "abstract", paper.abstract_text, Option::<i32>::None],
        )?;
    }
    if !paper.keywords.is_empty() {
        conn.execute(
            "INSERT INTO fulltext_index (paper_id, source_type, content, page) VALUES (?1, ?2, ?3, ?4)",
            params![paper_id, "keywords", paper.keywords.join(" "), Option::<i32>::None],
        )?;
    }
    if !paper.doi.is_empty() {
        conn.execute(
            "INSERT INTO fulltext_index (paper_id, source_type, content, page) VALUES (?1, ?2, ?3, ?4)",
            params![paper_id, "doi", paper.doi.clone(), Option::<i32>::None],
        )?;
    }

    // notes
    if !paper.note_path.is_empty() {
        let np = std::path::Path::new(&paper.note_path);
        if np.exists() {
            if let Ok(text) = std::fs::read_to_string(np) {
                conn.execute(
                    "INSERT INTO fulltext_index (paper_id, source_type, content, page) VALUES (?1, ?2, ?3, ?4)",
                    params![paper_id, "notes", text, Option::<i32>::None],
                )?;
            }
        }
    }

    // pdf
    if !paper.pdf_path.is_empty() {
        let pp = std::path::Path::new(&paper.pdf_path);
        if pp.exists() {
            for (page, text) in pdf::extract_pages(pp) {
                conn.execute(
                    "INSERT INTO fulltext_index (paper_id, source_type, content, page) VALUES (?1, ?2, ?3, ?4)",
                    params![paper_id, "pdf", text, page],
                )?;
            }
        }
    }

    // 写 status = 已索引
    conn.execute(
        "UPDATE index_status SET status = ?2, indexed_at = ?3 WHERE paper_id = ?1",
        params![paper_id, "已索引", chrono::Local::now().timestamp_millis()],
    )?;
    Ok(())
}

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

fn weight_for(source: &str) -> f32 {
    match source {
        "title" => 10.0,
        "authors" | "doi" => 8.0,
        "keywords" => 6.0,
        "abstract" => 4.0,
        "notes" => 3.0,
        "pdf" => 1.0,
        _ => 1.0,
    }
}

fn snippet_around(haystack: &str, needle: &str, half: usize) -> String {
    if needle.is_empty() {
        return haystack.chars().take(half * 2).collect();
    }
    let lower = haystack.to_lowercase();
    let lower_needle = needle.to_lowercase();
    if let Some(pos) = lower.find(&lower_needle) {
        let start = pos.saturating_sub(half);
        let end = (pos + needle.len() + half).min(haystack.len());
        let mut s = String::new();
        if start > 0 {
            s.push('…');
        }
        s.push_str(&haystack[start..end]);
        if end < haystack.len() {
            s.push('…');
        }
        s
    } else {
        haystack.chars().take(half * 2).collect()
    }
}

pub fn search(
    vault: &Path,
    query: &str,
    _scopes: Option<&[String]>,
) -> AppResult<Vec<SearchHit>> {
    let conn = db::open(vault)?;
    // 简单权重：按 source_type 区分。FTS5 自身按 bm25 排，我们再叠权重。
    let fts_query = sanitize_fts_query(query);
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT paper_id, source_type, content, page, bm25(fulltext_index) AS rank
         FROM fulltext_index
         WHERE fulltext_index MATCH ?1
         LIMIT 200",
    )?;
    let rows = stmt.query_map(params![fts_query], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, Option<i32>>(3)?,
            r.get::<_, f32>(4)?,
        ))
    })?;

    let mut hits: Vec<SearchHit> = Vec::new();
    for r in rows {
        let (pid, src, content, page, rank) = r?;
        let base = 1.0 / (1.0 + rank.max(0.0));
        let score = base * weight_for(&src);
        let snippet = snippet_around(&content, query, 40);
        hits.push(SearchHit {
            paper_id: pid,
            source_type: src,
            snippet,
            page,
            score,
        });
    }

    // 二次排序：score desc
    hits.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    hits.truncate(100);
    Ok(hits)
}

fn sanitize_fts_query(q: &str) -> String {
    // FTS5 关键字中含特殊字符会报错。这里只保留字母数字与空格并加引号包裹。
    let cleaned: String = q
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '_')
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    format!("\"{}\"", trimmed.replace('"', " "))
}

pub fn status_summary(vault: &Path) -> AppResult<IndexStatusSummary> {
    let conn = db::open(vault)?;
    let mut sum = IndexStatusSummary::default();
    sum.total = conn
        .query_row("SELECT COUNT(*) FROM papers", [], |r| r.get(0))?;
    let mut stmt = conn.prepare("SELECT status, COUNT(*) FROM index_status GROUP BY status")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
    for r in rows {
        let (s, n) = r?;
        match s.as_str() {
            "已索引" => sum.indexed = n,
            "索引中" => sum.indexing = n,
            "索引失败" => sum.failed = n,
            _ => sum.pending += n,
        }
    }
    sum.pending = sum.total - sum.indexed - sum.indexing - sum.failed;
    Ok(sum)
}
