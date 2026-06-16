//! 设置命令

use crate::error::AppResult;
use crate::services::ai_svc;
use crate::types::AIProviderConfig;
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
