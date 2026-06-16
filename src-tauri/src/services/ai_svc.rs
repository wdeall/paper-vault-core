//! AI 服务：取 preset → 渲染模板 → 调客户端 → 解析输出

use crate::ai::{client, template};
use crate::db;
use crate::error::AppResult;
use crate::services::preset;
use crate::types::{AIProviderConfig, AIResult};
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;

pub fn get_provider(vault: &Path) -> AppResult<AIProviderConfig> {
    let conn = db::open(vault)?;
    let cfg = conn
        .query_row(
            "SELECT base_url, api_key, model FROM ai_provider_config WHERE id = 'default'",
            [],
            |r| {
                Ok(AIProviderConfig {
                    base_url: r.get(0)?,
                    api_key: r.get(1)?,
                    model: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(cfg.unwrap_or_default())
}

pub fn update_provider(vault: &Path, patch: &AIProviderConfig) -> AppResult<AIProviderConfig> {
    let conn = db::open(vault)?;
    let now = chrono::Local::now().timestamp_millis();
    conn.execute(
        "INSERT INTO ai_provider_config (id, base_url, api_key, model, updated_at)
         VALUES ('default', ?1, ?2, ?3, ?4)
         ON CONFLICT(id) DO UPDATE SET
           base_url = excluded.base_url, api_key = excluded.api_key,
           model = excluded.model, updated_at = excluded.updated_at",
        params![patch.base_url, patch.api_key, patch.model, now],
    )?;
    get_provider(vault)
}

/// 跑 AI：preset_id (builtin:xxx 或 user:xxx) + paper_id 可选 + 直接输入。
pub async fn run(
    vault: &Path,
    preset_id: &str,
    paper_id: Option<&str>,
    direct_input: Option<&str>,
) -> AppResult<AIResult> {
    let p = preset::get_effective(vault, preset_id)?;

    // 收集 vars
    let mut vars: HashMap<String, String> = HashMap::new();
    if let Some(pid) = paper_id {
        if let Some(paper) = crate::services::paper::load_paper(vault, pid)? {
            vars.insert("title".into(), paper.title.clone());
            vars.insert("authors".into(), paper.authors.join(", "));
            vars.insert("year".into(), paper.year.map(|y| y.to_string()).unwrap_or_default());
            vars.insert("venue".into(), paper.venue.clone());
            vars.insert("doi".into(), paper.doi.clone());
            vars.insert("abstract".into(), paper.abstract_text.clone());
            vars.insert("keywords".into(), paper.keywords.join(", "));
            if !paper.pdf_path.is_empty() {
                let pp = std::path::Path::new(&paper.pdf_path);
                let file_name = pp
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                vars.insert("file_name".into(), file_name);
                if pp.exists() {
                    if let Ok(basic) = crate::pdf::extract_basic(pp) {
                        vars.insert("first_page_text".into(), basic.first_page_text);
                        vars.insert("page_count".into(), basic.page_count.to_string());
                    }
                    let pages = crate::pdf::extract_pages(pp);
                    let text: String = pages
                        .iter()
                        .take(20)
                        .map(|(_, t)| t.as_str())
                        .collect::<Vec<_>>()
                        .join("\n\n");
                    vars.insert("pdf_text".into(), text.chars().take(6000).collect());
                }
            }
        }
    }
    if let Some(d) = direct_input {
        vars.insert("input".into(), d.to_string());
    }

    let user_msg = template::render(&p.user_template, &vars)?;
    let messages = vec![
        client::ChatMessage {
            role: "system".into(),
            content: p.system_prompt.clone(),
        },
        client::ChatMessage {
            role: "user".into(),
            content: user_msg,
        },
    ];

    let cfg = get_provider(vault)?;
    let json_mode = p.output_format == "json";
    let raw = client::chat(&cfg, messages, json_mode).await?;

    let mut parsed = None;
    if json_mode {
        if let Some(start) = raw.find('{') {
            if let Some(end) = raw.rfind('}') {
                let json_str = &raw[start..=end];
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(json_str) {
                    parsed = Some(v);
                }
            }
        }
    }

    Ok(AIResult {
        raw: raw.clone(),
        parsed,
        markdown: raw,
    })
}
