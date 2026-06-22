//! PaperVault 库入口
//!
//! 暴露 `run()` 给 `main.rs`，注册 Tauri Builder、命令、插件、状态。

mod commands;
mod services;
mod db;
mod vault;
mod pdf;
mod markdown;
mod ai;
mod export;
mod duplicates;
mod seed;
mod error;
mod types;

use std::sync::Arc;
use tauri::Manager;

/// 全局 AppState — 持有 vault 路径与运行时缓存
pub struct AppState {
    pub vault_path: Arc<parking_lot::RwLock<Option<std::path::PathBuf>>>,
}

impl AppState {
    fn new() -> Self {
        Self {
            vault_path: Arc::new(parking_lot::RwLock::new(None)),
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
#[allow(unexpected_cfgs)]
pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .manage(AppState::new())
        .setup(|app| {
            // 启动时尝试加载上次的 vault 路径
            if let Some(state) = app.try_state::<AppState>() {
                if let Some(path) = vault::load_last_vault_path(app.handle()) {
                    if let Err(e) = vault::init_at(&path) {
                        log::warn!("加载上次 vault 失败: {e}");
                    } else {
                        *state.vault_path.write() = Some(path.clone());
                        // 启动时清理超过 5 分钟撤销窗口的 merge_log 行（避免长
                        // 期运行的 vault 累积过期快照）。
                        if let Err(e) = services::merge::cleanup_old_merge_log(&path) {
                            log::warn!("清理过期 merge_log 失败: {e}");
                        }
                    }
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::init_vault,
            commands::get_vault_info,
            commands::open_vault_folder,
            commands::backup_database,
            commands::import_pdf,
            commands::import_pdfs_batch,
            commands::import_by_identifier,
            commands::merge_papers,
            commands::undo_merge,
            commands::list_papers,
            commands::get_paper,
            commands::update_paper,
            commands::delete_paper,
            commands::update_progress,
            commands::list_collections,
            commands::create_collection,
            commands::add_paper_to_collection,
            commands::remove_paper_from_collection,
            commands::list_keywords,
            commands::list_tags,
            commands::check_duplicates,
            commands::extract_metadata,
            commands::read_pdf_bytes,
            commands::open_pdf,
            commands::create_note,
            commands::import_note,
            commands::get_note,
            commands::update_note,
            commands::update_note_ai_block,
            commands::search_structured,
            commands::search_fulltext,
            commands::search_both,
            commands::reindex_paper,
            commands::reindex_all,
            commands::get_ai_presets,
            commands::update_ai_preset,
            commands::reset_ai_preset,
            commands::run_ai,
            commands::get_ai_config,
            commands::update_ai_config,
            commands::export_bibtex,
            commands::export_markdown_citation,
            commands::load_seed_data,
            commands::get_fts_status,
            commands::create_annotation,
            commands::list_annotations,
            commands::update_annotation,
            commands::delete_annotation,
            commands::sync_annotations_to_note,
        ])
        .run(tauri::generate_context!())
        .expect("error while running PaperVault");
}
