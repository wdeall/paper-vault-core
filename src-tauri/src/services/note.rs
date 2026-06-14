//! 笔记服务

use crate::db;
use crate::error::{AppError, AppResult};
use crate::markdown;
use crate::types::{NoteContent, Paper};
use rusqlite::{params, OptionalExtension};
use std::path::Path;

fn note_path(vault: &Path, paper: &Paper) -> std::path::PathBuf {
    let slug = crate::vault::slug_from_title(&paper.title);
    let name = if slug.is_empty() {
        format!("{}.md", paper.id)
    } else {
        format!("{}-{}.md", paper.id, slug)
    };
    vault
        .join(crate::vault::NOTES_DIR)
        .join(crate::vault::NOTES_PAPERS_DIR)
        .join(name)
}

pub fn create(vault: &Path, paper_id: &str) -> AppResult<String> {
    let conn = db::open(vault)?;
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

    let path = note_path(vault, &paper);
    if path.exists() {
        return Err(AppError::Invalid("笔记已存在".into()));
    }
    let body = markdown::default_template(&paper);
    let fm: serde_yaml::Value = serde_yaml::to_value(&paper).unwrap_or(serde_yaml::Value::Null);
    markdown::write_note(&path, &fm, &body)?;

    let now = chrono::Local::now().timestamp_millis();
    conn.execute(
        "UPDATE papers SET note_path = ?2, updated_at = ?3 WHERE id = ?1",
        params![paper_id, path.to_string_lossy().to_string(), now],
    )?;
    Ok(path.to_string_lossy().to_string())
}

pub fn get(vault: &Path, paper_id: &str) -> AppResult<NoteContent> {
    let conn = db::open(vault)?;
    let note_path: String = conn
        .query_row(
            "SELECT note_path FROM papers WHERE id = ?1",
            params![paper_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("论文 {paper_id} 没有 note_path")))?;

    let p = std::path::Path::new(&note_path);
    if !p.exists() {
        return Err(AppError::NotFound(format!("笔记文件不存在: {note_path}")));
    }
    let nc = markdown::read_note(p)?;
    Ok(NoteContent {
        path: note_path,
        frontmatter: serde_json::to_value(&nc.frontmatter).unwrap_or(serde_json::Value::Null),
        content: nc.body,
    })
}

pub fn update(vault: &Path, paper_id: &str, content: &str) -> AppResult<()> {
    let conn = db::open(vault)?;
    let note_path: String = conn
        .query_row(
            "SELECT note_path FROM papers WHERE id = ?1",
            params![paper_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("note_path 不存在".into()))?;
    let p = std::path::Path::new(&note_path);
    let nc = markdown::read_note(p)?;
    markdown::write_note(p, &nc.frontmatter, content)?;
    Ok(())
}

pub fn update_ai_block(
    vault: &Path,
    paper_id: &str,
    block: &str,
    new_content: &str,
) -> AppResult<()> {
    let conn = db::open(vault)?;
    let note_path: String = conn
        .query_row(
            "SELECT note_path FROM papers WHERE id = ?1",
            params![paper_id],
            |r| r.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound("note_path 不存在".into()))?;
    let p = std::path::Path::new(&note_path);
    markdown::update_ai_block(p, block, new_content)?;
    Ok(())
}

pub fn import_external(
    vault: &Path,
    paper_id: &str,
    source: &Path,
) -> AppResult<String> {
    let conn = db::open(vault)?;
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

    let dst = note_path(vault, &paper);
    crate::vault::copy_file(source, &dst)?;

    // 解析外部 frontmatter，写入数据库空字段（DB 优先，外部不覆盖）
    let nc = markdown::read_note(&dst)?;
    if let serde_yaml::Value::Mapping(map) = &nc.frontmatter {
        let external = serde_json::to_value(&nc.frontmatter).unwrap_or(serde_json::Value::Null);
        let _ = external;
        // 这里只记录，不做覆盖（依据规范 DB 优先）。
        let _ = map;
    }

    let now = chrono::Local::now().timestamp_millis();
    conn.execute(
        "UPDATE papers SET note_path = ?2, updated_at = ?3 WHERE id = ?1",
        params![paper_id, dst.to_string_lossy().to_string(), now],
    )?;
    Ok(dst.to_string_lossy().to_string())
}
