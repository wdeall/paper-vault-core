//! 初始化 / 备份 / 打开库目录

use crate::error::AppResult;
use crate::vault;
use crate::types::VaultInfo;
use tauri::{AppHandle, Manager, State};

use crate::AppState;

#[tauri::command]
pub async fn init_vault(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> AppResult<()> {
    let p = std::path::PathBuf::from(&path);
    vault::init_at(&p)?;
    crate::db::migrate(&p)?;
    // seed 内置 AI 预设
    crate::services::preset::seed_builtins_if_empty(&p)?;
    vault::save_vault_path(&app, &p)?;
    *state.vault_path.write() = Some(p);
    Ok(())
}

#[tauri::command]
pub async fn get_vault_info(state: State<'_, AppState>) -> AppResult<VaultInfo> {
    let guard = state.vault_path.read();
    let path = guard
        .as_ref()
        .ok_or_else(|| crate::error::AppError::Config("vault 未初始化".into()))?;
    vault::vault_info(path)
}

#[tauri::command]
pub async fn open_vault_folder(state: State<'_, AppState>) -> AppResult<()> {
    let guard = state.vault_path.read();
    let path = guard
        .as_ref()
        .ok_or_else(|| crate::error::AppError::Config("vault 未初始化".into()))?;
    vault::open_in_explorer(path)
}

#[tauri::command]
pub async fn backup_database(state: State<'_, AppState>) -> AppResult<String> {
    let guard = state.vault_path.read();
    let path = guard
        .as_ref()
        .ok_or_else(|| crate::error::AppError::Config("vault 未初始化".into()))?;
    let p = vault::backup_database(path)?;
    Ok(p.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn load_seed_data(state: State<'_, AppState>) -> AppResult<Vec<String>> {
    let guard = state.vault_path.read();
    let path = guard
        .as_ref()
        .ok_or_else(|| crate::error::AppError::Config("vault 未初始化".into()))?;
    let ids = crate::seed::load(path)
        .map_err(|e| crate::error::AppError::Other(e.to_string()))?;
    Ok(ids)
}
