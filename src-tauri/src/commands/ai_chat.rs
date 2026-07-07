//! AI 对话历史 + 流式命令（agent 风格侧边栏后端）。

use crate::commands::common::require_vault;
use crate::error::AppResult;
use crate::services::ai_chat;
use crate::types::{AIConversation, AIMessage};
use tauri::{AppHandle, State};

use crate::AppState;

#[tauri::command]
pub async fn list_ai_conversations(
    state: State<'_, AppState>,
    paper_id: Option<String>,
) -> AppResult<Vec<AIConversation>> {
    let vault = require_vault(&state)?;
    ai_chat::list_conversations(&vault, paper_id.as_deref())
}

#[tauri::command]
pub async fn create_ai_conversation(
    state: State<'_, AppState>,
    paper_id: Option<String>,
    title: String,
) -> AppResult<AIConversation> {
    let vault = require_vault(&state)?;
    ai_chat::create_conversation(&vault, paper_id.as_deref(), &title)
}

#[tauri::command]
pub async fn delete_ai_conversation(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let vault = require_vault(&state)?;
    ai_chat::delete_conversation(&vault, &id)
}

#[tauri::command]
pub async fn rename_ai_conversation(
    state: State<'_, AppState>,
    id: String,
    title: String,
) -> AppResult<AIConversation> {
    let vault = require_vault(&state)?;
    ai_chat::rename_conversation(&vault, &id, &title)
}

#[tauri::command]
pub async fn list_ai_messages(
    state: State<'_, AppState>,
    conversation_id: String,
) -> AppResult<Vec<AIMessage>> {
    let vault = require_vault(&state)?;
    ai_chat::list_messages(&vault, &conversation_id)
}

#[tauri::command]
pub async fn send_ai_message(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    content: String,
) -> AppResult<AIMessage> {
    let vault = require_vault(&state)?;
    ai_chat::send_message(&vault, &app, &conversation_id, &content).await
}

#[tauri::command]
pub async fn run_ai_preset_in_chat(
    app: AppHandle,
    state: State<'_, AppState>,
    conversation_id: String,
    preset_id: String,
) -> AppResult<AIMessage> {
    let vault = require_vault(&state)?;
    ai_chat::run_preset_in_chat(&vault, &app, &conversation_id, &preset_id).await
}

/// 手动触发对话历史总结。
/// 强制对所有历史消息（除最近 N 条）做 LLM 总结并缓存，返回更新后的会话对象。
#[tauri::command]
pub async fn summarize_ai_conversation(
    state: State<'_, AppState>,
    conversation_id: String,
) -> AppResult<AIConversation> {
    let vault = require_vault(&state)?;
    ai_chat::summarize_now(&vault, &conversation_id).await
}
