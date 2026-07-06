//! AI 命令

use crate::commands::common::require_vault;
use crate::error::AppResult;
use crate::services::preset;
use crate::services::ai_svc;
use crate::types::{AIResult, AISkillPreset, ChatMessageInput};
use tauri::State;

use crate::AppState;

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

/// AI 多轮对话：以论文元数据为上下文，历史消息由前端传入。
#[tauri::command]
pub async fn chat_with_paper(
    state: State<'_, AppState>,
    paper_id: String,
    input: String,
    history: Vec<ChatMessageInput>,
) -> AppResult<String> {
    let vault = require_vault(&state)?;
    ai_svc::chat(&vault, &paper_id, &input, &history).await
}
