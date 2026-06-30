//! 导出命令

use crate::commands::common::require_vault;
use crate::error::AppResult;
use crate::services::paper;
use crate::export::{bibtex, citation, csl, ris};
use tauri::State;

use crate::AppState;

#[tauri::command]
pub async fn export_bibtex(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> AppResult<String> {
    let vault = require_vault(&state)?;
    let mut papers = Vec::new();
    for id in &ids {
        let d = paper::get(&vault, id)?;
        papers.push(d.paper);
    }
    Ok(bibtex::render(&papers))
}

#[tauri::command]
pub async fn export_markdown_citation(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> AppResult<String> {
    let vault = require_vault(&state)?;
    let mut papers = Vec::new();
    for id in &ids {
        let d = paper::get(&vault, id)?;
        papers.push(d.paper);
    }
    Ok(citation::render(&papers))
}

#[tauri::command]
pub async fn export_ris(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> AppResult<String> {
    let vault = require_vault(&state)?;
    let mut papers = Vec::new();
    for id in &ids {
        let d = paper::get(&vault, id)?;
        papers.push(d.paper);
    }
    Ok(ris::render(&papers))
}

#[tauri::command]
pub async fn export_csl_json(
    state: State<'_, AppState>,
    ids: Vec<String>,
) -> AppResult<String> {
    let vault = require_vault(&state)?;
    let mut papers = Vec::new();
    for id in &ids {
        let d = paper::get(&vault, id)?;
        papers.push(d.paper);
    }
    Ok(csl::papers_to_csl_json(&papers))
}
