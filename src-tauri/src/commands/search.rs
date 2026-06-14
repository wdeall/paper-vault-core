//! 搜索命令

use crate::error::AppResult;
use crate::services::index;
use crate::types::{IndexStatusSummary, SearchHit};
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
pub async fn search(
    state: State<'_, AppState>,
    query: String,
    scopes: Option<Vec<String>>,
) -> AppResult<Vec<SearchHit>> {
    let vault = require_vault(&state)?;
    index::search(&vault, &query, scopes.as_deref())
}

#[tauri::command]
pub async fn reindex_paper(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let vault = require_vault(&state)?;
    index::reindex_paper(&vault, &id)
}

#[tauri::command]
pub async fn reindex_all(state: State<'_, AppState>) -> AppResult<()> {
    let vault = require_vault(&state)?;
    index::reindex_all(&vault)
}

#[tauri::command]
pub async fn get_fts_status(state: State<'_, AppState>) -> AppResult<IndexStatusSummary> {
    let vault = require_vault(&state)?;
    index::status_summary(&vault)
}
