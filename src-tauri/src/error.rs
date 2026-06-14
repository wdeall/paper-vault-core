//! 统一错误类型。serde 序列化到前端为 `{ kind, message }`。

use serde::{Serialize, Serializer};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(String),

    #[error("IO 错误: {0}")]
    Io(String),

    #[error("PDF 错误: {0}")]
    Pdf(String),

    #[error("Markdown 错误: {0}")]
    Markdown(String),

    #[error("AI 错误: {0}")]
    Ai(String),

    #[error("配置错误: {0}")]
    Config(String),

    #[error("参数错误: {0}")]
    Invalid(String),

    #[error("未找到: {0}")]
    NotFound(String),

    #[error("其他: {0}")]
    Other(String),
}

impl AppError {
    pub fn kind(&self) -> &'static str {
        match self {
            AppError::Db(_) => "db",
            AppError::Io(_) => "io",
            AppError::Pdf(_) => "pdf",
            AppError::Markdown(_) => "markdown",
            AppError::Ai(_) => "ai",
            AppError::Config(_) => "config",
            AppError::Invalid(_) => "invalid",
            AppError::NotFound(_) => "not_found",
            AppError::Other(_) => "other",
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        AppError::Db(e.to_string())
    }
}
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}
impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::Other(format!("json: {e}"))
    }
}
impl From<serde_yaml::Error> for AppError {
    fn from(e: serde_yaml::Error) -> Self {
        AppError::Markdown(format!("yaml: {e}"))
    }
}
impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Ai(e.to_string())
    }
}
impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        AppError::Other(e.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
