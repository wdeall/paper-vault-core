//! 设置命令

use crate::commands::common::require_vault;
use crate::error::AppResult;
use crate::services::ai_svc;
use crate::types::AIProviderConfig;
use tauri::State;

use crate::AppState;

#[tauri::command]
pub async fn get_ai_config(state: State<'_, AppState>) -> AppResult<AIProviderConfig> {
    let vault = require_vault(&state)?;
    ai_svc::get_provider(&vault)
}

#[tauri::command]
pub async fn update_ai_config(
    state: State<'_, AppState>,
    patch: AIProviderConfig,
) -> AppResult<AIProviderConfig> {
    let vault = require_vault(&state)?;
    ai_svc::update_provider(&vault, &patch)
}
