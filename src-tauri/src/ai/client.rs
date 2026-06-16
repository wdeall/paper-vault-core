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
        .timeout(Duration::from_secs(120))
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
            };
            send_chat(&client, &url, &cfg.api_key, &fallback_req).await
        }
        Err(e) => Err(e),
    }
}
