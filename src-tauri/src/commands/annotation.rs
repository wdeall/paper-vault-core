//! P4: PDF 批注命令

use crate::commands::common::require_vault;
use crate::error::AppResult;
use crate::services::annotation;
use crate::types::{Annotation, AnnotationInput};
use tauri::State;

use crate::AppState;

#[tauri::command]
pub async fn create_annotation(
    state: State<'_, AppState>,
    paper_id: String,
    input: AnnotationInput,
) -> AppResult<Annotation> {
    let vault = require_vault(&state)?;
    annotation::create(&vault, &paper_id, &input)
}

#[tauri::command]
pub async fn list_annotations(
    state: State<'_, AppState>,
    paper_id: String,
) -> AppResult<Vec<Annotation>> {
    let vault = require_vault(&state)?;
    annotation::list_by_paper(&vault, &paper_id)
}

#[tauri::command]
pub async fn update_annotation(
    state: State<'_, AppState>,
    id: String,
    color: Option<String>,
    text: Option<String>,
    comment: Option<String>,
    rect: Option<String>,
) -> AppResult<Annotation> {
    let vault = require_vault(&state)?;
    annotation::update(
        &vault,
        &id,
        color.as_deref(),
        text.as_deref(),
        comment.as_deref(),
        rect.as_deref(),
    )
}

#[tauri::command]
pub async fn delete_annotation(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let vault = require_vault(&state)?;
    annotation::delete(&vault, &id)
}

#[tauri::command]
pub async fn sync_annotations_to_note(
    state: State<'_, AppState>,
    paper_id: String,
) -> AppResult<()> {
    let vault = require_vault(&state)?;
    annotation::sync_to_note(&vault, &paper_id)
}
