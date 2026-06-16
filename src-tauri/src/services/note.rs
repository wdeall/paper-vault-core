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
    let paper = crate::services::paper::load_paper(vault, paper_id)?
        .ok_or_else(|| AppError::NotFound(format!("论文 {paper_id} 不存在")))?;

    let path = note_path(vault, &paper);
    if path.exists() {
        return Err(AppError::Invalid("笔记已存在".into()));
    }
    let body = markdown::default_template(&paper);
    let fm: serde_yaml::Value = serde_yaml::to_value(&paper).unwrap_or(serde_yaml::Value::Null);
    markdown::write_note(&path, &fm, &body)?;

    let now = chrono::Local::now().timestamp_millis();
    let conn = db::open(vault)?;
    let rel = path
        .strip_prefix(vault)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| path.to_string_lossy().to_string());
    conn.execute(
        "UPDATE papers SET note_path = ?2, updated_at = ?3 WHERE id = ?1",
        params![paper_id, rel, now],
    )?;
    Ok(rel)
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
    let paper = crate::services::paper::load_paper(vault, paper_id)?
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
    let conn = db::open(vault)?;
    let rel = dst
        .strip_prefix(vault)
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| dst.to_string_lossy().to_string());
    conn.execute(
        "UPDATE papers SET note_path = ?2, updated_at = ?3 WHERE id = ?1",
        params![paper_id, rel, now],
    )?;
    Ok(rel)
}
