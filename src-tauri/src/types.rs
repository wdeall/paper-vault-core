//! 与前端共享的核心数据类型。
//! 用 snake_case 字段以便 serde 自动转换。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Paper {
    pub id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub authors: Vec<String>,
    pub year: Option<i32>,
    #[serde(default)]
    pub venue: String,
    #[serde(default)]
    pub doi: String,
    #[serde(default)]
    pub abstract_text: String,
    #[serde(default)]
    pub keywords: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_status")]
    pub status: String,
    pub rating: Option<i32>,
    #[serde(default)]
    pub pdf_path: String,
    #[serde(default)]
    pub note_path: String,
    pub created_at: i64,
    pub updated_at: i64,
}

fn default_status() -> String {
    "未读".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ReadingProgress {
    pub paper_id: String,
    pub current_page: i32,
    pub total_pages: i32,
    pub progress_percent: f32,
    pub last_read_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct PaperDetail {
    #[serde(flatten)]
    pub paper: Paper,
    pub reading_progress: Option<ReadingProgress>,
    #[serde(default)]
    pub index_status: String,
    #[serde(default)]
    pub collections: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct MetadataCandidate {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub venue: String,
    pub doi: String,
    pub abstract_text: String,
    pub keywords: Vec<String>,
    pub source: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct DuplicateCandidate {
    pub paper_id: String,
    pub title: String,
    pub reason: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct AISkillPreset {
    pub id: String,
    pub name: String,
    pub bound_action: String,
    pub skill: String,
    pub system_prompt: String,
    pub user_template: String,
    pub output_format: String,
    pub auto_write: bool,
    pub is_builtin: bool,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct AIProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct SearchHit {
    pub paper_id: String,
    pub source_type: String,
    pub snippet: String,
    pub page: Option<i32>,
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct NoteContent {
    pub path: String,
    pub frontmatter: serde_json::Value,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ImportResult {
    pub paper: Paper,
    pub duplicates: Vec<DuplicateCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct VaultInfo {
    pub path: String,
    pub paper_count: i64,
    pub indexed_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct IndexStatusSummary {
    pub total: i64,
    pub indexed: i64,
    pub indexing: i64,
    pub failed: i64,
    pub pending: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct AIResult {
    pub raw: String,
    pub parsed: Option<serde_json::Value>,
    pub markdown: String,
}
