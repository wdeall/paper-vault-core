//! AI 命令

use crate::error::AppResult;
use crate::services::preset;
use crate::services::ai_svc;
use crate::types::{AIResult, AISkillPreset};
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
pub async fn get_ai_presets(state: State<'_, AppState>) -> AppResult<Vec<AISkillPreset>> {
    let vault = require_vault(&state)?;
    preset::list(&vault)
}

#[tauri::command]
pub async fn update_ai_preset(
    state: State<'_, AppState>,
    id: String,
    patch: AISkillPreset,
) -> AppResult<AISkillPreset> {
    let vault = require_vault(&state)?;
    preset::update_user(&vault, &id, &patch)
}

#[tauri::command]
pub async fn reset_ai_preset(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<AISkillPreset> {
    let vault = require_vault(&state)?;
    preset::reset(&vault, &id)
}

#[tauri::command]
pub async fn run_ai(
    state: State<'_, AppState>,
    preset_id: String,
    paper_id: Option<String>,
    input: Option<String>,
) -> AppResult<AIResult> {
    let vault = require_vault(&state)?;
    ai_svc::run(&vault, &preset_id, paper_id.as_deref(), input.as_deref()).await
}
