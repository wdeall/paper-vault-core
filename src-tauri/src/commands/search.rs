//! P3 双通道搜索命令

use crate::commands::common::require_vault;
use crate::error::AppResult;
use crate::services::{index, search};
use crate::types::{IndexStatusSummary, PaperSummary, SearchHit, StructuredQuery};
use tauri::State;

use crate::AppState;

#[tauri::command]
pub async fn search_structured(
    state: State<'_, AppState>,
    query: StructuredQuery,
) -> AppResult<Vec<PaperSummary>> {
    let vault = require_vault(&state)?;
    search::search_structured(&vault, &query)
}

#[tauri::command]
pub async fn search_fulltext(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> AppResult<Vec<SearchHit>> {
    let vault = require_vault(&state)?;
    search::search_fulltext(&vault, &query, limit)
}

#[tauri::command]
pub async fn search_both(
    state: State<'_, AppState>,
    query: StructuredQuery,
    fts_query: String,
    limit: Option<usize>,
) -> AppResult<Vec<PaperSummary>> {
    let vault = require_vault(&state)?;
    search::search_both(&vault, &query, &fts_query, limit)
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
