//! 笔记相关命令

use crate::commands::common::require_vault;
use crate::error::AppResult;
use crate::services::note;
use crate::types::NoteContent;
use std::path::Path;
use tauri::State;

use crate::AppState;

#[tauri::command]
pub async fn create_note(state: State<'_, AppState>, id: String) -> AppResult<String> {
    let vault = require_vault(&state)?;
    note::create(&vault, &id)
}

#[tauri::command]
pub async fn import_note(
    state: State<'_, AppState>,
    id: String,
    source_path: String,
) -> AppResult<String> {
    let vault = require_vault(&state)?;
    note::import_external(&vault, &id, Path::new(&source_path))
}

#[tauri::command]
pub async fn get_note(state: State<'_, AppState>, id: String) -> AppResult<NoteContent> {
    let vault = require_vault(&state)?;
    note::get(&vault, &id)
}

#[tauri::command]
pub async fn update_note(
    state: State<'_, AppState>,
    id: String,
    content: String,
) -> AppResult<()> {
    let vault = require_vault(&state)?;
    note::update(&vault, &id, &content)
}

#[tauri::command]
pub async fn update_note_ai_block(
    state: State<'_, AppState>,
    id: String,
    block: String,
    content: String,
) -> AppResult<()> {
    let vault = require_vault(&state)?;
    note::update_ai_block(&vault, &id, &block, &content)
}
