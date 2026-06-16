//! Tauri IPC 命令

pub mod init;
pub mod papers;
pub mod notes;
pub mod search;
pub mod ai;
pub mod export;
pub mod settings;
pub mod indexer;

pub use init::*;
pub use papers::*;
pub use notes::*;
pub use ai::*;
pub use export::*;
pub use settings::*;
pub use indexer::*;
