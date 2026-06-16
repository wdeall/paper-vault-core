//! P0 Zotero 数据模型对齐：备份 + 数据搬运 + 失败回滚。
//!
//! 由 `db::migrate()` 在检测到旧 schema 时调用。流程：
//!
//! 1. 把 `vault.db` 复制到 `backups/vault-<timestamp>.db`（原子操作）。
//! 2. 在一次事务中：
//!    - 正规化 `papers.status`（未读→unread / 阅读中→reading / 已读→read / 重点重读→read）。
//!    - 把 `papers.authors` JSON 拆到 `creators` + `paper_creators`。
//!    - 把 `papers.keywords` JSON 拆到 `keywords` + `paper_keywords`。
//!    - 把 `papers.doi` 拆到 `identifiers`。
//!    - 把 `pdf_path` 拆到 `attachments` (kind='pdf')；
//!      若有 `note_path` 再补一条 `kind='note'` 记录。
//!    - 重建 `papers_fts`。
//! 3. 任意步骤失败 → 回滚事务；备份保留以便人工恢复。
//!
//! 旧 `papers.authors / keywords / tags` JSON 列**不**删除（按 v2.0 PLAN
//! §3.4 step 8，物理删除放到 P0 最后或后续阶段；P0 内只做逻辑废弃）。

use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::{Path, PathBuf};

/// 当前 schema 版本号。`migrate()` 在最后写入 `schema_version` 行。
pub const TARGET_SCHEMA_VERSION: i32 = 2;

/// 状态映射：旧值 → 新枚举。
///
/// v2.0 PLAN §3.3：
///   未读       -> unread
///   阅读中     -> reading
///   已读       -> read
///   重点重读   -> read
pub fn normalize_status(old: &str) -> &'static str {
    match old.trim() {
        "未读" => "unread",
        "阅读中" => "reading",
        "已读" => "read",
        "重点重读" => "read",
        "unread" | "reading" | "read" => match old.trim() {
            "unread" => "unread",
            "reading" => "reading",
            "read" => "read",
            _ => "unread",
        },
        // 未知值兜底为 unread，避免 CHECK 约束把事务打回。
        _ => "unread",
    }
}

/// 把字符串格式化为可作为 SQLite 主键 / slug 用的 id。
///
/// `creators` 与 `keywords` 的主键基于稳定哈希生成，避免名字中带
/// 特殊字符 / 空白 / 大小写差异导致同一实体产生多行。
fn stable_id(prefix: &str, raw: &str) -> String {
    // 简化稳定哈希：djb2 + 长度 mix。无需密码学强度，只要同一字符串
    // 多次计算结果一致即可。
    let mut hash: u64 = 5381;
    for b in raw.as_bytes() {
        hash = hash
            .wrapping_mul(33)
            .wrapping_add(u64::from(*b));
    }
    // 64-bit 拆成两段 32-bit，用 hex 拼出 16 字符。
    format!("{prefix}-{:08x}{:08x}", (hash >> 32) as u32, hash as u32)
}

fn has_legacy_json_columns(conn: &Connection) -> bool {
    // 旧 schema 中 papers 表有 `authors` 列且为 TEXT。P0 新建时不应该
    // 再有该列（app 层不再使用），因此它的存在 = 旧库。
    let stmt = conn.prepare("PRAGMA table_info(papers)").ok();
    if let Some(mut stmt) = stmt {
        if let Ok(rows) = stmt.query([]) {
            let mut iter = rows;
            while let Some(row) = iter.next().ok().flatten() {
                let name: String = row.get(1).unwrap_or_default();
                if name == "authors" || name == "keywords" || name == "tags" {
                    return true;
                }
            }
        }
    }
    false
}

/// 把 `papers.db` 复制到 `backups/vault-<unix_ms>.db`。
///
/// 失败返回 `AppError::Io`，但**不**终止迁移流程：备份只是
/// 兜底，主流程（事务搬运）失败时会由回滚保证一致性。
fn backup_database(vault: &Path) -> AppResult<PathBuf> {
    let src = vault.join(crate::vault::DB_FILE);
    if !src.exists() {
        return Err(AppError::Io(format!("备份源文件不存在: {}", src.display())));
    }
    let backups_dir = vault.join(crate::vault::BACKUPS_DIR);
    std::fs::create_dir_all(&backups_dir).map_err(|e| {
        AppError::Io(format!(
            "无法创建备份目录 {}: {e}",
            backups_dir.display()
        ))
    })?;
    let stamp = chrono::Local::now().timestamp_millis();
    let dst = backups_dir.join(format!("vault-{stamp}.db"));
    std::fs::copy(&src, &dst)
        .map_err(|e| AppError::Io(format!("备份 {} → {} 失败: {e}", src.display(), dst.display())))?;
    log::info!("已备份 {} → {}", src.display(), dst.display());
    Ok(dst)
}

/// 把旧 `authors` 字符串拆成 `(family_name, given_name, display_name)`。
///
/// 输入可能是：
/// - "Alice Smith"        → family="Smith", given="Alice"
/// - "Smith, Alice"       → family="Smith", given="Alice"
/// - "J. K. Smith"        → family="Smith", given="J. K."
/// - 单字 "Madonna"       → family="Madonna", given=""
fn split_creator_name(raw: &str) -> (String, String, String) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (String::new(), String::new(), String::new());
    }
    // 形如 "Family, Given" 用逗号分隔。
    if let Some((family, given)) = trimmed.split_once(',') {
        return (
            family.trim().to_string(),
            given.trim().to_string(),
            trimmed.to_string(),
        );
    }
    // 否则按最后一个空格切。
    if let Some((given, family)) = trimmed.rsplit_once(' ') {
        return (
            family.trim().to_string(),
            given.trim().to_string(),
            trimmed.to_string(),
        );
    }
    // 单词：整段当 family。
    (trimmed.to_string(), String::new(), trimmed.to_string())
}

pub fn upsert_creator(
    conn: &Connection,
    raw: &str,
    family: &str,
    given: &str,
    paper_id: &str,
    position: i32,
    role: &str,
) -> AppResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let id = stable_id("cr", &format!("{family}|{given}|{trimmed}").to_lowercase());
    conn.execute(
        "INSERT OR IGNORE INTO creators (id, family_name, given_name, display_name, raw)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, family, given, trimmed, trimmed],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO paper_creators (paper_id, creator_id, position, role)
         VALUES (?1, ?2, ?3, ?4)",
        params![paper_id, id, position, role],
    )?;
    Ok(id)
}

fn upsert_keyword(
    conn: &Connection,
    name: &str,
    paper_id: &str,
) -> AppResult<String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let id = stable_id("kw", &trimmed.to_lowercase());
    conn.execute(
        "INSERT OR IGNORE INTO keywords (id, name, source) VALUES (?1, ?2, 'manual')",
        params![id, trimmed],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO paper_keywords (paper_id, keyword_id) VALUES (?1, ?2)",
        params![paper_id, id],
    )?;
    Ok(id)
}

/// 把旧 `papers` 行的 JSON 数据写入新结构化表（不含 status 正规化）。
///
/// 拆分 row 数据避免借用问题：把所有字段先读成 owned String。
fn migrate_paper_fields(
    conn: &Connection,
    paper_id: &str,
    authors_json: &str,
    keywords_json: &str,
    doi: &str,
    pdf_path: &str,
    note_path: &str,
) -> AppResult<()> {
    // authors
    if let Ok(list) = serde_json::from_str::<Vec<String>>(authors_json) {
        for (pos, name) in list.iter().enumerate() {
            if name.trim().is_empty() {
                continue;
            }
            let (family, given, display) = split_creator_name(name);
            upsert_creator(conn, &display, &family, &given, paper_id, pos as i32, "author")?;
        }
    }

    // keywords
    if let Ok(list) = serde_json::from_str::<Vec<String>>(keywords_json) {
        for kw in list.iter() {
            if kw.trim().is_empty() {
                continue;
            }
            upsert_keyword(conn, kw, paper_id)?;
        }
    }

    // identifiers (DOI)
    let doi_trim = doi.trim();
    if !doi_trim.is_empty() {
        conn.execute(
            "INSERT OR IGNORE INTO identifiers (paper_id, type, value, is_primary)
             VALUES (?1, 'doi', ?2, 1)",
            params![paper_id, doi_trim],
        )?;
    }

    // attachments
    if !pdf_path.trim().is_empty() {
        let aid = stable_id("att-pdf", &format!("{paper_id}|{pdf_path}"));
        conn.execute(
            "INSERT OR IGNORE INTO attachments
                (id, paper_id, kind, rel_path, title, status)
             VALUES (?1, ?2, 'pdf', ?3, 'main', 'active')",
            params![aid, paper_id, pdf_path],
        )?;
    }
    if !note_path.trim().is_empty() {
        let aid = stable_id("att-note", &format!("{paper_id}|{note_path}"));
        conn.execute(
            "INSERT OR IGNORE INTO attachments
                (id, paper_id, kind, rel_path, title, status)
             VALUES (?1, ?2, 'note', ?3, 'main', 'active')",
            params![aid, paper_id, note_path],
        )?;
    }
    Ok(())
}

/// 重建 `papers_fts`。
fn rebuild_papers_fts(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM papers_fts", [])?;
    let mut stmt = conn.prepare(
        "SELECT p.id, p.title, p.abstract_text, p.venue, p.doi,
                COALESCE((SELECT GROUP_CONCAT(c.display_name, ' ')
                          FROM paper_creators pc
                          JOIN creators c ON c.id = pc.creator_id
                          WHERE pc.paper_id = p.id
                          ORDER BY pc.position), '') AS authors_str,
                COALESCE((SELECT GROUP_CONCAT(k.name, ' ')
                          FROM paper_keywords pk
                          JOIN keywords k ON k.id = pk.keyword_id
                          WHERE pk.paper_id = p.id), '') AS keywords_str
         FROM papers p",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>("id")?,
            r.get::<_, String>("title")?,
            r.get::<_, String>("abstract_text")?,
            r.get::<_, String>("authors_str")?,
            r.get::<_, String>("keywords_str")?,
            r.get::<_, String>("venue")?,
            r.get::<_, String>("doi")?,
        ))
    })?;
    for row in rows {
        let (pid, title, abstract_text, authors, keywords, venue, doi) = row?;
        conn.execute(
            "INSERT INTO papers_fts (paper_id, title, abstract, authors, keywords, venue, doi)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![pid, title, abstract_text, authors, keywords, venue, doi],
        )?;
    }
    Ok(())
}

/// 主入口：执行 P0 数据搬运。已被 `db::migrate()` 调用。
///
/// `migrated_legacy` 表示本次是否实际搬运了旧 JSON 数据（仅在新表
/// 全部为空且旧列存在时为 true；幂等）。
pub fn run_migration(vault: &Path, conn: &mut Connection) -> AppResult<bool> {
    // ---- 1. 检测 ----
    if !has_legacy_json_columns(conn) {
        log::info!("P0 migrate: 未发现旧 JSON schema，跳过数据搬运");
        return Ok(false);
    }

    // ---- 1.5 已经搬运过吗？----
    // 即使旧 JSON 列保留着，如果 schema_version 已经 >= TARGET，
    // 就不要再跑一遍（避免 rebuild_papers_fts 等重复执行）。
    // 全新库：read_schema_version 失败 → 视为 v1。
    let current_version = read_schema_version(conn).unwrap_or(1);
    if current_version >= TARGET_SCHEMA_VERSION {
        log::info!("P0 migrate: schema_version={current_version}，跳过数据搬运");
        return Ok(false);
    }

    // ---- 2. 备份 ----
    if let Err(e) = backup_database(vault) {
        log::warn!("P0 migrate 备份未完成（继续尝试事务）: {e}");
    } else {
        log::info!("P0 migrate 备份成功");
    }

    // ---- 3. 事务搬运 ----
    let tx = conn.transaction()?;
    let result: AppResult<()> = (|| {
        // 读所有 paper 的旧 JSON 字段（owned 化以避免借用问题）。
        let mut stmt = tx.prepare(
            "SELECT id, status, authors, keywords, doi, pdf_path, note_path
             FROM papers",
        )?;
        let rows: Vec<(String, String, String, String, String, String, String)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();
        drop(stmt);

        let now = chrono::Local::now().timestamp_millis();
        for (paper_id, old_status, authors_json, keywords_json, doi, pdf_path, note_path) in rows {
            // 状态正规化
            let new_status = normalize_status(&old_status);
            tx.execute(
                "UPDATE papers SET status = ?1, updated_at = ?2 WHERE id = ?3",
                params![new_status, now, paper_id],
            )?;
            // 结构化表搬运
            migrate_paper_fields(
                &tx,
                &paper_id,
                &authors_json,
                &keywords_json,
                &doi,
                &pdf_path,
                &note_path,
            )?;
        }
        rebuild_papers_fts(&tx)?;
        Ok(())
    })();

    if let Err(e) = result {
        // 主动回滚。
        let _ = tx.rollback();
        log::error!("P0 migrate 失败，已回滚：{e}");
        return Err(e);
    }
    tx.commit()?;
    log::info!("P0 数据搬运完成");

    Ok(true)
}

/// 写/更新 schema_version 表中的当前版本号。
pub fn write_schema_version(conn: &Connection, version: i32) -> AppResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            upgraded_at INTEGER NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "INSERT INTO schema_version (version, upgraded_at)
         VALUES (?1, ?2)
         ON CONFLICT(version) DO UPDATE SET upgraded_at = excluded.upgraded_at",
        params![version, chrono::Local::now().timestamp_millis()],
    )?;
    Ok(())
}

/// 读取当前 schema_version；不存在返回 1（v1 = 仅 0001_init.sql）。
pub fn read_schema_version(conn: &Connection) -> AppResult<i32> {
    let exists: Option<i32> = conn
        .query_row(
            "SELECT MAX(version) FROM schema_version",
            [],
            |r| r.get(0),
        )
        .optional()?;
    Ok(exists.unwrap_or(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mapping() {
        assert_eq!(normalize_status("未读"), "unread");
        assert_eq!(normalize_status("阅读中"), "reading");
        assert_eq!(normalize_status("已读"), "read");
        assert_eq!(normalize_status("重点重读"), "read");
        assert_eq!(normalize_status("unread"), "unread");
        assert_eq!(normalize_status("reading"), "reading");
        assert_eq!(normalize_status("read"), "read");
        assert_eq!(normalize_status("garbage"), "unread");
    }

    #[test]
    fn creator_name_split_space() {
        let (f, g, d) = split_creator_name("Alice Smith");
        assert_eq!(f, "Smith");
        assert_eq!(g, "Alice");
        assert_eq!(d, "Alice Smith");
    }

    #[test]
    fn creator_name_split_comma() {
        let (f, g, d) = split_creator_name("Smith, Alice");
        assert_eq!(f, "Smith");
        assert_eq!(g, "Alice");
        assert_eq!(d, "Smith, Alice");
    }

    #[test]
    fn creator_name_single_word() {
        let (f, g, _d) = split_creator_name("Madonna");
        assert_eq!(f, "Madonna");
        assert_eq!(g, "");
    }

    #[test]
    fn stable_id_deterministic() {
        assert_eq!(stable_id("cr", "Alice Smith"), stable_id("cr", "Alice Smith"));
        assert_ne!(stable_id("cr", "Alice Smith"), stable_id("cr", "Bob Smith"));
    }
}
