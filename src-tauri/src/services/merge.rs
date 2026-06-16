//! P2：去重合并 + 5 分钟撤销。
//!
//! 字段级合并策略（见 SPEC §3.3.4）：
//!
//!   - title / year / venue / abstract_text：src 补空，dst 已有保留 dst。
//!   - keywords auto：替换为 src 全部 auto。
//!   - keywords manual：并集。
//!   - status / rating / note_path / reading_progress：保留 dst。
//!   - attachments：并集去重（按 sha256 或 rel_path）。
//!   - annotations：并集（按 (attachment_id, page, rect, text)）。
//!   - paper_creators：按 display_name 合并去重，dst position 优先。
//!   - paper_relations：并集（src→其它 / 其它→src / src→dst 全转 dst）。
//!   - identifiers：并集（UNIQUE 约束保证）。
//!
//! 撤销机制：
//!   1. 合并前对 src + dst 都做完整 snapshot（论文 + 全部子表行），存
//!      `merge_log.snapshot` JSON。
//!   2. `merged_at > now - 300_000 ms` 才能撤销。
//!   3. 撤销时把两个 paper 的全部相关表恢复成 snapshot。
//!   4. 撤销成功后删除对应 `merge_log` 行；启动或每次合并后清理
//!      `merged_at < now - 300_000` 的旧行（不再可撤销）。

use crate::db;
use crate::error::{AppError, AppResult};
use crate::types::{Annotation, Attachment, Paper, PaperRelation};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 5 分钟撤销窗口（ms）。
pub const UNDO_WINDOW_MS: i64 = 5 * 60 * 1000;

// ============================================================
// 公开结构
// ============================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    /// merge_log 主键。
    pub merge_id: i64,
    /// 保留的主论文。
    pub canonical_id: String,
    /// 被合并掉的论文。
    pub duplicate_id: String,
    /// 本次合并实际改动了哪些字段（dst 侧的）。
    pub fields_merged: Vec<String>,
    /// 合并时刻（毫秒）。
    pub merged_at: i64,
}

// ============================================================
// Snapshot
// ============================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CreatorSnapshot {
    pub id: String,
    pub family_name: String,
    pub given_name: String,
    pub display_name: String,
    pub raw: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PaperCreatorSnapshot {
    pub creator: CreatorSnapshot,
    pub position: i32,
    pub role: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KeywordSnapshot {
    pub id: String,
    pub name: String,
    pub source: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IdentifierSnapshot {
    #[serde(rename = "type")]
    pub kind: String,
    pub value: String,
    pub is_primary: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadingProgressSnapshot {
    pub current_page: i32,
    pub total_pages: i32,
    pub progress_percent: f64,
    pub last_read_at: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PaperSnapshot {
    pub paper: Paper,
    /// paper_creators 完整快照（含 creator 自身）。
    pub creators: Vec<PaperCreatorSnapshot>,
    /// paper_keywords 完整快照（含 keyword 自身）。
    pub keywords: Vec<KeywordSnapshot>,
    /// 该 paper 所在的 collection id 列表。
    pub collection_ids: Vec<String>,
    pub attachments: Vec<Attachment>,
    pub annotations: Vec<Annotation>,
    pub identifiers: Vec<IdentifierSnapshot>,
    /// paper_relations 两端任一为该 paper 的行。
    pub relations: Vec<PaperRelation>,
    pub reading_progress: Option<ReadingProgressSnapshot>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MergeSnapshot {
    pub src: PaperSnapshot,
    pub dst: PaperSnapshot,
}

// ============================================================
// 内部工具
// ============================================================

fn take_paper_snapshot(conn: &Connection, paper_id: &str) -> AppResult<PaperSnapshot> {
    let paper = conn.query_row(
        "SELECT id, title, year, venue, doi, abstract_text, status, rating,
                pdf_path, note_path, created_at, updated_at
         FROM papers WHERE id = ?1",
        params![paper_id],
        |row| {
            let status_str: String = row.get(6)?;
            let authors: Vec<String> = Vec::new();
            let keywords: Vec<String> = Vec::new();
            Ok(Paper {
                id: row.get(0)?,
                title: row.get(1)?,
                authors,
                year: row.get(2)?,
                venue: row.get(3)?,
                doi: row.get(4)?,
                abstract_text: row.get(5)?,
                keywords,
                status: crate::types::PaperStatus::parse(&status_str),
                rating: row.get(7)?,
                pdf_path: row.get(8)?,
                note_path: row.get(9)?,
                created_at: row.get(10)?,
                updated_at: row.get(11)?,
            })
        },
    )?;

    let mut snap = PaperSnapshot {
        paper,
        ..Default::default()
    };

    // creators（via paper_creators JOIN creators）
    let mut stmt = conn.prepare(
        "SELECT c.id, c.family_name, c.given_name, c.display_name, c.raw,
                pc.position, pc.role
         FROM paper_creators pc
         JOIN creators c ON c.id = pc.creator_id
         WHERE pc.paper_id = ?1
         ORDER BY pc.position",
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(PaperCreatorSnapshot {
            creator: CreatorSnapshot {
                id: row.get(0)?,
                family_name: row.get(1)?,
                given_name: row.get(2)?,
                display_name: row.get(3)?,
                raw: row.get(4)?,
            },
            position: row.get(5)?,
            role: row.get(6)?,
        })
    })?;
    for r in rows {
        snap.creators.push(r?);
    }

    // keywords
    let mut stmt = conn.prepare(
        "SELECT k.id, k.name, k.source
         FROM paper_keywords pk
         JOIN keywords k ON k.id = pk.keyword_id
         WHERE pk.paper_id = ?1",
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(KeywordSnapshot {
            id: row.get(0)?,
            name: row.get(1)?,
            source: row.get(2)?,
        })
    })?;
    for r in rows {
        snap.keywords.push(r?);
    }

    // collection_ids
    let mut stmt = conn.prepare(
        "SELECT collection_id FROM paper_collections WHERE paper_id = ?1",
    )?;
    let rows = stmt.query_map(params![paper_id], |row| row.get::<_, String>(0))?;
    for r in rows {
        snap.collection_ids.push(r?);
    }

    // attachments
    let mut stmt = conn.prepare(
        "SELECT id, paper_id, kind, rel_path, abs_path, mime_type, title, frontmatter,
                sha256, imported_at, status
         FROM attachments WHERE paper_id = ?1",
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(Attachment {
            id: row.get(0)?,
            paper_id: row.get(1)?,
            kind: row.get(2)?,
            rel_path: row.get(3)?,
            abs_path: row.get(4)?,
            mime_type: row.get(5)?,
            title: row.get(6)?,
            frontmatter: row.get(7)?,
            sha256: row.get(8)?,
            imported_at: row.get(9)?,
            status: row.get(10)?,
        })
    })?;
    for r in rows {
        snap.attachments.push(r?);
    }

    // annotations
    let mut stmt = conn.prepare(
        "SELECT id, paper_id, attachment_id, kind, page, rect, color, text, comment,
                created_at, modified_at
         FROM annotations WHERE paper_id = ?1",
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(Annotation {
            id: row.get(0)?,
            paper_id: row.get(1)?,
            attachment_id: row.get(2)?,
            kind: row.get(3)?,
            page: row.get(4)?,
            rect: row.get(5)?,
            color: row.get(6)?,
            text: row.get(7)?,
            comment: row.get(8)?,
            created_at: row.get(9)?,
            modified_at: row.get(10)?,
        })
    })?;
    for r in rows {
        snap.annotations.push(r?);
    }

    // identifiers
    let mut stmt = conn.prepare(
        "SELECT [type], value, is_primary FROM identifiers WHERE paper_id = ?1",
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(IdentifierSnapshot {
            kind: row.get(0)?,
            value: row.get(1)?,
            is_primary: row.get(2)?,
        })
    })?;
    for r in rows {
        snap.identifiers.push(r?);
    }

    // paper_relations（src 或 dst 端为该 paper 的全部）
    let mut stmt = conn.prepare(
        "SELECT id, src_paper_id, dst_paper_id, relation, note, created_at
         FROM paper_relations
         WHERE src_paper_id = ?1 OR dst_paper_id = ?1",
    )?;
    let rows = stmt.query_map(params![paper_id], |row| {
        Ok(PaperRelation {
            id: row.get(0)?,
            src_paper_id: row.get(1)?,
            dst_paper_id: row.get(2)?,
            relation: row.get(3)?,
            note: row.get(4)?,
            created_at: row.get(5)?,
        })
    })?;
    for r in rows {
        snap.relations.push(r?);
    }

    // reading_progress
    snap.reading_progress = conn
        .query_row(
            "SELECT current_page, total_pages, progress_percent, last_read_at
             FROM reading_progress WHERE paper_id = ?1",
            params![paper_id],
            |row| {
                Ok(ReadingProgressSnapshot {
                    current_page: row.get(0)?,
                    total_pages: row.get(1)?,
                    progress_percent: row.get(2)?,
                    last_read_at: row.get(3)?,
                })
            },
        )
        .optional()?;

    Ok(snap)
}

fn restore_paper_snapshot(conn: &Connection, snap: &PaperSnapshot) -> AppResult<()> {
    let pid = &snap.paper.id;

    // 1) paper 主表：用 upsert（INSERT OR REPLACE）。
    conn.execute(
        "INSERT OR REPLACE INTO papers
            (id, title, year, venue, doi, abstract_text, status, rating,
             pdf_path, note_path, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            pid,
            snap.paper.title,
            snap.paper.year,
            snap.paper.venue,
            snap.paper.doi,
            snap.paper.abstract_text,
            snap.paper.status.as_str(),
            snap.paper.rating,
            snap.paper.pdf_path,
            snap.paper.note_path,
            snap.paper.created_at,
            snap.paper.updated_at,
        ],
    )?;

    // 2) paper_creators + creators：先删后插。
    conn.execute("DELETE FROM paper_creators WHERE paper_id = ?1", params![pid])?;
    for c in &snap.creators {
        conn.execute(
            "INSERT OR IGNORE INTO creators
                (id, family_name, given_name, display_name, raw)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![c.creator.id, c.creator.family_name, c.creator.given_name, c.creator.display_name, c.creator.raw],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO paper_creators
                (paper_id, creator_id, position, role)
             VALUES (?1, ?2, ?3, ?4)",
            params![pid, c.creator.id, c.position, c.role],
        )?;
    }

    // 3) paper_keywords + keywords
    conn.execute("DELETE FROM paper_keywords WHERE paper_id = ?1", params![pid])?;
    for k in &snap.keywords {
        conn.execute(
            "INSERT OR IGNORE INTO keywords (id, name, source) VALUES (?1, ?2, ?3)",
            params![k.id, k.name, k.source],
        )?;
        conn.execute(
            "INSERT OR REPLACE INTO paper_keywords (paper_id, keyword_id) VALUES (?1, ?2)",
            params![pid, k.id],
        )?;
    }

    // 4) paper_collections
    conn.execute("DELETE FROM paper_collections WHERE paper_id = ?1", params![pid])?;
    for cid in &snap.collection_ids {
        conn.execute(
            "INSERT OR IGNORE INTO paper_collections (paper_id, collection_id) VALUES (?1, ?2)",
            params![pid, cid],
        )?;
    }

    // 5) attachments
    conn.execute("DELETE FROM attachments WHERE paper_id = ?1", params![pid])?;
    for a in &snap.attachments {
        conn.execute(
            "INSERT OR REPLACE INTO attachments
                (id, paper_id, kind, rel_path, abs_path, mime_type, title, frontmatter,
                 sha256, imported_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                a.id, a.paper_id, a.kind, a.rel_path, a.abs_path, a.mime_type,
                a.title, a.frontmatter, a.sha256, a.imported_at, a.status,
            ],
        )?;
    }

    // 6) annotations
    conn.execute("DELETE FROM annotations WHERE paper_id = ?1", params![pid])?;
    for ann in &snap.annotations {
        conn.execute(
            "INSERT OR REPLACE INTO annotations
                (id, paper_id, attachment_id, kind, page, rect, color, text, comment,
                 created_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                ann.id, ann.paper_id, ann.attachment_id, ann.kind, ann.page, ann.rect,
                ann.color, ann.text, ann.comment, ann.created_at, ann.modified_at,
            ],
        )?;
    }

    // 7) identifiers
    conn.execute("DELETE FROM identifiers WHERE paper_id = ?1", params![pid])?;
    for id in &snap.identifiers {
        conn.execute(
            "INSERT OR REPLACE INTO identifiers (paper_id, [type], value, is_primary)
             VALUES (?1, ?2, ?3, ?4)",
            params![pid, id.kind, id.value, id.is_primary],
        )?;
    }

    // 8) paper_relations
    conn.execute(
        "DELETE FROM paper_relations WHERE src_paper_id = ?1 OR dst_paper_id = ?1",
        params![pid],
    )?;
    for r in &snap.relations {
        // dst 端为该 paper 的关系，src→dst 互换后再插（保持原 src/dst 方向）。
        conn.execute(
            "INSERT OR REPLACE INTO paper_relations
                (id, src_paper_id, dst_paper_id, relation, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![r.id, r.src_paper_id, r.dst_paper_id, r.relation, r.note, r.created_at],
        )?;
    }

    // 9) reading_progress
    conn.execute("DELETE FROM reading_progress WHERE paper_id = ?1", params![pid])?;
    if let Some(rp) = &snap.reading_progress {
        conn.execute(
            "INSERT OR REPLACE INTO reading_progress
                (paper_id, current_page, total_pages, progress_percent, last_read_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![pid, rp.current_page, rp.total_pages, rp.progress_percent, rp.last_read_at],
        )?;
    }

    // 10) papers_fts
    let _ = conn.execute("DELETE FROM papers_fts WHERE paper_id = ?1", params![pid]);
    let paper = &snap.paper;
    let authors = snap
        .creators
        .iter()
        .map(|c| c.creator.display_name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let kw = snap
        .keywords
        .iter()
        .map(|k| k.name.clone())
        .collect::<Vec<_>>()
        .join(", ");
    let _ = conn.execute(
        "INSERT INTO papers_fts (paper_id, title, abstract, authors, keywords, venue, doi)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![pid, paper.title, paper.abstract_text, authors, kw, paper.venue, paper.doi],
    );

    Ok(())
}

// ============================================================
// merge_papers
// ============================================================

/// 合并 src → dst（dst 保留）。返回 `MergeResult`。
pub fn merge_papers(
    vault: &Path,
    src_id: &str,
    dst_id: &str,
) -> AppResult<MergeResult> {
    if src_id == dst_id {
        return Err(AppError::Other("不能把同一篇论文合并到自身".into()));
    }
    let conn = db::open(vault)?;

    // 0) 确认两 paper 都存在
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM papers WHERE id IN (?1, ?2)",
        params![src_id, dst_id],
        |row| row.get(0),
    )?;
    if exists != 2 {
        return Err(AppError::NotFound("合并目标 paper 不存在".into()));
    }

    // 1) snapshot
    let snap_src = take_paper_snapshot(&conn, src_id)?;
    let snap_dst = take_paper_snapshot(&conn, dst_id)?;

    // 2) 整段合并包在事务里
    let tx = conn.unchecked_transaction()?;

    // 2a) 字段级合并：title / year / venue / abstract
    //     策略：dst 已有保留 dst；空才用 src 补。
    let mut fields_merged: Vec<String> = Vec::new();
    if snap_dst.paper.title.trim().is_empty() && !snap_src.paper.title.trim().is_empty() {
        tx.execute(
            "UPDATE papers SET title = ?1, updated_at = ?2 WHERE id = ?3",
            params![snap_src.paper.title, chrono::Local::now().timestamp_millis(), dst_id],
        )?;
        fields_merged.push("title".to_string());
    }
    if snap_dst.paper.year.is_none() && snap_src.paper.year.is_some() {
        tx.execute(
            "UPDATE papers SET year = ?1, updated_at = ?2 WHERE id = ?3",
            params![snap_src.paper.year, chrono::Local::now().timestamp_millis(), dst_id],
        )?;
        fields_merged.push("year".to_string());
    }
    if snap_dst.paper.venue.trim().is_empty() && !snap_src.paper.venue.trim().is_empty() {
        tx.execute(
            "UPDATE papers SET venue = ?1, updated_at = ?2 WHERE id = ?3",
            params![snap_src.paper.venue, chrono::Local::now().timestamp_millis(), dst_id],
        )?;
        fields_merged.push("venue".to_string());
    }
    if snap_dst.paper.abstract_text.trim().is_empty()
        && !snap_src.paper.abstract_text.trim().is_empty()
    {
        tx.execute(
            "UPDATE papers SET abstract_text = ?1, updated_at = ?2 WHERE id = ?3",
            params![snap_src.paper.abstract_text, chrono::Local::now().timestamp_millis(), dst_id],
        )?;
        fields_merged.push("abstract".to_string());
    }
    // 兜底：title 非空时，若两 paper 都有非空 title 但不相等 → 记录为冲突，
    // 不自动覆盖（dst 优先）。仅记录到 fields_merged 表示有观察过。
    if !snap_dst.paper.title.trim().is_empty()
        && !snap_src.paper.title.trim().is_empty()
        && snap_dst.paper.title != snap_src.paper.title
    {
        // 静默：dst 保留，不改。
    }

    // 2b) keywords auto: 替换为 src 全部 auto
    let src_auto: Vec<&KeywordSnapshot> = snap_src
        .keywords
        .iter()
        .filter(|k| k.source == "auto")
        .collect();
    if !src_auto.is_empty() {
        // 删 dst 中所有 auto 关键词
        tx.execute(
            "DELETE FROM paper_keywords
             WHERE paper_id = ?1
               AND keyword_id IN (SELECT id FROM keywords WHERE source = 'auto')",
            params![dst_id],
        )?;
        for k in &src_auto {
            tx.execute(
                "INSERT OR IGNORE INTO keywords (id, name, source) VALUES (?1, ?2, 'auto')",
                params![k.id, k.name],
            )?;
            tx.execute(
                "INSERT OR IGNORE INTO paper_keywords (paper_id, keyword_id) VALUES (?1, ?2)",
                params![dst_id, k.id],
            )?;
        }
        fields_merged.push("keywords.auto".to_string());
    }

    // 2c) keywords manual: 并集
    for k in &snap_src.keywords {
        if k.source != "manual" {
            continue;
        }
        tx.execute(
            "INSERT OR IGNORE INTO keywords (id, name, source) VALUES (?1, ?2, 'manual')",
            params![k.id, k.name],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO paper_keywords (paper_id, keyword_id) VALUES (?1, ?2)",
            params![dst_id, k.id],
        )?;
    }
    if snap_src.keywords.iter().any(|k| k.source == "manual") {
        fields_merged.push("keywords.manual".to_string());
    }

    // 2d) paper_creators: 按 display_name 合并，dst position 优先
    let dst_names: std::collections::HashSet<String> = snap_dst
        .creators
        .iter()
        .map(|c| c.creator.display_name.to_ascii_lowercase())
        .collect();
    // paper_creators：position 取 dst 已有最大值 + 1，保持在 dst 之后
    // 在 src 循环外计算一次，避免 O(N×M)
    let max_pos: i32 = snap_dst
        .creators
        .iter()
        .map(|c| c.position)
        .max()
        .unwrap_or(-1);
    for c in &snap_src.creators {
        if dst_names.contains(&c.creator.display_name.to_ascii_lowercase()) {
            continue;
        }
        // 复制 creator 行（如果不存在）
        tx.execute(
            "INSERT OR IGNORE INTO creators
                (id, family_name, given_name, display_name, raw)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                c.creator.id,
                c.creator.family_name,
                c.creator.given_name,
                c.creator.display_name,
                c.creator.raw
            ],
        )?;
        let new_pos = max_pos + 1 + rand_deterministic_offset(&c.creator.id) as i32 % 1024;
        tx.execute(
            "INSERT OR IGNORE INTO paper_creators
                (paper_id, creator_id, position, role)
             VALUES (?1, ?2, ?3, ?4)",
            params![dst_id, c.creator.id, new_pos, c.role],
        )?;
    }
    if !snap_src.creators.is_empty() {
        fields_merged.push("creators".to_string());
    }

    // 2e) paper_collections: 并集
    for cid in &snap_src.collection_ids {
        tx.execute(
            "INSERT OR IGNORE INTO paper_collections (paper_id, collection_id) VALUES (?1, ?2)",
            params![dst_id, cid],
        )?;
    }
    if !snap_src.collection_ids.is_empty() {
        fields_merged.push("collections".to_string());
    }

    // 2f) attachments: 并集（按 sha256 或 rel_path 去重）
    let dst_attach_keys: std::collections::HashSet<String> = snap_dst
        .attachments
        .iter()
        .map(|a| {
            if let Some(sha) = &a.sha256 {
                if !sha.is_empty() {
                    return format!("sha:{sha}");
                }
            }
            format!("path:{}:{}", a.kind, a.rel_path)
        })
        .collect();
    for a in &snap_src.attachments {
        let key = if let Some(sha) = &a.sha256 {
            if !sha.is_empty() {
                format!("sha:{sha}")
            } else {
                format!("path:{}:{}", a.kind, a.rel_path)
            }
        } else {
            format!("path:{}:{}", a.kind, a.rel_path)
        };
        if dst_attach_keys.contains(&key) {
            continue;
        }
        // 改写 attachment 的 paper_id 到 dst，保留 id。
        let mut a2 = a.clone();
        a2.paper_id = dst_id.to_string();
        tx.execute(
            "INSERT OR REPLACE INTO attachments
                (id, paper_id, kind, rel_path, abs_path, mime_type, title, frontmatter,
                 sha256, imported_at, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                a2.id, a2.paper_id, a2.kind, a2.rel_path, a2.abs_path, a2.mime_type,
                a2.title, a2.frontmatter, a2.sha256, a2.imported_at, a2.status,
            ],
        )?;
    }
    if !snap_src.attachments.is_empty() {
        fields_merged.push("attachments".to_string());
    }

    // 2g) annotations: 并集（按 (attachment_id, page, text) 去重）
    let dst_ann_keys: std::collections::HashSet<String> = snap_dst
        .annotations
        .iter()
        .map(|a| {
            format!(
                "{}|{}|{}",
                a.attachment_id.clone().unwrap_or_default(),
                a.page.unwrap_or(-1),
                a.text.clone().unwrap_or_default()
            )
        })
        .collect();
    for ann in &snap_src.annotations {
        let key = format!(
            "{}|{}|{}",
            ann.attachment_id.clone().unwrap_or_default(),
            ann.page.unwrap_or(-1),
            ann.text.clone().unwrap_or_default()
        );
        if dst_ann_keys.contains(&key) {
            continue;
        }
        let mut a2 = ann.clone();
        a2.paper_id = dst_id.to_string();
        tx.execute(
            "INSERT OR REPLACE INTO annotations
                (id, paper_id, attachment_id, kind, page, rect, color, text, comment,
                 created_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                a2.id, a2.paper_id, a2.attachment_id, a2.kind, a2.page, a2.rect,
                a2.color, a2.text, a2.comment, a2.created_at, a2.modified_at,
            ],
        )?;
    }
    if !snap_src.annotations.is_empty() {
        fields_merged.push("annotations".to_string());
    }

    // 2h) identifiers: 并集
    for id in &snap_src.identifiers {
        tx.execute(
            "INSERT OR IGNORE INTO identifiers (paper_id, [type], value, is_primary)
             VALUES (?1, ?2, ?3, ?4)",
            params![dst_id, id.kind, id.value, id.is_primary],
        )?;
    }
    if !snap_src.identifiers.is_empty() {
        fields_merged.push("identifiers".to_string());
    }

    // 2i) paper_relations: src→其它 / 其它→src / src→dst 全部转为 dst
    for r in &snap_src.relations {
        let (new_src, new_dst) = if r.src_paper_id == src_id && r.dst_paper_id == src_id {
            // 自指：罕见；保留为 src→dst = src→dst 替换为 dst→dst，意义不大。
            // 实际不会发生（一个 paper 不能既 src 又 dst 于同一条 relation），
            // 但保险起见跳过。
            continue;
        } else if r.src_paper_id == src_id {
            (dst_id.to_string(), r.dst_paper_id.clone())
        } else if r.dst_paper_id == src_id {
            (r.src_paper_id.clone(), dst_id.to_string())
        } else {
            continue; // 与 src 无关
        };
        // 自环避免：new_src == new_dst
        if new_src == new_dst {
            continue;
        }
        tx.execute(
            "INSERT OR IGNORE INTO paper_relations
                (src_paper_id, dst_paper_id, relation, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![new_src, new_dst, r.relation, r.note, r.created_at],
        )?;
    }
    if !snap_src.relations.is_empty() {
        fields_merged.push("relations".to_string());
    }

    // 2j) 写 merge_log（在删 src 之前）
    let snapshot_json = serde_json::to_string(&MergeSnapshot {
        src: snap_src,
        dst: snap_dst,
    })?;
    let now_ms = chrono::Local::now().timestamp_millis();
    tx.execute(
        "INSERT INTO merge_log (canonical_id, duplicate_id, fields_merged, snapshot, merged_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![dst_id, src_id, serde_json::to_string(&fields_merged)?, snapshot_json, now_ms],
    )?;
    let merge_id = tx.last_insert_rowid();

    // 2k) 删 src paper（FK CASCADE 自动清理残余子表行）
    tx.execute("DELETE FROM papers WHERE id = ?1", params![src_id])?;
    tx.execute("DELETE FROM papers_fts WHERE paper_id = ?1", params![src_id])?;

    // 2l) 清理 5 分钟前的 merge_log
    let cutoff = now_ms - UNDO_WINDOW_MS;
    tx.execute("DELETE FROM merge_log WHERE merged_at < ?1", params![cutoff])?;

    tx.commit()?;

    // 事务外再触发 reindex（reindex_paper 自管连接）。
    let _ = crate::services::index::reindex_paper(vault, dst_id);

    Ok(MergeResult {
        merge_id,
        canonical_id: dst_id.to_string(),
        duplicate_id: src_id.to_string(),
        fields_merged,
        merged_at: now_ms,
    })
}

fn rand_deterministic_offset(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in s.as_bytes() {
        h ^= u32::from(*b);
        h = h.wrapping_mul(16777619);
    }
    h
}

// ============================================================
// undo_merge
// ============================================================

/// 5 分钟内可撤销。
pub fn undo_merge(vault: &Path, merge_id: i64) -> AppResult<()> {
    let conn = db::open(vault)?;
    let entry: (String, String, String, i64) = conn
        .query_row(
            "SELECT canonical_id, duplicate_id, snapshot, merged_at
             FROM merge_log WHERE id = ?1",
            params![merge_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                AppError::NotFound(format!("merge_log #{merge_id} 不存在"))
            }
            other => AppError::from(other),
        })?;
    let (canonical_id, duplicate_id, snapshot_json, merged_at) = entry;

    let now_ms = chrono::Local::now().timestamp_millis();
    if now_ms - merged_at > UNDO_WINDOW_MS {
        return Err(AppError::Other(format!(
            "已超过 {} 分钟，无法撤销",
            UNDO_WINDOW_MS / 60_000
        )));
    }

    let snap: MergeSnapshot = serde_json::from_str(&snapshot_json)
        .map_err(|e| AppError::Other(format!("snapshot JSON 解析失败: {e}")))?;

    let tx = conn.unchecked_transaction()?;

    // 关键：先删 merge_log 行；否则后续 INSERT OR REPLACE INTO papers
    // 触发 ON DELETE SET NULL → canonical_id 被设 NULL → 违反 NOT NULL。
    tx.execute("DELETE FROM merge_log WHERE id = ?1", params![merge_id])?;

    // 关键恢复顺序：
    //   1) 先恢复 dst 论文：snapshot 里的 dst.paper.id 应是 canonical_id
    //      （与原 canonical_id 一致），用 snapshot 重写其全部相关表。
    //   2) 再恢复 src 论文：snapshot.src.paper.id 是 duplicate_id，可能
    //      在合并时已被 DELETE，所以这里用 INSERT OR REPLACE 复活。
    if snap.dst.paper.id != canonical_id {
        return Err(AppError::Other("snapshot.canonical 不匹配".into()));
    }
    if snap.src.paper.id != duplicate_id {
        return Err(AppError::Other("snapshot.duplicate 不匹配".into()));
    }
    restore_paper_snapshot(&tx, &snap.dst)?;
    restore_paper_snapshot(&tx, &snap.src)?;
    tx.commit()?;
    Ok(())
}

// ============================================================
// cleanup
// ============================================================

/// 启动时或每次 merge 后清理 `merged_at < now - UNDO_WINDOW_MS` 的行。
pub fn cleanup_old_merge_log(vault: &Path) -> AppResult<usize> {
    let conn = db::open(vault)?;
    let cutoff = chrono::Local::now().timestamp_millis() - UNDO_WINDOW_MS;
    let n = conn.execute("DELETE FROM merge_log WHERE merged_at < ?1", params![cutoff])?;
    Ok(n)
}

// ============================================================
// 内部：reindex_paper_in_tx 桥接
// ============================================================

// 单元测试用：我们提供 paper.rs 内的 reindex_paper 接口；
// 真实调用在 merge_papers 内部用 tx 版的 reindex_paper_in_tx。
// 为避免循环依赖，这里把 index 模块的 reindex_paper 函数暴露为
// 可在事务上执行的形式（如果 index 模块没提供 tx 版本，
// 则本调用方跳过 FTS 重建；reindex_all 启动时会重建）。
// 注：本文件通过 services::index 模块路径调用 reindex_paper_in_tx，
// 该函数在 index.rs 中实现。

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    fn fresh_vault() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        // 跑 migrate（应用 0001/0002 + P0 数据搬运）
        let _ = db::migrate(&path).unwrap();
        dir
    }

    fn insert_paper(conn: &Connection, id: &str, title: &str, doi: &str, year: Option<i32>) {
        conn.execute(
            "INSERT INTO papers (id, title, year, venue, doi, abstract_text, status,
                                 pdf_path, note_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'unread', '', '', 0, 0)",
            rusqlite::params![id, title, year, "", doi, ""],
        )
        .unwrap();
        if !doi.is_empty() {
            conn.execute(
                "INSERT OR IGNORE INTO identifiers (paper_id, [type], value, is_primary)
                 VALUES (?1, 'doi', ?2, 1)",
                rusqlite::params![id, doi],
            )
            .unwrap();
        }
    }

    #[test]
    fn merge_basic_metadata_field_strategy() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();

        // src：title、venue 完整；dst：title 空、year 空
        insert_paper(&conn, "src1", "Source Paper", "10.1/src", Some(2020));
        conn.execute(
            "UPDATE papers SET venue='Source Venue', abstract_text='src abs' WHERE id='src1'",
            [],
        )
        .unwrap();
        insert_paper(&conn, "dst1", "", "10.1/dst", None);

        let r = merge_papers(dir.path(), "src1", "dst1").unwrap();
        assert!(r.fields_merged.contains(&"title".to_string()));
        assert!(r.fields_merged.contains(&"year".to_string()));
        assert!(r.fields_merged.contains(&"venue".to_string()));
        assert!(r.fields_merged.contains(&"abstract".to_string()));

        // 验证 dst 已经被填充
        let dst_title: String = conn
            .query_row("SELECT title FROM papers WHERE id='dst1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(dst_title, "Source Paper");

        // src 已删
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM papers WHERE id='src1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(n, 0);

        // merge_log 写入
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM merge_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn merge_dst_already_has_value_keeps_dst() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "src1", "Source Title", "10.1/src", Some(2020));
        insert_paper(&conn, "dst1", "Dst Title", "10.1/dst", Some(2021));

        merge_papers(dir.path(), "src1", "dst1").unwrap();
        // dst 已有 title / year，保留 dst
        let (t, y): (String, Option<i32>) = conn
            .query_row(
                "SELECT title, year FROM papers WHERE id='dst1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(t, "Dst Title");
        assert_eq!(y, Some(2021));
    }

    #[test]
    fn merge_identifiers_union() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "src1", "s", "10.1/s", None);
        insert_paper(&conn, "dst1", "d", "10.1/d", None);
        // src 多一个 arxiv identifier
        conn.execute(
            "INSERT OR IGNORE INTO identifiers (paper_id, [type], value, is_primary)
             VALUES ('src1', 'arxiv', '2401.01234', 0)",
            [],
        )
        .unwrap();

        merge_papers(dir.path(), "src1", "dst1").unwrap();

        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM identifiers WHERE paper_id='dst1'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        // dst1 原本 1 个 DOI，加上 src1 的 1 个 DOI（重复但不报错）+ 1 个 arxiv
        // 实际上 src1 也有自己的 DOI 10.1/s，因此 dst1 拿到 2 个 identifiers。
        assert!(n >= 2, "expected ≥2, got {n}");
    }

    #[test]
    fn merge_keywords_auto_replaces() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        // 准备 keywords（auto 与 manual 各 1）
        conn.execute(
            "INSERT INTO keywords (id, name, source) VALUES ('k1', 'ml', 'auto')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO keywords (id, name, source) VALUES ('k2', 'cv', 'manual')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO keywords (id, name, source) VALUES ('k3', 'nlp', 'auto')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO keywords (id, name, source) VALUES ('k4', 'rl', 'manual')",
            [],
        )
        .unwrap();

        // dst 关联 k1 (auto) + k2 (manual)
        insert_paper(&conn, "dst1", "d", "10.1/d", None);
        conn.execute(
            "INSERT INTO paper_keywords (paper_id, keyword_id) VALUES ('dst1', 'k1')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO paper_keywords (paper_id, keyword_id) VALUES ('dst1', 'k2')",
            [],
        )
        .unwrap();

        // src 关联 k3 (auto) + k4 (manual)
        insert_paper(&conn, "src1", "s", "10.1/s", None);
        conn.execute(
            "INSERT INTO paper_keywords (paper_id, keyword_id) VALUES ('src1', 'k3')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO paper_keywords (paper_id, keyword_id) VALUES ('src1', 'k4')",
            [],
        )
        .unwrap();

        merge_papers(dir.path(), "src1", "dst1").unwrap();

        // dst 的 auto 应该只剩 k3（被替换）；manual 应该是 k2 ∪ k4 = {k2, k4}
        let names: Vec<String> = {
            let mut stmt = conn
                .prepare(
                    "SELECT k.name FROM paper_keywords pk
                     JOIN keywords k ON k.id = pk.keyword_id
                     WHERE pk.paper_id = 'dst1'
                     ORDER BY k.name",
                )
                .unwrap();
            stmt.query_map([], |row| row.get(0))
                .unwrap()
                .map(|r| r.unwrap())
                .collect()
        };
        // auto 应为 {nlp}（k3），manual 应为 {cv, rl}（k2, k4）
        assert!(names.contains(&"nlp".to_string()));
        assert!(names.contains(&"cv".to_string()));
        assert!(names.contains(&"rl".to_string()));
        // ml（k1）应被替换掉
        assert!(!names.contains(&"ml".to_string()), "ml should be replaced: {names:?}");
    }

    #[test]
    fn merge_creators_union_by_display_name() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "dst1", "d", "10.1/d", None);
        insert_paper(&conn, "src1", "s", "10.1/s", None);

        // dst: Alice, Bob
        conn.execute(
            "INSERT INTO creators (id, family_name, given_name, display_name, raw)
             VALUES ('c1', 'Smith', 'Alice', 'Alice Smith', 'Alice Smith')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO creators (id, family_name, given_name, display_name, raw)
             VALUES ('c2', 'Jones', 'Bob', 'Bob Jones', 'Bob Jones')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO paper_creators VALUES ('dst1', 'c1', 0, 'author')", []).unwrap();
        conn.execute("INSERT INTO paper_creators VALUES ('dst1', 'c2', 1, 'author')", []).unwrap();

        // src: Bob (重名，跳过), Charlie
        conn.execute(
            "INSERT INTO creators (id, family_name, given_name, display_name, raw)
             VALUES ('c3', 'Liu', 'Charlie', 'Charlie Liu', 'Charlie Liu')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO paper_creators VALUES ('src1', 'c2', 0, 'author')", []).unwrap();
        conn.execute("INSERT INTO paper_creators VALUES ('src1', 'c3', 1, 'author')", []).unwrap();

        merge_papers(dir.path(), "src1", "dst1").unwrap();

        let names: Vec<String> = conn
            .prepare(
                "SELECT c.display_name FROM paper_creators pc
                 JOIN creators c ON c.id = pc.creator_id
                 WHERE pc.paper_id = 'dst1'
                 ORDER BY pc.position",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(|r| r.unwrap())
            .collect();
        // 应有 3 个：Alice, Bob, Charlie（Bob 出现一次）
        assert_eq!(names.len(), 3, "names: {names:?}");
        assert!(names.contains(&"Charlie Liu".to_string()));
    }

    #[test]
    fn undo_within_5_min_restores_both() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "src1", "Source Title", "10.1/s", Some(2020));
        insert_paper(&conn, "dst1", "Dst Title", "10.1/d", Some(2021));
        // src 加一个 attachment
        conn.execute(
            "INSERT INTO attachments (id, paper_id, kind, rel_path, abs_path, mime_type,
                                       title, frontmatter, sha256, imported_at, status)
             VALUES ('a1', 'src1', 'pdf', 'papers/src1/x.pdf', NULL, 'application/pdf',
                     NULL, NULL, 'abc', 0, 'active')",
            [],
        )
        .unwrap();
        // src 关联一个 auto keyword
        conn.execute("INSERT INTO keywords (id, name, source) VALUES ('k1', 'ml', 'auto')", []).unwrap();
        conn.execute(
            "INSERT INTO paper_keywords (paper_id, keyword_id) VALUES ('src1', 'k1')",
            [],
        )
        .unwrap();

        let r = merge_papers(dir.path(), "src1", "dst1").unwrap();

        // 合并后 src 应已删
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM papers WHERE id='src1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(n, 0);

        // 撤销
        undo_merge(dir.path(), r.merge_id).unwrap();

        // src 应已恢复
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM papers WHERE id='src1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(n, 1);

        // src 的 attachment 也应恢复
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM attachments WHERE paper_id='src1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(n, 1, "src attachment should be restored");

        // src 的 auto keyword 关联应恢复
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM paper_keywords pk
                 JOIN keywords k ON k.id = pk.keyword_id
                 WHERE pk.paper_id='src1' AND k.source='auto'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "src auto keyword should be restored");

        // dst 应回到合并前状态
        let t: String = conn
            .query_row("SELECT title FROM papers WHERE id='dst1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(t, "Dst Title");

        // merge_log 行应已删
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM merge_log WHERE id=?1", params![r.merge_id], |row| row.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn undo_after_5_min_fails() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "src1", "Source", "10.1/s", Some(2020));
        insert_paper(&conn, "dst1", "Dst", "10.1/d", Some(2021));

        let r = merge_papers(dir.path(), "src1", "dst1").unwrap();

        // 手动把 merged_at 调成 6 分钟前
        let old = chrono::Local::now().timestamp_millis() - UNDO_WINDOW_MS - 1000;
        conn.execute(
            "UPDATE merge_log SET merged_at = ?1 WHERE id = ?2",
            params![old, r.merge_id],
        )
        .unwrap();

        let err = undo_merge(dir.path(), r.merge_id).unwrap_err();
        assert!(matches!(err, AppError::Other(_)), "got {err:?}");
    }

    #[test]
    fn merge_attachments_dedup_by_sha() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "dst1", "d", "10.1/d", None);
        insert_paper(&conn, "src1", "s", "10.1/s", None);
        // 两边各有一个 sha 相同的 attachment
        conn.execute(
            "INSERT INTO attachments (id, paper_id, kind, rel_path, sha256, imported_at, status)
             VALUES ('a1', 'dst1', 'pdf', 'd.pdf', 'shared-sha', 0, 'active')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO attachments (id, paper_id, kind, rel_path, sha256, imported_at, status)
             VALUES ('a2', 'src1', 'pdf', 's.pdf', 'shared-sha', 0, 'active')",
            [],
        )
        .unwrap();
        // src 再加一个独有的 attachment
        conn.execute(
            "INSERT INTO attachments (id, paper_id, kind, rel_path, sha256, imported_at, status)
             VALUES ('a3', 'src1', 'supplement', 's2.pdf', 'unique', 0, 'active')",
            [],
        )
        .unwrap();

        merge_papers(dir.path(), "src1", "dst1").unwrap();

        // dst 现在应该 2 个 attachment：a1（保留）+ a3（新增）
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM attachments WHERE paper_id='dst1'", [], |row| row.get(0))
            .unwrap();
        assert_eq!(n, 2, "expected 2 attachments (a1 + a3), got {n}");
    }

    #[test]
    fn merge_self_fails() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "p1", "t", "10.1/p", None);
        let err = merge_papers(dir.path(), "p1", "p1").unwrap_err();
        assert!(matches!(err, AppError::Other(_)));
    }

    #[test]
    fn merge_missing_paper_fails() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "p1", "t", "10.1/p", None);
        let err = merge_papers(dir.path(), "p1", "missing").unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[test]
    fn merge_log_cleanup_removes_old_entries() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        // 先建两个 paper（merge_log.canonical_id 有 FK）
        insert_paper(&conn, "a", "t-a", "10.1/a", None);
        insert_paper(&conn, "b", "t-b", "10.1/b", None);
        // 手工造一条 6 分钟前的 merge_log 行
        let old = chrono::Local::now().timestamp_millis() - UNDO_WINDOW_MS - 1000;
        conn.execute(
            "INSERT INTO merge_log (canonical_id, duplicate_id, fields_merged, snapshot, merged_at)
             VALUES ('a', 'b', '[]', '{}', ?1)",
            params![old],
        )
        .unwrap();
        let n = cleanup_old_merge_log(dir.path()).unwrap();
        assert_eq!(n, 1);
        let n: i64 = conn
            .query_row("SELECT COUNT(*) FROM merge_log", [], |row| row.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }
}
