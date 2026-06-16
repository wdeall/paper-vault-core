//! 笔记相关命令

use crate::error::AppResult;
use crate::services::note;
use crate::types::NoteContent;
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
