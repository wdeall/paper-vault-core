//! AI 服务：取 preset → 渲染模板 → 调客户端 → 解析输出

use crate::ai::{client, template};
use crate::db;
use crate::error::AppResult;
use crate::services::preset;
use crate::types::{AIProviderConfig, AIResult, ChatMessageInput};
use rusqlite::{params, OptionalExtension};
use std::collections::HashMap;
use std::path::Path;

/// 统一身份前缀：所有 AI 调用（preset + chat）都会拼接此提示
const BASE_IDENTITY: &str = "你是 PaperVault 的论文助手,专注于学术论文的阅读、理解、总结、元数据管理与复现规划。回答需准确、简洁、结构化,使用 Markdown 格式。";

/// 单块最大字符数。超过此长度的 PDF 文本会按块切分做 map-reduce 摘要。
/// 选取 25000 字（约 8000-12000 tokens）兼顾上下文窗口与单块信息密度。
const CHUNK_SIZE: usize = 25000;

/// 把全文按章节/页分割成若干块，每块不超过 CHUNK_SIZE 字符。
/// 切分策略：优先按 form feed（页分隔符）切；单页超长再按段落切。
fn chunk_text(full: &str) -> Vec<String> {
    if full.chars().count() <= CHUNK_SIZE {
        return vec![full.to_string()];
    }
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_len = 0usize;
    // 按 form feed（页分隔）拆，再按段落拆
    for page in full.split('\x0c') {
        if page.is_empty() {
            continue;
        }
        let page_len = page.chars().count();
        if current_len + page_len > CHUNK_SIZE && !current.is_empty() {
            chunks.push(std::mem::take(&mut current));
            current_len = 0;
        }
        if page_len > CHUNK_SIZE {
            // 单页超长，按段落再拆
            for para in page.split("\n\n") {
                let plen = para.chars().count();
                if current_len + plen > CHUNK_SIZE && !current.is_empty() {
                    chunks.push(std::mem::take(&mut current));
                    current_len = 0;
                }
                current.push_str(para);
                current.push_str("\n\n");
                current_len += plen + 2;
            }
        } else {
            current.push_str(page);
            current.push('\x0c');
            current_len += page_len + 1;
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// map-reduce 全文摘要：每块独立摘要 → 合并最终摘要。
async fn summarize_full_text(
    cfg: &AIProviderConfig,
    system_prompt: &str,
    user_template: &str,
    vars: &HashMap<String, String>,
    full_text: &str,
) -> AppResult<String> {
    let chunks = chunk_text(full_text);
    if chunks.len() <= 1 {
        // 单块直接走原模板
        let mut v = vars.clone();
        v.insert("pdf_text".into(), full_text.to_string());
        let user_msg = crate::ai::template::render(user_template, &v)?;
        let messages = vec![
            crate::ai::client::ChatMessage {
                role: "system".into(),
                content: format!("{BASE_IDENTITY}\n\n{}", system_prompt),
            },
            crate::ai::client::ChatMessage {
                role: "user".into(),
                content: user_msg,
            },
        ];
        return crate::ai::client::chat(cfg, messages, false).await;
    }

    log::info!("全文分 {} 块做 map-reduce 摘要", chunks.len());

    // map：每块独立摘要
    let mut partial_summaries: Vec<String> = Vec::with_capacity(chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        let mut v = vars.clone();
        v.insert("pdf_text".into(), chunk.clone());
        v.insert("chunk_index".into(), (i + 1).to_string());
        v.insert("chunk_total".into(), chunks.len().to_string());
        let user_msg = crate::ai::template::render(
            &format!("（这是论文的第 {{chunk_index}}/{{chunk_total}} 部分）\n{user_template}"),
            &v,
        )?;
        let messages = vec![
            crate::ai::client::ChatMessage {
                role: "system".into(),
                content: format!(
                    "{BASE_IDENTITY}\n\n{}你正在阅读论文的一个分块（第 {}/{} 部分），请只针对该部分做摘要，标注分块序号。",
                    system_prompt, i + 1, chunks.len()
                ),
            },
            crate::ai::client::ChatMessage {
                role: "user".into(),
                content: user_msg,
            },
        ];
        let partial = crate::ai::client::chat(cfg, messages, false).await?;
        partial_summaries.push(partial);
    }

    // reduce：合并所有分块摘要
    let combined = partial_summaries
        .iter()
        .enumerate()
        .map(|(i, s)| format!("## 分块 {} 摘要\n\n{}", i + 1, s))
        .collect::<Vec<_>>()
        .join("\n\n---\n\n");

    let reduce_user = format!(
        "以下是论文分块摘要的合并结果。请整合为一份完整、连贯、结构化的最终摘要，去除分块编号，合并重复信息，保留各部分要点：\n\n{}",
        combined
    );
    let messages = vec![
        crate::ai::client::ChatMessage {
            role: "system".into(),
            content: format!(
                "{BASE_IDENTITY}\n\n{}你正在把分块摘要合并为最终摘要。保持结构化，去除分块编号，合并重复内容。",
                system_prompt
            ),
        },
        crate::ai::client::ChatMessage {
            role: "user".into(),
            content: reduce_user,
        },
    ];
    crate::ai::client::chat(cfg, messages, false).await
}

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
                // 兼容绝对路径与相对 vault 的路径
                let pp = if std::path::Path::new(&paper.pdf_path).is_absolute() {
                    std::path::PathBuf::from(&paper.pdf_path)
                } else {
                    vault.join(&paper.pdf_path)
                };
                let file_name = pp
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                vars.insert("file_name".into(), file_name);
                if pp.exists() {
                    if let Ok(basic) = crate::pdf::extract_basic(&pp) {
                        vars.insert("first_page_text".into(), basic.first_page_text);
                        vars.insert("page_count".into(), basic.page_count.to_string());
                    } else {
                        log::warn!("PDF 首页提取失败: {}", pp.display());
                    }
                    let pages = crate::pdf::extract_pages(&pp);
                    if pages.is_empty() {
                        log::warn!("PDF 分页提取为空: {}", pp.display());
                        vars.insert("pdf_text".into(), String::new());
                    } else {
                        // 全文拼接（不再 take(40) 页 / take(20000) 字截断）。
                        // 长论文由 summarize_full_text 做 map-reduce 分块摘要。
                        let text: String = pages
                            .iter()
                            .map(|(_, t)| t.as_str())
                            .collect::<Vec<_>>()
                            .join("\x0c");
                        vars.insert("pdf_text".into(), text);
                    }
                } else {
                    log::warn!("PDF 文件不存在: {}", pp.display());
                    vars.insert("pdf_text".into(), String::new());
                }
            }
        }
    }
    if let Some(d) = direct_input {
        vars.insert("input".into(), d.to_string());
    }

    let cfg = get_provider(vault)?;
    let json_mode = p.output_format == "json";

    // 对需要全文阅读的 preset（skill=pdf 且非元数据提取）走 map-reduce 分块摘要，
    // 保证长论文也能被完整阅读。其他 preset 走单次调用。
    let needs_full_text = p.skill == "pdf"
        && p.bound_action != "metadata_from_pdf"
        && vars.contains_key("pdf_text");

    let raw = if needs_full_text {
        let full_text = vars.get("pdf_text").cloned().unwrap_or_default();
        if full_text.is_empty() {
            // 无 PDF 文本，降级到单次调用（template 里 pdf_text 为空）
            let user_msg = template::render(&p.user_template, &vars)?;
            let messages = vec![
                client::ChatMessage {
                    role: "system".into(),
                    content: format!("{BASE_IDENTITY}\n\n{}", p.system_prompt),
                },
                client::ChatMessage {
                    role: "user".into(),
                    content: user_msg,
                },
            ];
            client::chat(&cfg, messages, json_mode).await?
        } else {
            summarize_full_text(&cfg, &p.system_prompt, &p.user_template, &vars, &full_text).await?
        }
    } else {
        let user_msg = template::render(&p.user_template, &vars)?;
        let messages = vec![
            client::ChatMessage {
                role: "system".into(),
                content: format!("{BASE_IDENTITY}\n\n{}", p.system_prompt),
            },
            client::ChatMessage {
                role: "user".into(),
                content: user_msg,
            },
        ];
        client::chat(&cfg, messages, json_mode).await?
    };

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

/// AI 对话：多轮对话，以论文元数据作为上下文。
pub async fn chat(
    vault: &Path,
    paper_id: &str,
    input: &str,
    history: &[ChatMessageInput],
) -> AppResult<String> {
    let cfg = get_provider(vault)?;

    let mut messages = vec![client::ChatMessage {
        role: "system".into(),
        content: format!(
            "{BASE_IDENTITY}\n\n当前正在与用户讨论论文（ID: {paper_id}）。用户可能基于该论文提问。"
        ),
    }];

    // 加载论文元数据作为上下文
    if let Some(paper) = crate::services::paper::load_paper(vault, paper_id)? {
        messages.push(client::ChatMessage {
            role: "system".into(),
            content: format!(
                "论文上下文：\n标题：{}\n作者：{}\n年份：{}\nDOI：{}\n摘要：{}",
                paper.title,
                paper.authors.join(", "),
                paper.year.map(|y| y.to_string()).unwrap_or_default(),
                paper.doi,
                paper.abstract_text
            ),
        });
    }

    // 加历史消息
    for m in history {
        messages.push(client::ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        });
    }

    // 加当前输入
    messages.push(client::ChatMessage {
        role: "user".into(),
        content: input.to_string(),
    });

    let raw = client::chat(&cfg, messages, false).await?;
    Ok(raw)
}
