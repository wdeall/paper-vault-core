//! 命令公共工具：vault 路径获取等。

use crate::error::{AppError, AppResult};
use crate::AppState;
use tauri::State;

/// 从 AppState 读取 vault 路径。若未初始化返回 `AppError::Config`。
pub fn require_vault(state: &State<'_, AppState>) -> AppResult<std::path::PathBuf> {
    let guard = state.vault_path.read();
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| AppError::Config("vault 未初始化".into()))
}
