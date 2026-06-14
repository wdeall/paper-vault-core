//! 论文相关命令

use crate::error::AppResult;
use crate::pdf;
use crate::services::{duplicate, paper};
use crate::types::{DuplicateCandidate, ImportResult, MetadataCandidate, Paper, PaperDetail, ReadingProgress};
use crate::vault;
use std::path::Path;
use tauri::State;

use crate::AppState;

fn require_vault<'a>(state: &'a State<'_, AppState>) -> AppResult<std::path::PathBuf> {
    let guard = state.vault_path.read();
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| crate::error::AppError::Config("vault 未初始化".into()))
}

#[tauri::command]
pub async fn import_pdf(
    state: State<'_, AppState>,
    source_path: String,
) -> AppResult<ImportResult> {
    import_pdf_impl(&state, source_path).await
}

#[tauri::command]
pub async fn import_pdfs_batch(
    state: State<'_, AppState>,
    source_paths: Vec<String>,
) -> AppResult<Vec<ImportResult>> {
    let mut out = Vec::new();
    for sp in source_paths {
        match import_pdf_impl(&state, sp).await {
            Ok(r) => out.push(r),
            Err(e) => log::warn!("导入失败: {e}"),
        }
    }
    Ok(out)
}

async fn import_pdf_impl(
    state: &State<'_, AppState>,
    source_path: String,
) -> AppResult<ImportResult> {
    let vault = require_vault(state)?;
    let src = Path::new(&source_path);

    let file_stem = src
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("untitled")
        .to_string();

    let basic = pdf::extract_basic(src).unwrap_or_default();

    let paper_id = uuid::Uuid::new_v4().simple().to_string();
    let title = if !basic.title.is_empty() {
        basic.title.clone()
    } else {
        file_stem.clone()
    };

    let duplicates: Vec<DuplicateCandidate> = if !basic.doi.is_empty() {
        duplicate::detect(
            &vault,
            Some(&basic.doi),
            Some(&title),
            None,
            None,
        )?
    } else {
        duplicate::detect(&vault, None, Some(&title), None, None)?
    };

    let pdf_path = vault::copy_pdf(&vault, src, &paper_id, &title)?;

    let now = chrono::Local::now().timestamp_millis();
    let p = Paper {
        id: paper_id.clone(),
        title: title.clone(),
        authors: Vec::new(),
        year: None,
        venue: String::new(),
        doi: crate::duplicates::normalize_doi(&basic.doi),
        abstract_text: String::new(),
        keywords: Vec::new(),
        tags: vec!["待补全".into()],
        status: "未读".into(),
        rating: None,
        pdf_path: pdf_path.to_string_lossy().to_string(),
        note_path: String::new(),
        created_at: now,
        updated_at: now,
    };
    paper::insert(&vault, &p)?;
    if let Err(e) = crate::services::index::reindex_paper(&vault, &paper_id) {
        log::warn!("导入后索引失败 {paper_id}: {e}");
    }

    Ok(ImportResult { paper: p, duplicates })
}

#[tauri::command]
pub async fn list_papers(
    state: State<'_, AppState>,
    status: Option<String>,
    collection_id: Option<String>,
    tag: Option<String>,
) -> AppResult<Vec<Paper>> {
    let vault = require_vault(&state)?;
    paper::list(
        &vault,
        status.as_deref(),
        collection_id.as_deref(),
        tag.as_deref(),
    )
}

#[tauri::command]
pub async fn get_paper(state: State<'_, AppState>, id: String) -> AppResult<PaperDetail> {
    let vault = require_vault(&state)?;
    paper::get(&vault, &id)
}

#[tauri::command]
pub async fn update_paper(
    state: State<'_, AppState>,
    id: String,
    patch: Paper,
) -> AppResult<Paper> {
    let vault = require_vault(&state)?;
    paper::update(&vault, &id, &patch)
}

#[tauri::command]
pub async fn delete_paper(
    state: State<'_, AppState>,
    id: String,
    mode: String,
) -> AppResult<()> {
    let vault = require_vault(&state)?;
    paper::delete(&vault, &id, &mode)
}

#[tauri::command]
pub async fn update_progress(
    state: State<'_, AppState>,
    id: String,
    current_page: i32,
    total_pages: Option<i32>,
) -> AppResult<ReadingProgress> {
    let vault = require_vault(&state)?;
    paper::update_progress(&vault, &id, current_page, total_pages)
}

#[tauri::command]
pub async fn check_duplicates(
    state: State<'_, AppState>,
    doi: Option<String>,
    title: Option<String>,
    authors: Option<Vec<String>>,
    year: Option<i32>,
) -> AppResult<Vec<DuplicateCandidate>> {
    let vault = require_vault(&state)?;
    duplicate::detect(
        &vault,
        doi.as_deref(),
        title.as_deref(),
        authors.as_deref(),
        year,
    )
}

#[tauri::command]
pub async fn extract_metadata(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<MetadataCandidate> {
    let vault = require_vault(&state)?;
    let detail = paper::get(&vault, &id)?;
    let pp = Path::new(&detail.paper.pdf_path);
    let basic = if pp.exists() {
        pdf::extract_basic(pp).unwrap_or_default()
    } else {
        Default::default()
    };
    Ok(MetadataCandidate {
        title: if !basic.title.is_empty() { basic.title } else { detail.paper.title },
        authors: detail.paper.authors,
        year: detail.paper.year,
        venue: detail.paper.venue,
        doi: if !basic.doi.is_empty() { basic.doi } else { detail.paper.doi },
        abstract_text: detail.paper.abstract_text,
        keywords: detail.paper.keywords,
        source: "pdf-text".into(),
        confidence: "medium".into(),
    })
}

#[tauri::command]
pub async fn read_pdf_bytes(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Vec<u8>> {
    let vault = require_vault(&state)?;
    let detail = paper::get(&vault, &id)?;
    let path = Path::new(&detail.paper.pdf_path);
    if !path.exists() {
        return Err(crate::error::AppError::NotFound(format!(
            "PDF 文件不存在: {}",
            detail.paper.pdf_path
        )));
    }
    Ok(std::fs::read(path)?)
}

#[tauri::command]
pub async fn open_pdf(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let vault = require_vault(&state)?;
    let detail = paper::get(&vault, &id)?;
    let path = Path::new(&detail.paper.pdf_path);
    if !path.exists() {
        return Err(crate::error::AppError::NotFound(format!(
            "PDF 文件不存在: {}",
            detail.paper.pdf_path
        )));
    }
    open::that_detached(path).map_err(|e| crate::error::AppError::Other(e.to_string()))
}

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> AppResult<Vec<crate::types::Collection>> {
    let vault = require_vault(&state)?;
    let conn = crate::db::open(&vault)?;
    let mut stmt = conn.prepare("SELECT id, name, parent_id, created_at FROM collections ORDER BY name")?;
    let rows = stmt.query_map([], |r| {
        Ok(crate::types::Collection {
            id: r.get(0)?,
            name: r.get(1)?,
            parent_id: r.get(2)?,
            created_at: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

#[tauri::command]
pub async fn create_collection(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> AppResult<crate::types::Collection> {
    let vault = require_vault(&state)?;
    let conn = crate::db::open(&vault)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let now = chrono::Local::now().timestamp_millis();
    conn.execute(
        "INSERT INTO collections (id, name, parent_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, name, parent_id, now],
    )?;
    Ok(crate::types::Collection { id, name, parent_id, created_at: now })
}

#[tauri::command]
pub async fn add_paper_to_collection(
    state: State<'_, AppState>,
    paper_id: String,
    collection_id: String,
) -> AppResult<()> {
    let vault = require_vault(&state)?;
    let conn = crate::db::open(&vault)?;
    conn.execute(
        "INSERT OR IGNORE INTO paper_collections (paper_id, collection_id) VALUES (?1, ?2)",
        rusqlite::params![paper_id, collection_id],
    )?;
    Ok(())
}

#[tauri::command]
pub async fn remove_paper_from_collection(
    state: State<'_, AppState>,
    paper_id: String,
    collection_id: String,
) -> AppResult<()> {
    let vault = require_vault(&state)?;
    let conn = crate::db::open(&vault)?;
    conn.execute(
        "DELETE FROM paper_collections WHERE paper_id = ?1 AND collection_id = ?2",
        rusqlite::params![paper_id, collection_id],
    )?;
    Ok(())
}

#[tauri::command]
pub async fn list_keywords(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let vault = require_vault(&state)?;
    let conn = crate::db::open(&vault)?;
    let mut stmt = conn.prepare("SELECT keywords FROM papers")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut set = std::collections::BTreeSet::new();
    for r in rows {
        let s = r?;
        let v: Vec<String> = serde_json::from_str(&s).unwrap_or_default();
        for k in v { set.insert(k); }
    }
    Ok(set.into_iter().collect())
}

#[tauri::command]
pub async fn list_tags(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let vault = require_vault(&state)?;
    let conn = crate::db::open(&vault)?;
    let mut stmt = conn.prepare("SELECT tags FROM papers")?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut set = std::collections::BTreeSet::new();
    for r in rows {
        let s = r?;
        let v: Vec<String> = serde_json::from_str(&s).unwrap_or_default();
        for t in v { set.insert(t); }
    }
    Ok(set.into_iter().collect())
}
