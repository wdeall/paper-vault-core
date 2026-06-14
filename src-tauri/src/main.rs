// Tauri 2.x 桌面应用入口
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    paper_vault_lib::run();
}
