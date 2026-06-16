//! 导出命令

use crate::error::AppResult;
use crate::services::paper;
use crate::export::{bibtex, citation};
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
