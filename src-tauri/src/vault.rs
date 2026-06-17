//! Vault 模块：管理 PaperVault/ 库目录。

use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

pub const VAULT_CONFIG_FILE: &str = "vault.json";
pub const PDFS_DIR: &str = "pdfs";
pub const NOTES_DIR: &str = "notes";
pub const NOTES_PAPERS_DIR: &str = "papers";
pub const NOTES_TOPICS_DIR: &str = "topics";
pub const ATTACHMENTS_DIR: &str = "attachments";
pub const EXPORTS_DIR: &str = "exports";
pub const BACKUPS_DIR: &str = "backups";
pub const DB_FILE: &str = "papers.db";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VaultConfig {
    pub path: PathBuf,
}

/// 加载上次保存的 vault 路径。失败或不存在返回 None。
pub fn load_last_vault_path(app: &AppHandle) -> Option<PathBuf> {
    let cfg_dir = app.path().app_config_dir().ok()?;
    let p = cfg_dir.join(VAULT_CONFIG_FILE);
    let raw = fs::read_to_string(&p).ok()?;
    let cfg: VaultConfig = serde_json::from_str(&raw).ok()?;
    Some(cfg.path)
}

pub fn save_vault_path(app: &AppHandle, path: &Path) -> AppResult<()> {
    let cfg_dir = app
        .path()
        .app_config_dir()
        .map_err(|e| AppError::Other(e.to_string()))?;
    fs::create_dir_all(&cfg_dir)?;
    let raw = serde_json::to_string_pretty(&VaultConfig { path: path.to_path_buf() })?;
    fs::write(cfg_dir.join(VAULT_CONFIG_FILE), raw)?;
    Ok(())
}

/// 在指定路径创建 PaperVault 目录结构。已存在则校验必备子目录。
pub fn init_at(path: &Path) -> AppResult<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
    }
    let meta = fs::metadata(path)?;
    if !meta.is_dir() {
        return Err(AppError::Invalid(format!(
            "{} 不是目录",
            path.display()
        )));
    }
    // 写权限检查
    let probe = path.join(".write_probe");
    fs::write(&probe, b"ok")?;
    let _ = fs::remove_file(&probe);

    // 必备子目录
    for sub in [
        PDFS_DIR,
        NOTES_DIR,
        format!("{NOTES_DIR}/{NOTES_PAPERS_DIR}").as_str(),
        format!("{NOTES_DIR}/{NOTES_TOPICS_DIR}").as_str(),
        ATTACHMENTS_DIR,
        EXPORTS_DIR,
        BACKUPS_DIR,
    ] {
        fs::create_dir_all(path.join(sub))?;
    }
    Ok(())
}

pub fn vault_info(path: &Path) -> AppResult<crate::types::VaultInfo> {
    let db_path = path.join(DB_FILE);
    let paper_count = if db_path.exists() {
        crate::db::count_papers(path)?
    } else {
        0
    };
    let indexed_count = if db_path.exists() {
        crate::db::count_indexed(path)?
    } else {
        0
    };
    Ok(crate::types::VaultInfo {
        path: path.to_string_lossy().to_string(),
        paper_count,
        indexed_count,
    })
}

/// 复制 PDF 到 vault 内 `pdfs/YYYY/{id}-{slug}.pdf`。
/// 返回最终目标路径。
pub fn copy_pdf(vault: &Path, src: &Path, paper_id: &str, title: &str) -> AppResult<PathBuf> {
    use std::io::Read;
    if !src.exists() {
        return Err(AppError::NotFound(format!(
            "源文件不存在: {}",
            src.display()
        )));
    }
    // 大小限制 200MB
    let meta = fs::metadata(src)?;
    if meta.len() > 200 * 1024 * 1024 {
        return Err(AppError::Invalid(format!(
            "PDF 超过 200MB 限制 ({} bytes)",
            meta.len()
        )));
    }
    // 扩展名
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "pdf" {
        return Err(AppError::Invalid(format!("不支持的扩展名: .{ext}")));
    }

    let year = chrono::Local::now().format("%Y").to_string();
    let year_dir = vault.join(PDFS_DIR).join(&year);
    fs::create_dir_all(&year_dir)?;

    let slug = slug_from_title(title);
    let filename = if slug.is_empty() {
        format!("{paper_id}.pdf")
    } else {
        format!("{paper_id}-{slug}.pdf")
    };
    let mut dst = year_dir.join(&filename);
    let mut counter = 1u32;
    while dst.exists() {
        let stem = dst.file_stem().and_then(|s| s.to_str()).unwrap_or("p");
        let new_name = format!("{stem}-{counter}.pdf");
        dst = year_dir.join(new_name);
        counter += 1;
    }

    // 流式复制
    let mut src_f = fs::File::open(src)?;
    let mut buf = Vec::with_capacity(meta.len() as usize);
    src_f.read_to_end(&mut buf)?;
    fs::write(&dst, &buf)?;
    Ok(dst)
}

/// 简单 slug：ASCII 转小写、非字母数字替换为 -、合并连续 -、去首尾 -、截断 60 字符。
pub fn slug_from_title(title: &str) -> String {
    let s: String = title
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let mut out = String::with_capacity(s.len());
    let mut last_dash = true;
    for ch in s.chars() {
        if ch == '-' {
            if !last_dash {
                out.push(ch);
            }
            last_dash = true;
        } else {
            out.push(ch);
            last_dash = false;
        }
    }
    let trimmed = out.trim_matches('-').to_string();
    if trimmed.len() > 60 {
        trimmed[..60].trim_end_matches('-').to_string()
    } else {
        trimmed
    }
}

/// 复制普通文件（用于导入 md 等）。
pub fn copy_file(src: &Path, dst: &Path) -> AppResult<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(src, dst)?;
    Ok(())
}

/// 在系统资源管理器中打开路径。
pub fn open_in_explorer(path: &Path) -> AppResult<()> {
    open::that_detached(path).map_err(|e| AppError::Other(e.to_string()))
}

/// 备份数据库。
pub fn backup_database(vault: &Path) -> AppResult<PathBuf> {
    let db_src = vault.join(DB_FILE);
    if !db_src.exists() {
        return Err(AppError::NotFound("papers.db 不存在".into()));
    }
    let ts = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let dst = vault.join(BACKUPS_DIR).join(format!("papers-{ts}.db"));
    fs::create_dir_all(dst.parent().unwrap())?;
    fs::copy(&db_src, &dst)?;
    Ok(dst)
}

/// 把 vault 相对路径解析为绝对路径。`..` 穿越会被拒绝。
#[allow(dead_code)] // 主要给测试与未来 PDF 相对路径解析用；当前 commands/* 直接用 vault::join 即可。
pub fn resolve_safe(vault: &Path, rel: &str) -> AppResult<PathBuf> {
    if rel.contains("..") {
        return Err(AppError::Invalid("路径含 '..' 拒绝".into()));
    }
    let p = vault.join(rel);
    p.canonicalize().map_err(|e| AppError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn slug_works() {
        assert_eq!(slug_from_title("Hello World! 2024"), "hello-world-2024");
        assert_eq!(slug_from_title("  --  "), "");
        assert_eq!(slug_from_title("中文标题"), "");
        assert_eq!(slug_from_title(&"A".repeat(80)).len(), 60);
    }

    #[test]
    fn init_at_creates_layout() {
        let dir = tempdir().unwrap();
        init_at(dir.path()).unwrap();
        assert!(dir.path().join(PDFS_DIR).is_dir());
        assert!(dir.path().join(NOTES_DIR).join(NOTES_PAPERS_DIR).is_dir());
        assert!(dir.path().join(BACKUPS_DIR).is_dir());
    }

    #[test]
    fn resolve_safe_blocks_traversal() {
        let dir = tempdir().unwrap();
        init_at(dir.path()).unwrap();
        assert!(resolve_safe(dir.path(), "../etc/passwd").is_err());
    }
}
