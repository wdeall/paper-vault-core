//! P4: PDF 批注 CRUD + Markdown 导出

use crate::db;
use crate::error::{AppError, AppResult};
use crate::types::Annotation;
use rusqlite::{params, OptionalExtension};
use std::path::{Path, PathBuf};

// ============================================================
// 内部工具
// ============================================================

fn now_ms() -> i64 {
    chrono::Local::now().timestamp_millis()
}

fn row_to_annotation(row: &rusqlite::Row<'_>) -> rusqlite::Result<Annotation> {
    Ok(Annotation {
        id: row.get(0)?,
        paper_id: row.get(1)?,
        attachment_id: row.get(2)?,
        kind: row.get(3)?,
        page: row.get(4)?,
        rect: row.get(5)?,
        color: row.get(6)?,
        text: row.get(7)?,
        comment: row.get(8)?,
        created_at: row.get(9)?,
        modified_at: row.get(10)?,
    })
}

const SELECT_COLS: &str = "id, paper_id, attachment_id, kind, page, rect, color, text, comment,
                created_at, modified_at";

// ============================================================
// CRUD
// ============================================================

/// 创建批注。生成 UUID v4 作为 id，created_at = now_ms。
pub fn create(
    vault: &Path,
    paper_id: &str,
    kind: &str,
    page: Option<i32>,
    rect: Option<&str>,
    color: Option<&str>,
    text: Option<&str>,
    comment: Option<&str>,
) -> AppResult<Annotation> {
    let id = uuid::Uuid::new_v4().to_string();
    let created_at = now_ms();

    let conn = db::open(vault)?;
    conn.execute(
        "INSERT INTO annotations
            (id, paper_id, kind, page, rect, color, text, comment, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, paper_id, kind, page, rect, color, text, comment, created_at],
    )?;

    let ann = Annotation {
        id,
        paper_id: paper_id.to_string(),
        attachment_id: None,
        kind: kind.to_string(),
        page,
        rect: rect.map(|s| s.to_string()),
        color: color.map(|s| s.to_string()),
        text: text.map(|s| s.to_string()),
        comment: comment.map(|s| s.to_string()),
        created_at,
        modified_at: None,
    };
    Ok(ann)
}

/// 按 paper_id 列出批注，按 page ASC, created_at ASC 排序。
pub fn list_by_paper(vault: &Path, paper_id: &str) -> AppResult<Vec<Annotation>> {
    let conn = db::open(vault)?;
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLS} FROM annotations
         WHERE paper_id = ?1
         ORDER BY page ASC, created_at ASC"
    ))?;
    let rows = stmt.query_map(params![paper_id], row_to_annotation)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// 更新批注。只更新非 None 的字段（color / text / comment / rect）。
/// modified_at = now_ms。返回更新后的 Annotation。
pub fn update(
    vault: &Path,
    id: &str,
    color: Option<&str>,
    text: Option<&str>,
    comment: Option<&str>,
    rect: Option<&str>,
) -> AppResult<Annotation> {
    let conn = db::open(vault)?;

    // 先确认存在
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM annotations WHERE id = ?1",
        params![id],
        |r| r.get(0),
    )?;
    if exists == 0 {
        return Err(AppError::NotFound(format!("批注 {id} 不存在")));
    }

    let now = now_ms();

    // 动态拼 UPDATE：只更新非 None 的字段
    let mut sets: Vec<&str> = Vec::new();
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(v) = color {
        sets.push("color = ?");
        args.push(Box::new(v.to_string()));
    }
    if let Some(v) = text {
        sets.push("text = ?");
        args.push(Box::new(v.to_string()));
    }
    if let Some(v) = comment {
        sets.push("comment = ?");
        args.push(Box::new(v.to_string()));
    }
    if let Some(v) = rect {
        sets.push("rect = ?");
        args.push(Box::new(v.to_string()));
    }

    if sets.is_empty() {
        // 没有字段要更新，但仍刷新 modified_at 以满足"调用即更新"语义
        conn.execute(
            "UPDATE annotations SET modified_at = ?1 WHERE id = ?2",
            params![now, id],
        )?;
    } else {
        let sql = format!(
            "UPDATE annotations SET {}, modified_at = ? WHERE id = ?",
            sets.join(", ")
        );
        args.push(Box::new(now));
        args.push(Box::new(id.to_string()));
        let params_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| &**b).collect();
        conn.execute(&sql, params_refs.as_slice())?;
    }

    // 读回
    let ann: Annotation = conn.query_row(
        &format!("SELECT {SELECT_COLS} FROM annotations WHERE id = ?1"),
        params![id],
        row_to_annotation,
    )?;
    Ok(ann)
}

/// 删除批注。
pub fn delete(vault: &Path, id: &str) -> AppResult<()> {
    let conn = db::open(vault)?;
    conn.execute("DELETE FROM annotations WHERE id = ?1", params![id])?;
    Ok(())
}

// ============================================================
// Markdown 导出
// ============================================================

const MD_BLOCK_START: &str = "<!-- ANNOTATIONS_START -->";
const MD_BLOCK_END: &str = "<!-- ANNOTATIONS_END -->";

/// 导出批注为 Markdown。无批注返回空字符串。
///
/// 格式：
/// ```text
/// <!-- ANNOTATIONS_START -->
/// ## 批注
///
/// ### 第 {page} 页
///
/// > **[{color}]** {selected_text}
///
/// 💬 {comment}
///
/// ---
///
/// <!-- ANNOTATIONS_END -->
/// ```
pub fn export_to_markdown(vault: &Path, paper_id: &str) -> AppResult<String> {
    let anns = list_by_paper(vault, paper_id)?;
    if anns.is_empty() {
        return Ok(String::new());
    }

    let mut out = String::new();
    out.push_str(MD_BLOCK_START);
    out.push_str("\n## 批注\n\n");

    let mut last_page: Option<i32> = None;
    for ann in &anns {
        let page = ann.page.unwrap_or(0);
        if last_page != Some(page) {
            out.push_str(&format!("### 第 {page} 页\n\n"));
            last_page = Some(page);
        }

        let color = ann.color.clone().unwrap_or_default();
        let text = ann.text.clone().unwrap_or_default();
        out.push_str(&format!("> **[{color}]** {text}\n\n"));

        if let Some(c) = &ann.comment {
            if !c.is_empty() {
                out.push_str(&format!("💬 {c}\n\n"));
            }
        }

        out.push_str("---\n\n");
    }

    out.push_str(MD_BLOCK_END);
    out.push('\n');
    Ok(out)
}

// ============================================================
// 同步到笔记
// ============================================================

/// 把 note_path 解析为绝对路径。相对路径会拼接 vault 目录。
fn resolve_note_path(vault: &Path, note_path: &str) -> PathBuf {
    let p = Path::new(note_path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        vault.join(note_path)
    }
}

/// 把批注区块同步到 paper.note_path 指向的笔记文件。
///
/// - note_path 为空或文件不存在：返回 Ok(())（无笔记可同步）
/// - 已有 ANNOTATIONS_START/END 区块：替换区块内容
/// - 没有区块：追加到文件末尾
pub fn sync_to_note(vault: &Path, paper_id: &str) -> AppResult<()> {
    let md = export_to_markdown(vault, paper_id)?;

    let conn = db::open(vault)?;
    let note_path: Option<String> = conn
        .query_row(
            "SELECT note_path FROM papers WHERE id = ?1",
            params![paper_id],
            |r| r.get(0),
        )
        .optional()?;
    let note_path = match note_path {
        Some(p) if !p.is_empty() => p,
        _ => return Ok(()),
    };

    let path = resolve_note_path(vault, &note_path);
    if !path.exists() {
        return Ok(());
    }

    let raw = std::fs::read_to_string(&path)?;

    let new_body = if md.is_empty() {
        // 没有批注：若笔记里有区块，移除整个区块（含标记）；否则不动
        if let Some(updated) = strip_block(&raw) {
            updated
        } else {
            return Ok(());
        }
    } else if let Some(updated) = replace_block(&raw, &md) {
        // 已有区块：替换
        updated
    } else {
        // 没有区块：追加
        let mut s = raw;
        if !s.ends_with('\n') {
            s.push('\n');
        }
        s.push('\n');
        s.push_str(&md);
        s
    };

    std::fs::write(&path, new_body)?;
    Ok(())
}

/// 替换笔记中已存在的 ANNOTATIONS_START/END 区块。
/// 返回 Some(new_body) 表示找到并替换；None 表示没找到区块。
fn replace_block(body: &str, new_block: &str) -> Option<String> {
    let start_idx = body.find(MD_BLOCK_START)?;
    let after_start = start_idx + MD_BLOCK_START.len();
    let end_rel = body[after_start..].find(MD_BLOCK_END)?;
    let end_idx = after_start + end_rel + MD_BLOCK_END.len();

    let mut out = String::with_capacity(body.len() + new_block.len());
    out.push_str(&body[..start_idx]);
    out.push_str(new_block);
    if !new_block.ends_with('\n') {
        out.push('\n');
    }
    out.push_str(&body[end_idx..]);
    Some(out)
}

/// 移除笔记中已存在的 ANNOTATIONS_START/END 区块（含标记）。
/// 返回 Some(new_body) 表示找到并移除；None 表示没找到区块。
fn strip_block(body: &str) -> Option<String> {
    let start_idx = body.find(MD_BLOCK_START)?;
    let after_start = start_idx + MD_BLOCK_START.len();
    let end_rel = body[after_start..].find(MD_BLOCK_END)?;
    let end_idx = after_start + end_rel + MD_BLOCK_END.len();

    // 跳过区块后面紧跟的一个换行（如果有），避免留下空行
    let mut consume_end = end_idx;
    if body.get(end_idx..end_idx + 1) == Some("\n") {
        consume_end += 1;
    } else if body.get(end_idx..end_idx + 2) == Some("\r\n") {
        consume_end += 2;
    }

    let mut out = String::with_capacity(body.len());
    out.push_str(&body[..start_idx]);
    out.push_str(&body[consume_end..]);
    Some(out)
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use crate::vault;
    use rusqlite::Connection;

    fn fresh_vault() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        vault::init_at(&path).unwrap();
        db::migrate(&path).unwrap();
        dir
    }

    fn insert_paper(conn: &Connection, id: &str, title: &str) {
        let now = chrono::Local::now().timestamp_millis();
        conn.execute(
            "INSERT INTO papers (id, title, year, venue, doi, abstract_text, status,
                                 pdf_path, note_path, created_at, updated_at)
             VALUES (?1, ?2, NULL, '', '', '', 'unread', '', '', ?3, ?3)",
            params![id, title, now],
        )
        .unwrap();
    }

    fn insert_paper_with_note(conn: &Connection, id: &str, title: &str, note_path: &str) {
        let now = chrono::Local::now().timestamp_millis();
        conn.execute(
            "INSERT INTO papers (id, title, year, venue, doi, abstract_text, status,
                                 pdf_path, note_path, created_at, updated_at)
             VALUES (?1, ?2, NULL, '', '', '', 'unread', '', ?3, ?4, ?4)",
            params![id, title, note_path, now],
        )
        .unwrap();
    }

    #[test]
    fn test_create_annotation() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "p1", "Paper 1");

        let ann = create(
            dir.path(),
            "p1",
            "highlight",
            Some(3),
            Some(r#"{"x":1,"y":2,"w":3,"h":4}"#),
            Some("#ffeb3b"),
            Some("selected text"),
            Some("my comment"),
        )
        .unwrap();

        assert!(!ann.id.is_empty());
        assert_eq!(ann.paper_id, "p1");
        assert_eq!(ann.kind, "highlight");
        assert_eq!(ann.page, Some(3));
        assert_eq!(ann.color.as_deref(), Some("#ffeb3b"));
        assert_eq!(ann.text.as_deref(), Some("selected text"));
        assert_eq!(ann.comment.as_deref(), Some("my comment"));
        assert!(ann.modified_at.is_none());
        assert!(ann.created_at > 0);

        // DB 中应有 1 条
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM annotations WHERE paper_id = 'p1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn test_list_by_paper() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "p1", "Paper 1");

        // 创建 3 条，page 顺序故意打乱
        let _a1 = create(dir.path(), "p1", "highlight", Some(2), None, None, None, None).unwrap();
        // 让 created_at 拉开差距
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _a2 = create(dir.path(), "p1", "highlight", Some(1), None, None, None, None).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _a3 = create(dir.path(), "p1", "highlight", Some(1), None, None, None, None).unwrap();

        let list = list_by_paper(dir.path(), "p1").unwrap();
        assert_eq!(list.len(), 3);
        // page ASC, created_at ASC
        assert_eq!(list[0].page, Some(1));
        assert_eq!(list[1].page, Some(1));
        assert_eq!(list[2].page, Some(2));
        // 同 page 的两条按 created_at 升序
        assert!(list[0].created_at <= list[1].created_at);
    }

    #[test]
    fn test_update_annotation() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "p1", "Paper 1");

        let ann = create(
            dir.path(),
            "p1",
            "highlight",
            Some(1),
            None,
            Some("#fff"),
            Some("old text"),
            Some("old comment"),
        )
        .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));
        let updated = update(
            dir.path(),
            &ann.id,
            None,
            None,
            Some("new comment"),
            None,
        )
        .unwrap();

        assert_eq!(updated.comment.as_deref(), Some("new comment"));
        // 未更新的字段保持原值
        assert_eq!(updated.text.as_deref(), Some("old text"));
        assert_eq!(updated.color.as_deref(), Some("#fff"));
        // modified_at 应被设置
        assert!(updated.modified_at.is_some(), "modified_at should be set");
        assert!(updated.modified_at.unwrap() > ann.created_at);

        // DB 中也确认
        let db_comment: String = conn
            .query_row(
                "SELECT comment FROM annotations WHERE id = ?1",
                params![ann.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(db_comment, "new comment");
    }

    #[test]
    fn test_update_annotation_not_found() {
        let dir = fresh_vault();
        let err = update(
            dir.path(),
            "non-existent-id",
            Some("#000"),
            None,
            None,
            None,
        )
        .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)), "got {err:?}");
    }

    #[test]
    fn test_delete_annotation() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "p1", "Paper 1");

        let ann = create(dir.path(), "p1", "highlight", Some(1), None, None, None, None).unwrap();
        let list = list_by_paper(dir.path(), "p1").unwrap();
        assert_eq!(list.len(), 1);

        delete(dir.path(), &ann.id).unwrap();

        let list = list_by_paper(dir.path(), "p1").unwrap();
        assert!(list.is_empty());

        // DB 中也确认
        let n: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM annotations WHERE id = ?1",
                params![ann.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn test_export_to_markdown_empty() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "p1", "Paper 1");

        let md = export_to_markdown(dir.path(), "p1").unwrap();
        assert!(md.is_empty(), "expected empty, got: {md}");
    }

    #[test]
    fn test_export_to_markdown_with_data() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        insert_paper(&conn, "p1", "Paper 1");

        create(
            dir.path(),
            "p1",
            "highlight",
            Some(5),
            None,
            Some("#ffeb3b"),
            Some("important text"),
            Some("a comment"),
        )
        .unwrap();
        create(
            dir.path(),
            "p1",
            "note",
            Some(5),
            None,
            None,
            None,
            Some("standalone note"),
        )
        .unwrap();

        let md = export_to_markdown(dir.path(), "p1").unwrap();
        assert!(md.contains(MD_BLOCK_START), "missing start marker");
        assert!(md.contains(MD_BLOCK_END), "missing end marker");
        assert!(md.contains("## 批注"));
        assert!(md.contains("### 第 5 页"));
        assert!(md.contains("[#ffeb3b]"));
        assert!(md.contains("important text"));
        assert!(md.contains("💬 a comment"));
        assert!(md.contains("💬 standalone note"));
        // 多条批注之间应有分隔
        assert!(md.contains("---"));
    }

    #[test]
    fn test_sync_to_note_no_note() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        // paper 没有 note_path
        insert_paper(&conn, "p1", "Paper 1");
        create(dir.path(), "p1", "highlight", Some(1), None, None, None, None).unwrap();

        // 应直接 Ok，不写文件
        sync_to_note(dir.path(), "p1").unwrap();

        // 没有笔记文件被创建（note_path 为空）
        let note_path: String = conn
            .query_row(
                "SELECT note_path FROM papers WHERE id = 'p1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(note_path.is_empty());
    }

    #[test]
    fn test_sync_to_note_note_not_exist() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        // note_path 指向不存在的文件
        insert_paper_with_note(&conn, "p1", "Paper 1", "notes/papers/p1.md");
        create(dir.path(), "p1", "highlight", Some(1), None, None, None, None).unwrap();

        // 应 Ok（无笔记可同步）
        sync_to_note(dir.path(), "p1").unwrap();
    }

    #[test]
    fn test_sync_to_note_insert() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        let note_rel = "notes/papers/p1.md";
        insert_paper_with_note(&conn, "p1", "Paper 1", note_rel);

        let abs = dir.path().join(note_rel);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        let original = "# Paper 1\n\nSome hand-written notes.\n";
        std::fs::write(&abs, original).unwrap();

        create(
            dir.path(),
            "p1",
            "highlight",
            Some(2),
            None,
            Some("#ff0"),
            Some("highlighted"),
            Some("note text"),
        )
        .unwrap();

        sync_to_note(dir.path(), "p1").unwrap();

        let after = std::fs::read_to_string(&abs).unwrap();
        // 原内容保留
        assert!(after.contains("Some hand-written notes."), "original lost: {after}");
        // 批注区块被追加
        assert!(after.contains(MD_BLOCK_START), "missing start marker: {after}");
        assert!(after.contains(MD_BLOCK_END), "missing end marker: {after}");
        assert!(after.contains("### 第 2 页"));
        assert!(after.contains("highlighted"));
        assert!(after.contains("💬 note text"));
    }

    #[test]
    fn test_sync_to_note_replace() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        let note_rel = "notes/papers/p2.md";
        insert_paper_with_note(&conn, "p2", "Paper 2", note_rel);

        let abs = dir.path().join(note_rel);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        let original = format!(
            "# Paper 2\n\n{MD_BLOCK_START}\n## 批注\n\n### 第 1 页\n\n> **[old]** old text\n\n---\n\n{MD_BLOCK_END}\n\nfooter\n"
        );
        std::fs::write(&abs, &original).unwrap();

        create(
            dir.path(),
            "p2",
            "highlight",
            Some(7),
            None,
            Some("#0f0"),
            Some("new text"),
            None,
        )
        .unwrap();

        sync_to_note(dir.path(), "p2").unwrap();

        let after = std::fs::read_to_string(&abs).unwrap();
        // 旧批注应被替换
        assert!(!after.contains("old text"), "old block not replaced: {after}");
        assert!(!after.contains("[old]"));
        // 新批注存在
        assert!(after.contains("### 第 7 页"));
        assert!(after.contains("new text"));
        assert!(after.contains("[#0f0]"));
        // 区块外的内容保留
        assert!(after.contains("# Paper 2"));
        assert!(after.contains("footer"));
        // 只有一个 START 标记（没有重复）
        assert_eq!(
            after.matches(MD_BLOCK_START).count(),
            1,
            "should have exactly 1 start marker: {after}"
        );
        assert_eq!(
            after.matches(MD_BLOCK_END).count(),
            1,
            "should have exactly 1 end marker: {after}"
        );
    }

    #[test]
    fn test_sync_to_note_empty_annotations_removes_block() {
        let dir = fresh_vault();
        let conn = db::open(dir.path()).unwrap();
        let note_rel = "notes/papers/p3.md";
        insert_paper_with_note(&conn, "p3", "Paper 3", note_rel);

        let abs = dir.path().join(note_rel);
        std::fs::create_dir_all(abs.parent().unwrap()).unwrap();
        let original = format!(
            "# Paper 3\n\nintro\n\n{MD_BLOCK_START}\n## 批注\n\n### 第 1 页\n\n> **[old]** old text\n\n---\n\n{MD_BLOCK_END}\n\noutro\n"
        );
        std::fs::write(&abs, &original).unwrap();

        // 没有任何批注
        sync_to_note(dir.path(), "p3").unwrap();

        let after = std::fs::read_to_string(&abs).unwrap();
        assert!(!after.contains(MD_BLOCK_START), "block should be removed: {after}");
        assert!(!after.contains(MD_BLOCK_END));
        assert!(!after.contains("old text"));
        // 区块外的内容保留
        assert!(after.contains("# Paper 3"));
        assert!(after.contains("intro"));
        assert!(after.contains("outro"));
    }
}
