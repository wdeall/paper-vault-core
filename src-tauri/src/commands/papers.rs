//! 论文相关命令。
//!
//! v2.0 P0：
//! - `status` 解析为 `PaperStatus` 枚举（仍兼容旧中文值）。
//! - `tag` 入参已废弃（tags 表不再保留）。
//! - `mode` 入参已废弃（删除始终硬删；P3 引入软删除 / 回收站）。
//! - 作者 / 关键词 / 状态 走 `services::paper`（结构化表）。

use crate::error::{AppError, AppResult};
use crate::pdf;
use crate::services::identifier::{self, Scheme};
use crate::services::resolver;
use crate::services::{duplicate, paper};
use crate::types::{
    Collection, DuplicateCandidate, ImportResult, MetadataCandidate, Paper, PaperDetail,
    PaperStatus, ReadingProgress,
};
use crate::vault;
use std::path::Path;
use tauri::State;

use crate::AppState;

fn require_vault<'a>(state: &'a State<'_, AppState>) -> AppResult<std::path::PathBuf> {
    let guard = state.vault_path.read();
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| AppError::Config("vault 未初始化".into()))
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

    // P1：先尝试从 PDF 文本抽出的 identifier 调 resolver；
    //     命中则复用 metadata 字段，命中不了走老流程。
    let resolver_meta = if !basic.doi.is_empty() {
        let parsed = identifier::parse(&basic.doi);
        if let Some((scheme, value)) = parsed.into_iter().next() {
            match resolver::default_resolver(scheme) {
                Ok(r) => match r.fetch(&value).await {
                    Ok(m) => Some((scheme, value, m)),
                    Err(e) => {
                        log::warn!("PDF 内 identifier 解析失败 ({scheme} {value}): {e}");
                        None
                    }
                },
                Err(e) => {
                    log::warn!("PDF 内 identifier scheme 不可用 ({scheme}): {e}");
                    None
                }
            }
        } else {
            None
        }
    } else {
        None
    };

    let paper_id = if let Some((scheme, value, _meta)) = &resolver_meta {
        // 用稳定前缀：同一篇 PDF 多次导入复用同一 id。
        let prefix = match scheme {
            Scheme::Doi => "meta-doi",
            Scheme::Arxiv => "meta-arxiv",
            Scheme::Pmid => "meta-pmid",
            Scheme::Isbn => "meta-isbn",
        };
        paper::stable_id(prefix, &format!("{prefix}|{value}"))
    } else {
        uuid::Uuid::new_v4().simple().to_string()
    };

    let (title, doi, year, venue, authors, abstract_text, keywords) = if let Some((_, _, m)) =
        resolver_meta.as_ref()
    {
        (
            if !m.title.is_empty() {
                m.title.clone()
            } else {
                file_stem.clone()
            },
            crate::duplicates::normalize_doi(&m.doi),
            m.year,
            m.venue.clone(),
            m.authors.clone(),
            m.abstract_text.clone(),
            m.keywords.clone(),
        )
    } else {
        (
            if !basic.title.is_empty() {
                basic.title.clone()
            } else {
                file_stem.clone()
            },
            crate::duplicates::normalize_doi(&basic.doi),
            None,
            String::new(),
            Vec::new(),
            String::new(),
            Vec::new(),
        )
    };

    let duplicates: Vec<DuplicateCandidate> = if !doi.is_empty() {
        duplicate::detect(&vault, Some(&doi), Some(&title), None, None)?
    } else {
        duplicate::detect(&vault, None, Some(&title), None, None)?
    };

    let pdf_path = vault::copy_pdf(&vault, src, &paper_id, &title)?;

    let now = chrono::Local::now().timestamp_millis();
    let p = Paper {
        id: paper_id.clone(),
        title,
        authors,
        year,
        venue,
        doi,
        abstract_text,
        keywords,
        status: PaperStatus::Unread,
        rating: None,
        pdf_path: pdf_path.to_string_lossy().to_string(),
        note_path: String::new(),
        created_at: now,
        updated_at: now,
    };
    paper::insert(&vault, &p)?;

    // 来自 resolver 的额外 identifier（除 DOI 之外的 scheme）。
    if let Some((_, _, m)) = resolver_meta.as_ref() {
        for (scheme, val) in &m.identifiers {
            if scheme == "doi" {
                continue;
            }
            let _ = crate::db::open(&vault).and_then(|conn| {
                conn.execute(
                    "INSERT OR IGNORE INTO identifiers (paper_id, type, value, is_primary)
                     VALUES (?1, ?2, ?3, 0)",
                    rusqlite::params![paper_id, scheme, val],
                )
                .map_err(AppError::from)
            });
        }
    }

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
) -> AppResult<Vec<Paper>> {
    let vault = require_vault(&state)?;
    let parsed = status.as_deref().map(PaperStatus::parse);
    paper::list(
        &vault,
        parsed,
        collection_id.as_deref(),
        None,
    )
}

#[tauri::command]
pub async fn get_paper(state: State<'_, AppState>, id: String) -> AppResult<PaperDetail> {
    let vault = require_vault(&state)?;
    paper::get(&vault, &id)
}

// ============================================================
// P2: 合并 + 5 分钟撤销
// ============================================================

#[tauri::command]
pub async fn merge_papers(
    state: State<'_, AppState>,
    src_id: String,
    dst_id: String,
) -> AppResult<crate::types::MergeResult> {
    let vault = require_vault(&state)?;
    let r = crate::services::merge::merge_papers(&vault, &src_id, &dst_id)?;
    Ok(crate::types::MergeResult {
        merge_id: r.merge_id,
        canonical_id: r.canonical_id,
        duplicate_id: r.duplicate_id,
        fields_merged: r.fields_merged,
        snapshot: None,
        merged_at: r.merged_at,
    })
}

#[tauri::command]
pub async fn undo_merge(state: State<'_, AppState>, merge_id: i64) -> AppResult<()> {
    let vault = require_vault(&state)?;
    crate::services::merge::undo_merge(&vault, merge_id)
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
pub async fn delete_paper(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let vault = require_vault(&state)?;
    paper::delete(&vault, &id)
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
        confidence: 0.7,
        identifiers: Vec::new(),
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
        return Err(AppError::NotFound(format!(
            "PDF 文件不存在: {}",
            detail.paper.pdf_path
        )));
    }
    Ok(std::fs::read(path)?)
}

#[tauri::command]
pub async fn open_pdf(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let vault = require_vault(&state)?;
    let detail = paper::get(&vault, &id)?;
    let path = Path::new(&detail.paper.pdf_path);
    if !path.exists() {
        return Err(AppError::NotFound(format!(
            "PDF 文件不存在: {}",
            detail.paper.pdf_path
        )));
    }
    open::that_detached(path).map_err(|e| AppError::Other(e.to_string()))
}

#[tauri::command]
pub async fn list_collections(state: State<'_, AppState>) -> AppResult<Vec<Collection>> {
    let vault = require_vault(&state)?;
    let conn = crate::db::open(&vault)?;
    let mut stmt =
        conn.prepare("SELECT id, name, parent_id, created_at FROM collections ORDER BY name")?;
    let rows = stmt.query_map([], |r| {
        Ok(Collection {
            id: r.get(0)?,
            name: r.get(1)?,
            parent_id: r.get(2)?,
            created_at: r.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

#[tauri::command]
pub async fn create_collection(
    state: State<'_, AppState>,
    name: String,
    parent_id: Option<String>,
) -> AppResult<Collection> {
    let vault = require_vault(&state)?;
    let conn = crate::db::open(&vault)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let now = chrono::Local::now().timestamp_millis();
    conn.execute(
        "INSERT INTO collections (id, name, parent_id, created_at) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![id, name, parent_id, now],
    )?;
    Ok(Collection {
        id,
        name,
        parent_id,
        created_at: now,
    })
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

/// 关键词聚合（按使用次数降序）。
#[tauri::command]
pub async fn list_keywords(state: State<'_, AppState>) -> AppResult<Vec<(String, i64)>> {
    let vault = require_vault(&state)?;
    paper::list_keywords(&vault)
}

/// 兼容旧 API：P0 移除 tags 表，返回空列表。
#[tauri::command]
pub async fn list_tags(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let _ = state;
    Ok(Vec::new())
}

// ============================================================
// P1: import_by_identifier
// ============================================================

#[tauri::command]
pub async fn import_by_identifier(
    state: State<'_, AppState>,
    raw: String,
) -> AppResult<ImportResult> {
    let vault = require_vault(&state)?;

    // 1) 解析输入
    let parsed = identifier::parse(&raw);
    let (scheme, value) = parsed
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Other("未识别出有效的 identifier".into()))?;

    // 2) resolver 拉元数据
    let resolver = resolver::default_resolver(scheme)?;
    let meta = resolver.fetch(&value).await?;

    // 3) 稳定 paper id
    let prefix = match scheme {
        Scheme::Doi => "meta-doi",
        Scheme::Arxiv => "meta-arxiv",
        Scheme::Pmid => "meta-pmid",
        Scheme::Isbn => "meta-isbn",
    };
    let paper_id = paper::stable_id(prefix, &format!("{prefix}|{value}"));

    // 4) duplicate check
    let duplicates: Vec<DuplicateCandidate> = if !meta.doi.is_empty() {
        duplicate::detect(
            &vault,
            Some(&meta.doi),
            if meta.title.is_empty() { None } else { Some(&meta.title) },
            Some(&meta.authors),
            meta.year,
        )?
    } else {
        duplicate::detect(
            &vault,
            None,
            if meta.title.is_empty() { None } else { Some(&meta.title) },
            Some(&meta.authors),
            meta.year,
        )?
    };

    // 5) 写入 DB（insert_from_metadata 会处理 papers / creators /
    //    paper_creators / keywords / paper_keywords / DOI identifiers）。
    paper::insert_from_metadata(&vault, &paper_id, &meta)?;
    if let Err(e) = crate::services::index::reindex_paper(&vault, &paper_id) {
        log::warn!("identifier 导入后索引失败 {paper_id}: {e}");
    }

    // 6) 回读最新视图
    let detail = paper::get(&vault, &paper_id)?;
    Ok(ImportResult {
        paper: detail.paper,
        duplicates,
    })
}
