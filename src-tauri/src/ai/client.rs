//! OpenAI 兼容 API 客户端。
use crate::error::{AppError, AppResult};
use crate::types::AIProviderConfig;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    response_format: Option<ResponseFormat>,
    temperature: f32,
    stream: bool,
}

#[derive(Debug, Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    type_: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    reasoning: Option<String>,
}

async fn send_chat(
    client: &reqwest::Client,
    url: &str,
    api_key: &str,
    req: &ChatRequest,
) -> AppResult<String> {
    let resp = client
        .post(url)
        .bearer_auth(api_key)
        .json(req)
        .send()
        .await?;

    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(AppError::Ai(format!(
            "HTTP {}: {}",
            status,
            body.chars().take(500).collect::<String>()
        )));
    }

    let parsed: ChatResponse = serde_json::from_str(&body)
        .map_err(|e| AppError::Ai(format!("响应解析失败: {e}; body={}", body.chars().take(500).collect::<String>())))?;

    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| AppError::Ai("API 返回缺少 choices".into()))
}

/// 调用一次 chat completion。
pub async fn chat(
    cfg: &AIProviderConfig,
    messages: Vec<ChatMessage>,
    json_mode: bool,
) -> AppResult<String> {
    if cfg.api_key.is_empty() {
        return Err(AppError::Config("API key 未配置".into()));
    }
    if cfg.base_url.is_empty() {
        return Err(AppError::Config("base_url 未配置".into()));
    }
    if cfg.model.is_empty() {
        return Err(AppError::Config("model 未配置".into()));
    }

    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|e| AppError::Ai(e.to_string()))?;

    let req = ChatRequest {
        model: cfg.model.clone(),
        messages: messages.clone(),
        response_format: if json_mode {
            Some(ResponseFormat {
                type_: "json_object".into(),
            })
        } else {
            None
        },
        temperature: 0.2,
        stream: false,
    };

    match send_chat(&client, &url, &cfg.api_key, &req).await {
        Ok(content) => Ok(content),
        Err(AppError::Ai(msg))
            if json_mode
                && (msg.contains("json_object is not supported")
                    || msg.contains("response_format")
                    || msg.contains("InvalidParameter")) =>
        {
            let fallback_req = ChatRequest {
                model: cfg.model.clone(),
                messages,
                response_format: None,
                temperature: 0.2,
                stream: false,
            };
            send_chat(&client, &url, &cfg.api_key, &fallback_req).await
        }
        Err(e) => Err(e),
    }
}

/// 流式 chat completion。
///
/// `on_delta` 回调在每个 delta 到达时被调用：
///   - `delta`：本次增量正文（可能为空）
///   - `thinking`：本次增量思考过程（部分模型支持，如 deepseek-reasoner）
///
/// 返回完整累积的正文。
pub async fn chat_stream<F>(
    cfg: &AIProviderConfig,
    messages: Vec<ChatMessage>,
    mut on_delta: F,
) -> AppResult<String>
where
    F: FnMut(&str, &str),
{
    if cfg.api_key.is_empty() {
        return Err(AppError::Config("API key 未配置".into()));
    }
    if cfg.base_url.is_empty() {
        return Err(AppError::Config("base_url 未配置".into()));
    }
    if cfg.model.is_empty() {
        return Err(AppError::Config("model 未配置".into()));
    }

    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()
        .map_err(|e| AppError::Ai(e.to_string()))?;

    let req = ChatRequest {
        model: cfg.model.clone(),
        messages,
        response_format: None,
        temperature: 0.2,
        stream: true,
    };

    let resp = client
        .post(&url)
        .bearer_auth(&cfg.api_key)
        .json(&req)
        .send()
        .await?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(AppError::Ai(format!(
            "HTTP {}: {}",
            status,
            body.chars().take(500).collect::<String>()
        )));
    }

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();
    let mut full_content = String::new();

    while let Some(chunk_res) = stream.next().await {
        let chunk = chunk_res.map_err(|e| AppError::Ai(format!("流读取失败: {e}")))?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        // SSE 按双换行分帧
        while let Some(idx) = buf.find("\n\n") {
            let frame: String = buf.drain(..idx + 2).collect();
            for line in frame.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                // 只处理 data: 开头的行
                let data = if let Some(rest) = line.strip_prefix("data:") {
                    rest.trim()
                } else {
                    continue;
                };
                if data == "[DONE]" {
                    continue;
                }
                // 解析 JSON
                if let Ok(parsed) = serde_json::from_str::<StreamChunk>(data) {
                    if let Some(choice) = parsed.choices.into_iter().next() {
                        let delta_content = choice.delta.content.unwrap_or_default();
                        let delta_thinking = choice.delta.reasoning.unwrap_or_default();
                        if !delta_content.is_empty() || !delta_thinking.is_empty() {
                            on_delta(&delta_content, &delta_thinking);
                            if !delta_content.is_empty() {
                                full_content.push_str(&delta_content);
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(full_content)
}
