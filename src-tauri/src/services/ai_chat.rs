//! AI 对话历史持久化 + 流式推送。
//!
//! agent 风格侧边栏的后端：
//! - 会话 CRUD（按 paper_id 分组）
//! - 消息持久化（含 thinking/context/tool_calls 字段）
//! - 流式响应：通过 Tauri event 推送 delta 到前端
//! - 快捷功能（preset）在对话中执行：展示思考过程/上下文/发送内容

use crate::ai::client;
use crate::db;
use crate::error::{AppError, AppResult};
use crate::services::ai_svc;
use crate::types::{AIConversation, AIMessage, AIProviderConfig};
use rusqlite::{params, OptionalExtension};
use std::path::Path;
use tauri::{AppHandle, Emitter};

const BASE_IDENTITY: &str = "你是 PaperVault 的论文助手,专注于学术论文的阅读、理解、总结、元数据管理与复现规划。回答需准确、简洁、结构化,使用 Markdown 格式。";

/// 历史消息总字符数上限。超过此值时,对旧消息做 LLM 总结压缩。
/// 选取 50000 字（约 15000-20000 tokens），兼顾上下文窗口与信息密度。
const MAX_HISTORY_CHARS: usize = 50000;
/// 触发总结后,保留最近 N 条消息原文（不被总结），确保近期上下文完整。
const KEEP_RECENT_COUNT: usize = 6;

/// 上下文管理：根据历史消息总长度决定是否触发总结压缩。
/// - 未超限：直接返回全部历史
/// - 超限且无缓存摘要：调用 LLM 总结旧消息，缓存到 conversation 表
/// - 超限且有缓存摘要：复用缓存（若覆盖范围未变）
///
/// 返回 (历史消息列表, 是否刚做了总结)
async fn build_context_aware_history(
    vault: &Path,
    conv: &AIConversation,
) -> AppResult<(Vec<client::ChatMessage>, bool)> {
    // 加载该会话全部消息（按时间升序）
    let all: Vec<(String, String, String)> = {
        let conn = db::open(vault)?;
        let mut stmt = conn.prepare(
            "SELECT id, role, content FROM ai_messages
             WHERE conversation_id = ?1 ORDER BY created_at ASC",
        )?;
        let rows = stmt.query_map(params![conv.id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        out
    };

    if all.is_empty() {
        return Ok((Vec::new(), false));
    }

    let total_chars: usize = all.iter().map(|(_, _, c)| c.chars().count()).sum();
    if total_chars <= MAX_HISTORY_CHARS {
        // 未超限,全用原文
        let msgs = all
            .into_iter()
            .map(|(_, role, content)| client::ChatMessage { role, content })
            .collect();
        return Ok((msgs, false));
    }

    // 超限：保留最近 KEEP_RECENT_COUNT 条原文,其余做总结
    let split = all.len().saturating_sub(KEEP_RECENT_COUNT);
    if split == 0 {
        // 全部都保留（消息数 < KEEP_RECENT_COUNT），无需总结
        let msgs = all
            .into_iter()
            .map(|(_, role, content)| client::ChatMessage { role, content })
            .collect();
        return Ok((msgs, false));
    }

    let old_part = &all[..split];
    let recent_part = &all[split..];

    // 检查缓存是否可用：summary_up_to 等于 old_part 最后一条消息的 id
    let old_last_id = old_part.last().map(|(id, _, _)| id.as_str()).unwrap_or("");
    let old_chars: usize = old_part.iter().map(|(_, _, c)| c.chars().count()).sum();

    let mut summary = conv.summary.clone();
    let mut just_summarized = false;

    let need_resummarize = conv.summary_up_to.as_deref() != Some(old_last_id)
        || conv.summary_chars != old_chars as i64
        || summary.is_none();

    if need_resummarize {
        // 调用 LLM 总结 old_part
        let old_text = old_part
            .iter()
            .map(|(id, role, content)| format!("[{}] (id={})\n{}", role, id, content))
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");

        let summarize_messages = vec![
            client::ChatMessage {
                role: "system".into(),
                content: format!(
                    "{BASE_IDENTITY}\n\n你正在压缩对话历史。请把以下对话总结为一份结构化的 Markdown 摘要，保留：\n1. 用户的核心问题与意图\n2. AI 回答的关键结论与论据\n3. 涉及的论文要点\n4. 任何未解决的问题或后续计划\n\n删除寒暄、重复内容、冗长引用。摘要应简洁但信息完整，便于后续对话引用。"
                ),
            },
            client::ChatMessage {
                role: "user".into(),
                content: format!("需要总结的对话历史（共 {} 条消息，{} 字）：\n\n{}", old_part.len(), old_chars, old_text),
            },
        ];

        let cfg = ai_svc::get_provider(vault)?;
        summary = Some(client::chat(&cfg, summarize_messages, false).await?);
        just_summarized = true;

        // 缓存到 DB
        {
            let conn = db::open(vault)?;
            conn.execute(
                "UPDATE ai_conversations
                 SET summary = ?2, summary_up_to = ?3, summary_chars = ?4
                 WHERE id = ?1",
                params![conv.id, summary, old_last_id, old_chars as i64],
            )?;
        }
        log::info!(
            "会话 {} 触发上下文总结：{} 条消息 / {} 字 → 摘要 {} 字",
            conv.id,
            old_part.len(),
            old_chars,
            summary.as_ref().map(|s| s.chars().count()).unwrap_or(0)
        );
    }

    // 构造最终历史：[摘要 system 消息] + [最近 N 条原文]
    let mut msgs = Vec::with_capacity(1 + recent_part.len());
    if let Some(s) = &summary {
        msgs.push(client::ChatMessage {
            role: "system".into(),
            content: format!("以下是之前对话的压缩摘要（覆盖 {} 条消息，{} 字）：\n\n{}", old_part.len(), old_chars, s),
        });
    }
    for (_, role, content) in recent_part {
        msgs.push(client::ChatMessage {
            role: role.clone(),
            content: content.clone(),
        });
    }

    Ok((msgs, just_summarized))
}

fn row_to_conversation(row: &rusqlite::Row<'_>) -> rusqlite::Result<AIConversation> {
    Ok(AIConversation {
        id: row.get("id")?,
        paper_id: row.get("paper_id")?,
        title: row.get("title")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        summary: row.get("summary")?,
        summary_up_to: row.get("summary_up_to")?,
        summary_chars: row.get::<_, Option<i64>>("summary_chars")?.unwrap_or(0),
    })
}

fn row_to_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<AIMessage> {
    Ok(AIMessage {
        id: row.get("id")?,
        conversation_id: row.get("conversation_id")?,
        role: row.get("role")?,
        content: row.get("content")?,
        thinking: row.get("thinking")?,
        context: row.get("context")?,
        tool_calls: row.get("tool_calls")?,
        preset_id: row.get("preset_id")?,
        created_at: row.get("created_at")?,
    })
}

/// 列出会话（按 paper_id 过滤，最近更新在前）。
pub fn list_conversations(vault: &Path, paper_id: Option<&str>) -> AppResult<Vec<AIConversation>> {
    let conn = db::open(vault)?;
    let sql = if paper_id.is_some() {
        "SELECT id, paper_id, title, created_at, updated_at, summary, summary_up_to, summary_chars
         FROM ai_conversations WHERE paper_id = ?1
         ORDER BY updated_at DESC"
    } else {
        "SELECT id, paper_id, title, created_at, updated_at, summary, summary_up_to, summary_chars
         FROM ai_conversations ORDER BY updated_at DESC"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = if paper_id.is_some() {
        stmt.query_map(params![paper_id], row_to_conversation)?
    } else {
        stmt.query_map([], row_to_conversation)?
    };
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// 创建新会话。
pub fn create_conversation(
    vault: &Path,
    paper_id: Option<&str>,
    title: &str,
) -> AppResult<AIConversation> {
    let conn = db::open(vault)?;
    let id = uuid::Uuid::new_v4().simple().to_string();
    let now = chrono::Local::now().timestamp_millis();
    let title = if title.is_empty() {
        "新对话".to_string()
    } else {
        title.to_string()
    };
    conn.execute(
        "INSERT INTO ai_conversations (id, paper_id, title, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, paper_id, title, now, now],
    )?;
    Ok(AIConversation {
        id,
        paper_id: paper_id.map(|s| s.to_string()),
        title,
        created_at: now,
        updated_at: now,
        summary: None,
        summary_up_to: None,
        summary_chars: 0,
    })
}

/// 删除会话（CASCADE 删除其下所有消息）。
pub fn delete_conversation(vault: &Path, id: &str) -> AppResult<()> {
    let conn = db::open(vault)?;
    conn.execute("DELETE FROM ai_conversations WHERE id = ?1", params![id])?;
    Ok(())
}

/// 重命名会话。
pub fn rename_conversation(vault: &Path, id: &str, title: &str) -> AppResult<AIConversation> {
    let conn = db::open(vault)?;
    let now = chrono::Local::now().timestamp_millis();
    conn.execute(
        "UPDATE ai_conversations SET title = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, title, now],
    )?;
    conn.query_row(
        "SELECT id, paper_id, title, created_at, updated_at, summary, summary_up_to, summary_chars FROM ai_conversations WHERE id = ?1",
        params![id],
        row_to_conversation,
    )
    .optional()?
    .ok_or_else(|| AppError::NotFound(format!("会话 {id} 不存在")))
}

/// 列出会话的所有消息（按时间升序）。
pub fn list_messages(vault: &Path, conversation_id: &str) -> AppResult<Vec<AIMessage>> {
    let conn = db::open(vault)?;
    let mut stmt = conn.prepare(
        "SELECT id, conversation_id, role, content, thinking, context, tool_calls, preset_id, created_at
         FROM ai_messages WHERE conversation_id = ?1 ORDER BY created_at ASC",
    )?;
    let rows = stmt.query_map(params![conversation_id], row_to_message)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// 插入一条消息。
fn insert_message(
    conn: &rusqlite::Connection,
    conversation_id: &str,
    role: &str,
    content: &str,
    thinking: Option<&str>,
    context: Option<&str>,
    tool_calls: Option<&str>,
    preset_id: Option<&str>,
) -> AppResult<AIMessage> {
    let id = uuid::Uuid::new_v4().simple().to_string();
    let now = chrono::Local::now().timestamp_millis();
    conn.execute(
        "INSERT INTO ai_messages
            (id, conversation_id, role, content, thinking, context, tool_calls, preset_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, conversation_id, role, content, thinking, context, tool_calls, preset_id, now],
    )?;
    // 更新会话 updated_at
    conn.execute(
        "UPDATE ai_conversations SET updated_at = ?2 WHERE id = ?1",
        params![conversation_id, now],
    )?;
    Ok(AIMessage {
        id,
        conversation_id: conversation_id.to_string(),
        role: role.to_string(),
        content: content.to_string(),
        thinking: thinking.map(|s| s.to_string()),
        context: context.map(|s| s.to_string()),
        tool_calls: tool_calls.map(|s| s.to_string()),
        preset_id: preset_id.map(|s| s.to_string()),
        created_at: now,
    })
}

/// 手动触发对话历史总结。
/// 强制对所有历史消息（除最近 KEEP_RECENT_COUNT 条）做 LLM 总结，并缓存。
pub async fn summarize_now(vault: &Path, conversation_id: &str) -> AppResult<AIConversation> {
    let conv: AIConversation = {
        let conn = db::open(vault)?;
        conn.query_row(
            "SELECT id, paper_id, title, created_at, updated_at, summary, summary_up_to, summary_chars FROM ai_conversations WHERE id = ?1",
            params![conversation_id],
            row_to_conversation,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("会话 {conversation_id} 不存在")))?
    };

    // 强制重新总结：临时构造一个无缓存的 conv
    let mut conv_forced = conv.clone();
    conv_forced.summary = None;
    conv_forced.summary_up_to = None;
    conv_forced.summary_chars = 0;

    let (_, _) = build_context_aware_history(vault, &conv_forced).await?;

    // 重新读取更新后的会话
    let conn = db::open(vault)?;
    Ok(conn
        .query_row(
            "SELECT id, paper_id, title, created_at, updated_at, summary, summary_up_to, summary_chars FROM ai_conversations WHERE id = ?1",
            params![conversation_id],
            row_to_conversation,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("会话 {conversation_id} 不存在")))?)
}

/// 发送用户消息 + 流式获取 AI 回复。
///
/// 事件流（通过 Tauri event 推送到前端）：
/// - `ai-chat-delta`：`{ conversation_id, message_id, delta, thinking, done }`
///   - delta：本次增量正文
///   - thinking：本次增量思考（可选）
///   - done：是否完成
/// - `ai-chat-error`：`{ conversation_id, error }`
pub async fn send_message(
    vault: &Path,
    app: &AppHandle,
    conversation_id: &str,
    content: &str,
) -> AppResult<AIMessage> {
    let cfg = ai_svc::get_provider(vault)?;

    // 1. 持久化用户消息
    {
        let conn = db::open(vault)?;
        insert_message(&conn, conversation_id, "user", content, None, None, None, None)?;
    }

    // 2. 加载会话（含 summary 缓存字段）
    let conv: AIConversation = {
        let conn = db::open(vault)?;
        conn.query_row(
            "SELECT id, paper_id, title, created_at, updated_at, summary, summary_up_to, summary_chars FROM ai_conversations WHERE id = ?1",
            params![conversation_id],
            row_to_conversation,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("会话 {conversation_id} 不存在")))?
    };

    // 3. 加载历史消息作为上下文（超限时自动触发 LLM 总结压缩）
    let (history, just_summarized) = build_context_aware_history(vault, &conv).await?;

    let mut messages = vec![client::ChatMessage {
        role: "system".into(),
        content: format!("{BASE_IDENTITY}\n\n你是用户的论文阅读助手。回答时优先引用论文内容。"),
    }];

    // 注入论文上下文
    let context_summary = if let Some(pid) = &conv.paper_id {
        if let Some(paper) = crate::services::paper::load_paper(vault, pid)? {
            let ctx = format!(
                "论文上下文：\n标题：{}\n作者：{}\n年份：{}\nDOI：{}\n摘要：{}",
                paper.title,
                paper.authors.join(", "),
                paper.year.map(|y| y.to_string()).unwrap_or_default(),
                paper.doi,
                paper.abstract_text
            );
            messages.push(client::ChatMessage {
                role: "system".into(),
                content: ctx.clone(),
            });
            Some(ctx)
        } else {
            None
        }
    } else {
        None
    };

    // 加历史消息（不含刚插入的 user 消息，因为它在 history 末尾）
    // 实际上 history 已含刚插入的 user 消息，直接用
    messages.extend(history);

    // 若刚做了总结，推送通知给前端
    if just_summarized {
        let _ = app.emit(
            "ai-chat-summarized",
            serde_json::json!({
                "conversation_id": conversation_id,
                "message": "对话历史较长，已对旧消息做总结压缩以节省上下文"
            }),
        );
    }

    // 4. 流式调用 LLM
    let conv_id = conversation_id.to_string();
    let app_clone = app.clone();
    let thinking_acc = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let thinking_acc_clone = thinking_acc.clone();

    let full_content = client::chat_stream(&cfg, messages, move |delta, thinking| {
        if !thinking.is_empty() {
            if let Ok(mut t) = thinking_acc_clone.lock() {
                t.push_str(thinking);
            }
        }
        let _ = app_clone.emit(
            "ai-chat-delta",
            serde_json::json!({
                "conversation_id": conv_id,
                "delta": delta,
                "thinking": thinking,
                "done": false,
            }),
        );
    })
    .await?;

    let thinking_final = thinking_acc
        .lock()
        .map(|t| t.clone())
        .unwrap_or_default();

    // 5. 持久化 AI 消息
    let ai_msg = {
        let conn = db::open(vault)?;
        insert_message(
            &conn,
            conversation_id,
            "assistant",
            &full_content,
            if thinking_final.is_empty() {
                None
            } else {
                Some(&thinking_final)
            },
            context_summary.as_deref(),
            None,
            None,
        )?
    };

    // 6. 推送完成事件
    let _ = app.emit(
        "ai-chat-delta",
        serde_json::json!({
            "conversation_id": conversation_id,
            "message_id": ai_msg.id,
            "delta": "",
            "thinking": "",
            "done": true,
        }),
    );

    Ok(ai_msg)
}

/// 在对话中执行 preset 快捷功能（流式）。
///
/// 与 `ai_svc::run` 类似，但：
/// - 把 preset 的 user 消息作为用户输入持久化到对话
/// - 流式推送 AI 回复
/// - 持久化 AI 回复（含 preset_id 标记）
pub async fn run_preset_in_chat(
    vault: &Path,
    app: &AppHandle,
    conversation_id: &str,
    preset_id: &str,
) -> AppResult<AIMessage> {
    let cfg = ai_svc::get_provider(vault)?;
    let p = crate::services::preset::get_effective(vault, preset_id)?;

    // 加载会话 + 论文
    let conv: AIConversation = {
        let conn = db::open(vault)?;
        conn.query_row(
            "SELECT id, paper_id, title, created_at, updated_at, summary, summary_up_to, summary_chars FROM ai_conversations WHERE id = ?1",
            params![conversation_id],
            row_to_conversation,
        )
        .optional()?
        .ok_or_else(|| AppError::NotFound(format!("会话 {conversation_id} 不存在")))?
    };

    // 收集 vars（与 ai_svc::run 一致）
    let mut vars: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut context_summary = String::new();

    if let Some(pid) = &conv.paper_id {
        if let Some(paper) = crate::services::paper::load_paper(vault, pid)? {
            vars.insert("title".into(), paper.title.clone());
            vars.insert("authors".into(), paper.authors.join(", "));
            vars.insert(
                "year".into(),
                paper.year.map(|y| y.to_string()).unwrap_or_default(),
            );
            vars.insert("venue".into(), paper.venue.clone());
            vars.insert("doi".into(), paper.doi.clone());
            vars.insert("abstract".into(), paper.abstract_text.clone());
            vars.insert("keywords".into(), paper.keywords.join(", "));

            context_summary = format!(
                "论文：{}（{}）\nDOI：{}",
                paper.title,
                paper.authors.join(", "),
                paper.doi
            );

            // file_name 变量（metadata_from_pdf preset 需要）
            if !paper.pdf_path.is_empty() {
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

                // first_page_text 变量（metadata_from_pdf preset 需要）
                if pp.exists() {
                    if let Ok(basic) = crate::pdf::extract_basic(&pp) {
                        vars.insert("first_page_text".into(), basic.first_page_text);
                    }
                }
            }

            // pdf_text 变量：优先读导入时转存的 {id}-fulltext.md（避免重复提取 PDF），
            // 回退到现场提取 PDF 全文。
            // 注意：pdf_text 只用于渲染 LLM 的 user_msg，不会持久化到对话记录。
            let fulltext_md = vault
                .join(crate::vault::NOTES_DIR)
                .join(crate::vault::NOTES_PAPERS_DIR)
                .join(format!("{pid}-fulltext.md"));
            if fulltext_md.exists() {
                if let Ok(text) = std::fs::read_to_string(&fulltext_md) {
                    vars.insert("pdf_text".into(), text);
                }
            } else if !paper.pdf_path.is_empty() {
                let pp = if std::path::Path::new(&paper.pdf_path).is_absolute() {
                    std::path::PathBuf::from(&paper.pdf_path)
                } else {
                    vault.join(&paper.pdf_path)
                };
                if pp.exists() {
                    let pages = crate::pdf::extract_pages(&pp);
                    if !pages.is_empty() {
                        let text: String = pages
                            .iter()
                            .map(|(_, t)| t.as_str())
                            .collect::<Vec<_>>()
                            .join("\x0c");
                        vars.insert("pdf_text".into(), text);
                    }
                }
            }
        }
    }

    let user_msg = crate::ai::template::render(&p.user_template, &vars)?;

    // 1. 持久化用户消息（preset 触发）
    // 所有 preset 都只存简短引用，不含论文内容（全文/摘要/元数据）。
    // 原因：论文上下文通过 send_message 的 system 消息注入（每次请求一次），
    // PDF 全文通过当前 preset 的 user 消息注入（仅当前请求，不持久化）。
    // 这样历史消息中不会重复出现论文原文，避免 LLM 请求臃肿。
    let persisted_user_msg = format!("[快捷功能：{}]", p.name);
    {
        let conn = db::open(vault)?;
        insert_message(
            &conn,
            conversation_id,
            "user",
            &persisted_user_msg,
            None,
            if context_summary.is_empty() {
                None
            } else {
                Some(&context_summary)
            },
            None,
            Some(preset_id),
        )?;
    }

    // 2. 构造 LLM 请求
    let system_content = format!("{BASE_IDENTITY}\n\n{}", p.system_prompt);
    let mut messages = vec![client::ChatMessage {
        role: "system".into(),
        content: system_content,
    }];

    // 加载历史消息作为上下文（超限时自动触发 LLM 总结压缩）。
    // 注意：build_context_aware_history 会加载全部消息（含刚持久化的当前 user 消息），
    // 因此下面不再单独 push user_msg，避免重复。
    let (history, just_summarized) = build_context_aware_history(vault, &conv).await?;
    messages.extend(history);

    // 若刚做了总结，推送通知给前端
    if just_summarized {
        let _ = app.emit(
            "ai-chat-summarized",
            serde_json::json!({
                "conversation_id": conversation_id,
                "message": "对话历史较长，已对旧消息做总结压缩以节省上下文"
            }),
        );
    }

    // 当前 preset 渲染后的 user_msg（含全文，发给 LLM 但不持久化）
    // 但若 history 已含刚持久化的 user 消息（引用版），则替换为含全文版
    if let Some(last) = messages.last_mut() {
        if last.role == "user" {
            last.content = user_msg;
        }
    } else {
        messages.push(client::ChatMessage {
            role: "user".into(),
            content: user_msg,
        });
    }

    // 3. 流式调用
    let conv_id = conversation_id.to_string();
    let app_clone = app.clone();
    let thinking_acc = std::sync::Arc::new(std::sync::Mutex::new(String::new()));
    let thinking_acc_clone = thinking_acc.clone();

    let full_content = client::chat_stream(&cfg, messages, move |delta, thinking| {
        if !thinking.is_empty() {
            if let Ok(mut t) = thinking_acc_clone.lock() {
                t.push_str(thinking);
            }
        }
        let _ = app_clone.emit(
            "ai-chat-delta",
            serde_json::json!({
                "conversation_id": conv_id,
                "delta": delta,
                "thinking": thinking,
                "done": false,
            }),
        );
    })
    .await?;

    let thinking_final = thinking_acc
        .lock()
        .map(|t| t.clone())
        .unwrap_or_default();

    // 4. 持久化 AI 消息
    let ai_msg = {
        let conn = db::open(vault)?;
        insert_message(
            &conn,
            conversation_id,
            "assistant",
            &full_content,
            if thinking_final.is_empty() {
                None
            } else {
                Some(&thinking_final)
            },
            if context_summary.is_empty() {
                None
            } else {
                Some(&context_summary)
            },
            None,
            Some(preset_id),
        )?
    };

    // 5. 推送完成事件
    let _ = app.emit(
        "ai-chat-delta",
        serde_json::json!({
            "conversation_id": conversation_id,
            "message_id": ai_msg.id,
            "delta": "",
            "thinking": "",
            "done": true,
        }),
    );

    Ok(ai_msg)
}

/// 让未使用的导入告警安静（AIProviderConfig 在签名里用到，但某些路径下被优化掉）。
#[allow(dead_code)]
fn _ensure_imports(_: &AIProviderConfig) {}
