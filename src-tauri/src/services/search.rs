//! P3 双通道搜索: Structured + Fulltext + Both
//!
//! - Structured: 按 StructuredQuery 字段 AND 组合过滤 papers。
//! - Fulltext:   papers_fts MATCH + bm25 排序。
//! - Both:       先 FTS 命中再 Structured 过滤,结果带 FTS score。
//!
//! 辅助函数 `sanitize_fts_query` / `snippet_around` / `weight_for`
//! 从 services/index.rs 迁移过来。

use crate::db;
use crate::duplicates::normalize_doi;
use crate::error::AppResult;
use crate::types::{PaperSummary, SearchHit, StructuredQuery};
use rusqlite::{params, Connection};
use std::path::Path;

// ============================================================
// FTS 同步
// ============================================================

/// 单篇论文同步到 papers_fts。先 DELETE 该 paper_id 的旧行,再从
/// papers + paper_creators + creators + paper_keywords + keywords
/// 拼数据 INSERT。参考 `db::migrate_v2::rebuild_papers_fts` 的 SQL
/// (那个是全量重建,这个是单篇)。
pub fn sync_papers_fts(conn: &Connection, paper_id: &str) -> AppResult<()> {
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

/// 全量重建 papers_fts: DELETE ALL + 遍历所有 paper_id 调 sync_papers_fts。
#[allow(dead_code)]
pub fn reindex_papers_fts_all(vault: &Path) -> AppResult<()> {
    let conn = db::open(vault)?;
    conn.execute("DELETE FROM papers_fts", [])?;
    let mut stmt = conn.prepare("SELECT id FROM papers")?;
    let ids: Vec<String> = stmt
        .query_map([], |r| r.get::<_, String>(0))?
        .filter_map(|r| r.ok())
        .collect();
    drop(stmt);
    for id in ids {
        sync_papers_fts(&conn, &id)?;
    }
    Ok(())
}

// ============================================================
// Structured 搜索
// ============================================================

/// 动态拼 SQL,AND 组合所有提供的字段。结果按 `p.updated_at DESC`
/// 排序,映射到 PaperSummary (authors 从 paper_creators+creators 查)。
pub fn search_structured(
    vault: &Path,
    query: &StructuredQuery,
) -> AppResult<Vec<PaperSummary>> {
    let conn = db::open(vault)?;
    let mut sql = String::from(
        "SELECT p.id, p.title, p.year, p.venue, p.status, p.rating, p.updated_at
         FROM papers p
         WHERE 1=1",
    );
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(t) = query.title.as_ref() {
        let t = t.trim();
        if !t.is_empty() {
            sql.push_str(" AND p.title LIKE ?");
            args.push(Box::new(format!("%{t}%")));
        }
    }
    if let Some(a) = query.author.as_ref() {
        let a = a.trim();
        if !a.is_empty() {
            sql.push_str(
                " AND EXISTS (SELECT 1 FROM paper_creators pc \
                 JOIN creators c ON c.id = pc.creator_id \
                 WHERE pc.paper_id = p.id AND c.display_name LIKE ?)",
            );
            args.push(Box::new(format!("%{a}%")));
        }
    }
    if let Some(y) = query.year.as_ref() {
        sql.push_str(" AND p.year = ?");
        args.push(Box::new(*y));
    }
    if let Some(v) = query.venue.as_ref() {
        let v = v.trim();
        if !v.is_empty() {
            sql.push_str(" AND p.venue LIKE ?");
            args.push(Box::new(format!("%{v}%")));
        }
    }
    if let Some(d) = query.doi.as_ref() {
        let nd = normalize_doi(d);
        if !nd.is_empty() {
            sql.push_str(" AND p.doi = ?");
            args.push(Box::new(nd));
        }
    }
    if let Some(s) = query.status.as_ref() {
        let s = s.trim();
        if !s.is_empty() {
            sql.push_str(" AND p.status = ?");
            args.push(Box::new(s.to_string()));
        }
    }
    if let Some(k) = query.keyword.as_ref() {
        let k = k.trim();
        if !k.is_empty() {
            sql.push_str(
                " AND EXISTS (SELECT 1 FROM paper_keywords pk \
                 JOIN keywords k ON k.id = pk.keyword_id \
                 WHERE pk.paper_id = p.id AND k.name LIKE ?)",
            );
            args.push(Box::new(format!("%{k}%")));
        }
    }

    sql.push_str(" ORDER BY p.updated_at DESC");

    let mut stmt = conn.prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| &**b).collect();
    let rows = stmt.query_map(params_refs.as_slice(), |r| {
        Ok(PaperSummary {
            id: r.get::<_, String>(0)?,
            title: r.get::<_, String>(1)?,
            year: r.get::<_, Option<i32>>(2)?,
            authors: Vec::new(), // 稍后填充
            venue: r.get::<_, String>(3)?,
            status: r.get::<_, String>(4)?,
            rating: r.get::<_, Option<i32>>(5)?,
            score: None,
        })
    })?;

    let mut out: Vec<PaperSummary> = Vec::new();
    for r in rows {
        let mut s: PaperSummary = r?;
        s.authors = load_authors(&conn, &s.id)?;
        out.push(s);
    }
    Ok(out)
}

fn load_authors(conn: &Connection, paper_id: &str) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT c.display_name
         FROM paper_creators pc
         JOIN creators c ON c.id = pc.creator_id
         WHERE pc.paper_id = ?1 AND pc.role = 'author'
         ORDER BY pc.position",
    )?;
    let rows = stmt.query_map(params![paper_id], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

// ============================================================
// Fulltext 搜索
// ============================================================

/// 查 papers_fts。bm25 越小越好 (返回负值),
/// score = 1.0 / (1.0 + rank.abs())。
pub fn search_fulltext(
    vault: &Path,
    q: &str,
    limit: Option<usize>,
) -> AppResult<Vec<SearchHit>> {
    let fts_query = sanitize_fts_query(q);
    if fts_query.is_empty() {
        return Ok(Vec::new());
    }
    let limit = limit.unwrap_or(50);
    let conn = db::open(vault)?;
    let mut stmt = conn.prepare(
        "SELECT paper_id, title, abstract, authors, keywords, venue, doi,
                bm25(papers_fts) AS rank
         FROM papers_fts
         WHERE papers_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![fts_query, limit as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,       // paper_id
            r.get::<_, String>(1)?,       // title
            r.get::<_, String>(2)?,       // abstract
            r.get::<_, String>(3)?,       // authors
            r.get::<_, String>(4)?,       // keywords
            r.get::<_, String>(5)?,       // venue
            r.get::<_, String>(6)?,       // doi
            r.get::<_, f32>(7)?,          // rank (bm25)
        ))
    })?;

    let mut hits: Vec<SearchHit> = Vec::new();
    for r in rows {
        let (pid, title, abstract_, authors, keywords, venue, doi, rank) = r?;
        // score = 1.0 / (1.0 + |rank|)
        let score = 1.0 / (1.0 + rank.abs());
        // 找到第一个包含查询词的字段,作为 source_type
        let needle = q.trim().to_lowercase();
        let (source_type, snippet_src) = first_match_field(
            &needle,
            &[
                ("title", title.as_str()),
                ("abstract", abstract_.as_str()),
                ("authors", authors.as_str()),
                ("keywords", keywords.as_str()),
                ("venue", venue.as_str()),
                ("doi", doi.as_str()),
            ],
        );
        let snippet = snippet_around(&snippet_src, q, 40);
        hits.push(SearchHit {
            paper_id: pid,
            source_type: source_type.to_string(),
            snippet,
            page: None,
            score,
        });
    }
    Ok(hits)
}

fn first_match_field<'a>(
    needle: &str,
    fields: &[(&'a str, &'a str)],
) -> (&'a str, String) {
    if needle.is_empty() {
        // 空查询: 默认 title,内容取 title 原值
        for (name, val) in fields {
            if !val.is_empty() {
                return (name, val.to_string());
            }
        }
        return ("title", String::new());
    }
    for (name, val) in fields {
        if val.to_lowercase().contains(needle) {
            return (name, val.to_string());
        }
    }
    // 兜底: 取第一个非空字段
    for (name, val) in fields {
        if !val.is_empty() {
            return (name, val.to_string());
        }
    }
    ("title", String::new())
}

// ============================================================
// Both 搜索
// ============================================================

/// 先 search_fulltext 拿到 paper_id 集合,再用 StructuredQuery 条件过滤。
/// 结果带 score (FTS score)。
pub fn search_both(
    vault: &Path,
    query: &StructuredQuery,
    fts_query: &str,
    limit: Option<usize>,
) -> AppResult<Vec<PaperSummary>> {
    let fts_hits = search_fulltext(vault, fts_query, limit)?;
    if fts_hits.is_empty() {
        return Ok(Vec::new());
    }
    // 收集 (paper_id, score)
    let mut score_map: std::collections::HashMap<String, f32> =
        std::collections::HashMap::with_capacity(fts_hits.len());
    for h in &fts_hits {
        // 同一 paper 可能多次命中,保留最大 score
        let e = score_map.entry(h.paper_id.clone()).or_insert(0.0);
        if h.score > *e {
            *e = h.score;
        }
    }
    let ids: Vec<String> = score_map.keys().cloned().collect();

    // 在 SQL 里加 `AND p.id IN (...)` 子查询过滤
    let conn = db::open(vault)?;
    let placeholders: Vec<String> = (0..ids.len())
        .map(|i| format!("?{}", i + 1))
        .collect();
    let in_clause = placeholders.join(",");
    let mut sql = String::from(
        "SELECT p.id, p.title, p.year, p.venue, p.status, p.rating, p.updated_at
         FROM papers p
         WHERE p.id IN (",
    );
    sql.push_str(&in_clause);
    sql.push(')');

    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();
    for id in &ids {
        args.push(Box::new(id.clone()));
    }

    if let Some(t) = query.title.as_ref() {
        let t = t.trim();
        if !t.is_empty() {
            sql.push_str(" AND p.title LIKE ?");
            args.push(Box::new(format!("%{t}%")));
        }
    }
    if let Some(a) = query.author.as_ref() {
        let a = a.trim();
        if !a.is_empty() {
            sql.push_str(
                " AND EXISTS (SELECT 1 FROM paper_creators pc \
                 JOIN creators c ON c.id = pc.creator_id \
                 WHERE pc.paper_id = p.id AND c.display_name LIKE ?)",
            );
            args.push(Box::new(format!("%{a}%")));
        }
    }
    if let Some(y) = query.year.as_ref() {
        sql.push_str(" AND p.year = ?");
        args.push(Box::new(*y));
    }
    if let Some(v) = query.venue.as_ref() {
        let v = v.trim();
        if !v.is_empty() {
            sql.push_str(" AND p.venue LIKE ?");
            args.push(Box::new(format!("%{v}%")));
        }
    }
    if let Some(d) = query.doi.as_ref() {
        let nd = normalize_doi(d);
        if !nd.is_empty() {
            sql.push_str(" AND p.doi = ?");
            args.push(Box::new(nd));
        }
    }
    if let Some(s) = query.status.as_ref() {
        let s = s.trim();
        if !s.is_empty() {
            sql.push_str(" AND p.status = ?");
            args.push(Box::new(s.to_string()));
        }
    }
    if let Some(k) = query.keyword.as_ref() {
        let k = k.trim();
        if !k.is_empty() {
            sql.push_str(
                " AND EXISTS (SELECT 1 FROM paper_keywords pk \
                 JOIN keywords k ON k.id = pk.keyword_id \
                 WHERE pk.paper_id = p.id AND k.name LIKE ?)",
            );
            args.push(Box::new(format!("%{k}%")));
        }
    }

    sql.push_str(" ORDER BY p.updated_at DESC");

    let mut stmt = conn.prepare(&sql)?;
    let params_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| &**b).collect();
    let rows = stmt.query_map(params_refs.as_slice(), |r| {
        Ok(PaperSummary {
            id: r.get::<_, String>(0)?,
            title: r.get::<_, String>(1)?,
            year: r.get::<_, Option<i32>>(2)?,
            authors: Vec::new(),
            venue: r.get::<_, String>(3)?,
            status: r.get::<_, String>(4)?,
            rating: r.get::<_, Option<i32>>(5)?,
            score: None,
        })
    })?;

    let mut out: Vec<PaperSummary> = Vec::new();
    for r in rows {
        let mut s: PaperSummary = r?;
        s.authors = load_authors(&conn, &s.id)?;
        s.score = score_map.get(&s.id).copied();
        out.push(s);
    }
    Ok(out)
}

// ============================================================
// 辅助函数 (从 services/index.rs 迁移)
// ============================================================

/// FTS5 关键字中含特殊字符会报错。这里只保留字母数字与空格并加引号包裹。
pub(crate) fn sanitize_fts_query(q: &str) -> String {
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

/// 在 `haystack` 中截取 `needle` 上下文。
pub(crate) fn snippet_around(haystack: &str, needle: &str, half: usize) -> String {
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

#[allow(dead_code)]
fn weight_for(source: &str) -> f32 {
    match source {
        "title" => 10.0,
        "authors" | "doi" => 8.0,
        "keywords" => 6.0,
        "abstract" => 4.0,
        "venue" => 3.0,
        _ => 1.0,
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::vault;
    use rusqlite::params;

    fn fresh_vault() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        vault::init_at(&path).unwrap();
        db::migrate(&path).unwrap();
        dir
    }

    /// 插入一篇论文 (含 creators + keywords),返回 paper_id。
    fn insert_paper(
        conn: &Connection,
        id: &str,
        title: &str,
        year: Option<i32>,
        venue: &str,
        doi: &str,
        abstract_text: &str,
        status: &str,
        authors: &[&str],
        keywords: &[&str],
    ) {
        let now = chrono::Local::now().timestamp_millis();
        conn.execute(
            "INSERT INTO papers (id, title, year, venue, doi, abstract_text, status,
                                 pdf_path, note_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, '', '', ?8, ?8)",
            params![id, title, year, venue, doi, abstract_text, status, now],
        )
        .unwrap();
        for (pos, name) in authors.iter().enumerate() {
            let (family, given) = split_name(name);
            let cid = crate::services::paper::stable_id(
                "cr",
                &format!("{family}|{given}|{name}").to_lowercase(),
            );
            conn.execute(
                "INSERT OR IGNORE INTO creators (id, family_name, given_name, display_name, raw)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![cid, family, given, name, name],
            )
            .unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO paper_creators (paper_id, creator_id, position, role)
                 VALUES (?1, ?2, ?3, 'author')",
                params![id, cid, pos as i32],
            )
            .unwrap();
        }
        for kw in keywords {
            let kid = crate::services::paper::stable_id("kw", &kw.to_lowercase());
            conn.execute(
                "INSERT OR IGNORE INTO keywords (id, name, source) VALUES (?1, ?2, 'manual')",
                params![kid, kw],
            )
            .unwrap();
            conn.execute(
                "INSERT OR IGNORE INTO paper_keywords (paper_id, keyword_id) VALUES (?1, ?2)",
                params![id, kid],
            )
            .unwrap();
        }
        // 同步到 papers_fts
        sync_papers_fts(conn, id).unwrap();
    }

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

    #[test]
    fn test_structured_search_by_title() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(
            &conn,
            "p1",
            "Deep Learning for NLP",
            Some(2020),
            "ACL",
            "",
            "",
            "unread",
            &[],
            &[],
        );
        insert_paper(
            &conn,
            "p2",
            "Reinforcement Learning Basics",
            Some(2021),
            "NeurIPS",
            "",
            "",
            "unread",
            &[],
            &[],
        );
        let q = StructuredQuery {
            title: Some("Deep".into()),
            ..Default::default()
        };
        let r = search_structured(dir.path(), &q).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "p1");
    }

    #[test]
    fn test_structured_search_by_year() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "p1", "A", Some(2020), "", "", "", "unread", &[], &[]);
        insert_paper(&conn, "p2", "B", Some(2021), "", "", "", "unread", &[], &[]);
        let q = StructuredQuery {
            year: Some(2021),
            ..Default::default()
        };
        let r = search_structured(dir.path(), &q).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "p2");
    }

    #[test]
    fn test_structured_search_by_author() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(
            &conn,
            "p1",
            "Paper One",
            None,
            "",
            "",
            "",
            "unread",
            &["Alice Smith", "Bob"],
            &[],
        );
        insert_paper(
            &conn,
            "p2",
            "Paper Two",
            None,
            "",
            "",
            "",
            "unread",
            &["Carol"],
            &[],
        );
        let q = StructuredQuery {
            author: Some("Alice".into()),
            ..Default::default()
        };
        let r = search_structured(dir.path(), &q).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "p1");
        assert_eq!(r[0].authors, vec!["Alice Smith", "Bob"]);
    }

    #[test]
    fn test_structured_search_by_status() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "p1", "A", None, "", "", "", "unread", &[], &[]);
        insert_paper(&conn, "p2", "B", None, "", "", "", "read", &[], &[]);
        let q = StructuredQuery {
            status: Some("read".into()),
            ..Default::default()
        };
        let r = search_structured(dir.path(), &q).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "p2");
    }

    #[test]
    fn test_structured_search_multi_field() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(
            &conn,
            "p1",
            "Deep Learning",
            Some(2020),
            "",
            "",
            "",
            "unread",
            &[],
            &[],
        );
        insert_paper(
            &conn,
            "p2",
            "Deep Learning",
            Some(2021),
            "",
            "",
            "",
            "unread",
            &[],
            &[],
        );
        // title + year AND
        let q = StructuredQuery {
            title: Some("Deep".into()),
            year: Some(2021),
            ..Default::default()
        };
        let r = search_structured(dir.path(), &q).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "p2");
    }

    #[test]
    fn test_fulltext_search_basic() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(
            &conn,
            "p1",
            "Attention Is All You Need",
            None,
            "",
            "",
            "We propose a new architecture called Transformer",
            "unread",
            &[],
            &[],
        );
        insert_paper(
            &conn,
            "p2",
            "Cooking Recipes",
            None,
            "",
            "",
            "How to make pasta",
            "unread",
            &[],
            &[],
        );
        let r = search_fulltext(dir.path(), "Transformer", None).unwrap();
        assert!(!r.is_empty());
        assert_eq!(r[0].paper_id, "p1");
        // source_type 应是命中字段之一
        assert!(matches!(
            r[0].source_type.as_str(),
            "title" | "abstract" | "authors" | "keywords" | "venue" | "doi"
        ));
    }

    #[test]
    fn test_fulltext_search_empty_query() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "p1", "Hello", None, "", "", "", "unread", &[], &[]);
        let r = search_fulltext(dir.path(), "", None).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn test_both_mode_intersection() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(
            &conn,
            "p1",
            "Deep Learning",
            Some(2020),
            "",
            "",
            "neural networks",
            "unread",
            &[],
            &[],
        );
        insert_paper(
            &conn,
            "p2",
            "Deep Learning",
            Some(2021),
            "",
            "",
            "neural networks",
            "read",
            &[],
            &[],
        );
        // FTS 命中两篇,Structured 过滤 year=2021 只剩 p2
        let sq = StructuredQuery {
            year: Some(2021),
            ..Default::default()
        };
        let r = search_both(dir.path(), &sq, "Deep", None).unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "p2");
        assert!(r[0].score.is_some(), "Both 模式应带 FTS score");
    }

    #[test]
    fn test_sync_papers_fts_single() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        // 直接写 papers 表 (不走 insert_paper 的 sync 步骤)
        let now = chrono::Local::now().timestamp_millis();
        conn.execute(
            "INSERT INTO papers (id, title, year, venue, doi, abstract_text, status,
                                 pdf_path, note_path, created_at, updated_at)
             VALUES ('p1', 'Manual', 2020, 'V', 'd', 'abs', 'unread', '', '', ?1, ?1)",
            params![now],
        )
        .unwrap();
        // 同步前 papers_fts 应无该行
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM papers_fts WHERE paper_id='p1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
        // 同步
        sync_papers_fts(&conn, "p1").unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM papers_fts WHERE paper_id='p1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
        let title: String = conn
            .query_row(
                "SELECT title FROM papers_fts WHERE paper_id='p1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(title, "Manual");
    }

    #[test]
    fn test_reindex_all_fts() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        // 写两篇 paper 但不调 sync_papers_fts
        let now = chrono::Local::now().timestamp_millis();
        for i in 1..=2 {
            conn.execute(
                "INSERT INTO papers (id, title, year, venue, doi, abstract_text, status,
                                     pdf_path, note_path, created_at, updated_at)
                 VALUES (?1, ?2, 2020, '', '', '', 'unread', '', '', ?3, ?3)",
                params![format!("p{i}"), format!("Paper {i}"), now],
            )
            .unwrap();
        }
        // 全量重建
        reindex_papers_fts_all(dir.path()).unwrap();
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM papers_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 2);
    }
}
