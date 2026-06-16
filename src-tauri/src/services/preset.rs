//! AI 预设 CRUD 服务

use crate::db;
use crate::error::{AppError, AppResult};
use crate::types::AISkillPreset;
use rusqlite::{params, OptionalExtension};

fn row_to_preset(row: &rusqlite::Row<'_>) -> rusqlite::Result<AISkillPreset> {
    Ok(AISkillPreset {
        id: row.get("id")?,
        name: row.get("name")?,
        bound_action: row.get("bound_action")?,
        skill: row.get("skill")?,
        system_prompt: row.get("system_prompt")?,
        user_template: row.get("user_template")?,
        output_format: row.get("output_format")?,
        auto_write: row.get::<_, i32>("auto_write")? != 0,
        is_builtin: row.get::<_, i32>("is_builtin")? != 0,
        updated_at: row.get("updated_at")?,
    })
}

pub fn list(vault: &std::path::Path) -> AppResult<Vec<AISkillPreset>> {
    let conn = db::open(vault)?;
    let mut stmt = conn.prepare("SELECT * FROM ai_skill_presets ORDER BY bound_action, is_builtin DESC")?;
    let rows = stmt.query_map([], row_to_preset)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn get(vault: &std::path::Path, id: &str) -> AppResult<AISkillPreset> {
    let conn = db::open(vault)?;
    conn.query_row(
        "SELECT * FROM ai_skill_presets WHERE id = ?1",
        params![id],
        row_to_preset,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("预设 {id} 不存在")))
}

pub fn get_effective(vault: &std::path::Path, id_or_action: &str) -> AppResult<AISkillPreset> {
    let conn = db::open(vault)?;
    conn.query_row(
        "SELECT * FROM ai_skill_presets
         WHERE id = ?1 OR bound_action = ?1
         ORDER BY is_builtin ASC
         LIMIT 1",
        params![id_or_action],
        row_to_preset,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("预设 {id_or_action} 不存在")))
}

/// 用户更新 → 写入同 bound_action 的非 builtin 行（保留 builtin 不动）。
pub fn update_user(
    vault: &std::path::Path,
    builtin_id: &str,
    p: &AISkillPreset,
) -> AppResult<AISkillPreset> {
    let conn = db::open(vault)?;
    // 找到原 builtin bound_action
    let builtin = get(vault, builtin_id)?;
    let now = chrono::Local::now().timestamp_millis();

    // 删除已有同 bound_action 的非 builtin 行
    conn.execute(
        "DELETE FROM ai_skill_presets WHERE bound_action = ?1 AND is_builtin = 0",
        params![builtin.bound_action],
    )?;

    let user_id = format!("user:{}", builtin.bound_action);
    conn.execute(
        "INSERT INTO ai_skill_presets
         (id, name, bound_action, skill, system_prompt, user_template, output_format, auto_write, is_builtin, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 0, ?9)",
        params![
            user_id, p.name, builtin.bound_action, p.skill,
            p.system_prompt, p.user_template, p.output_format,
            p.auto_write as i32, now,
        ],
    )?;
    get(vault, &user_id)
}

pub fn reset(vault: &std::path::Path, builtin_id: &str) -> AppResult<AISkillPreset> {
    let conn = db::open(vault)?;
    let builtin = get(vault, builtin_id)?;
    conn.execute(
        "DELETE FROM ai_skill_presets WHERE bound_action = ?1 AND is_builtin = 0",
        params![builtin.bound_action],
    )?;
    get(vault, builtin_id)
}

pub fn seed_builtins_if_empty(vault: &std::path::Path) -> AppResult<()> {
    let conn = db::open(vault)?;
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM ai_skill_presets WHERE is_builtin = 1",
        [],
        |r| r.get(0),
    )?;
    if n > 0 {
        return Ok(());
    }
    let now = chrono::Local::now().timestamp_millis();
    for p in crate::ai::presets::builtin_presets(now) {
        conn.execute(
            "INSERT OR IGNORE INTO ai_skill_presets
             (id, name, bound_action, skill, system_prompt, user_template, output_format, auto_write, is_builtin, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                p.id, p.name, p.bound_action, p.skill,
                p.system_prompt, p.user_template, p.output_format,
                p.auto_write as i32, p.is_builtin as i32, p.updated_at,
            ],
        )?;
    }
    Ok(())
}
