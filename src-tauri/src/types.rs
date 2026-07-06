//! 跨前后端的领域模型定义（serde 兼容）。
//!
//! v2.0 P0 之后：app 层只读写新结构化表（creators / paper_creators /
//! identifiers / keywords / paper_keywords / attachments / …）。`Paper`
//! 始终是"合并视图"：在 `services::paper` 层从多个表 JOIN 出来，再
//! 以这种形态返回给前端。
//!
//! `Paper.status` 在 P0 起统一为 `unread | reading | read` 三个英文
//! 枚举值；旧的中文值（未读 / 阅读中 / 已读 / 重点重读）在
//! `db::migrate_v2::normalize_status` 中已经映射完成。

use serde::{Deserialize, Serialize};

// ============================================================
// 枚举
// ============================================================

/// 阅读状态枚举。P0 唯一持久化形态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PaperStatus {
    #[default]
    Unread,
    Reading,
    Read,
}

impl PaperStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            PaperStatus::Unread => "unread",
            PaperStatus::Reading => "reading",
            PaperStatus::Read => "read",
        }
    }

    /// 接受任意旧值或新值；不在白名单时回退到 `unread`。
    pub fn parse(s: &str) -> Self {
        match s.trim() {
            "reading" => PaperStatus::Reading,
            "read" => PaperStatus::Read,
            // 旧值：
            "未读" => PaperStatus::Unread,
            "阅读中" => PaperStatus::Reading,
            "已读" | "重点重读" => PaperStatus::Read,
            // 兜底
            _ => PaperStatus::Unread,
        }
    }
}

impl std::fmt::Display for PaperStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for PaperStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::parse(s))
    }
}

// ============================================================
// 主表：Paper 合并视图
// ============================================================

/// 论文"合并视图"，是 API 边界。底层由 services 层 JOIN 多表后填充。
///
/// 注意：
/// - `tags` 字段已被 P0 移除（v2.0 PLAN §3.2：tags 本轮不延续到新
///   schema）。如有遗留 UI 调用应自行去除。
/// - `abstract_text` 不重命名为 `abstract`，避免与 SQL 关键字冲突。
/// - `doi` 仍保留在 `papers` 主表（兼容旧 layout）；同时
///   `identifiers` 表也保存一份（主源）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct Paper {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub venue: String,
    pub doi: String,
    pub abstract_text: String,
    pub keywords: Vec<String>,
    pub status: PaperStatus,
    pub rating: Option<i32>,
    pub pdf_path: String,
    pub note_path: String,
    pub created_at: i64,
    pub updated_at: i64,
}

// ============================================================
// 子表
// ============================================================

// P0: 这些结构化子表类型先就位（迁移 + 类型同步），具体写入 / 读取
// 在 P1+（子表 CRUD 命令）启用，目前仅作 serde 兼容与文档作用。
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Creator {
    pub id: String,
    pub family_name: String,
    pub given_name: String,
    pub display_name: String,
    pub raw: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperCreator {
    pub paper_id: String,
    pub creator_id: String,
    pub position: i32,
    pub role: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Identifier {
    pub id: i64,
    pub paper_id: String,
    #[serde(rename = "type")]
    pub kind: String, // "doi" / "arxiv" / "pmid" / "isbn" / "issn" / "url"
    pub value: String,
    pub is_primary: bool,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyword {
    pub id: String,
    pub name: String,
    pub source: String, // "auto" / "manual"
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub id: String,
    pub paper_id: String,
    pub kind: String, // "pdf" / "note" / "supplement" / "snapshot"
    pub rel_path: String,
    pub abs_path: Option<String>,
    pub mime_type: Option<String>,
    pub title: Option<String>,
    pub frontmatter: Option<String>,
    pub sha256: Option<String>,
    pub imported_at: i64,
    pub status: String, // "active" / "missing" / "deleted"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Annotation {
    pub id: String,
    pub paper_id: String,
    pub attachment_id: Option<String>,
    pub kind: String, // "highlight" / "note" / "underline" / "strike" / "image"
    pub page: Option<i32>,
    pub rect: Option<String>,
    pub color: Option<String>,
    pub text: Option<String>,
    pub comment: Option<String>,
    pub created_at: i64,
    pub modified_at: Option<i64>,
}

/// `create_annotation` 命令入参（避免 8 参数触发 clippy::too_many_arguments）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationInput {
    pub kind: String,
    pub page: Option<i32>,
    pub rect: Option<String>,
    pub color: Option<String>,
    pub text: Option<String>,
    pub comment: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaperRelation {
    pub id: i64,
    pub src_paper_id: String,
    pub dst_paper_id: String,
    pub relation: String, // "cites" / "cited_by" / "related" / "replaces"
    pub note: Option<String>,
    pub created_at: i64,
}

/// P2 重复合并审计。
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    /// merge_log 主键（5 分钟内可用 `undo_merge` 撤销）。
    pub merge_id: i64,
    pub canonical_id: String,
    pub duplicate_id: String,
    pub fields_merged: Vec<String>,
    pub snapshot: Option<String>,
    pub merged_at: i64,
}

// ============================================================
// 现有领域类型（保持 API 稳定）
// ============================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PaperDetail {
    #[serde(flatten)]
    pub paper: Paper,
    pub reading_progress: Option<ReadingProgress>,
    pub index_status: String,
    pub collections: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReadingProgress {
    pub current_page: i32,
    pub total_pages: i32,
    pub progress_percent: f32,
    pub last_read_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DuplicateCandidate {
    pub paper_id: String,
    pub title: String,
    pub reason: String,
    pub confidence: String, // "high" / "medium" / "low"
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SearchHit {
    pub paper_id: String,
    // papers_fts 字段名: "title" / "abstract" / "authors" / "keywords" / "venue" / "doi"
    pub source_type: String,
    pub snippet: String,
    pub page: Option<i32>,
    pub score: f32,
}

/// P3: 结构化搜索查询。所有字段可选,AND 组合。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StructuredQuery {
    pub title: Option<String>,   // 模糊 LIKE %x%
    pub author: Option<String>,  // 跨 creators + paper_creators, display_name LIKE %x%
    pub year: Option<i32>,       // 精确
    pub venue: Option<String>,   // 模糊 LIKE %x%
    pub doi: Option<String>,     // 精确 (规范化后)
    pub status: Option<String>,  // unread|reading|read
    pub keyword: Option<String>, // 跨 keywords + paper_keywords, name LIKE %x%
}

/// P3: 搜索结果摘要 (Structured / Both 模式返回)。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PaperSummary {
    pub id: String,
    pub title: String,
    pub year: Option<i32>,
    pub authors: Vec<String>,
    pub venue: String,
    pub status: String,
    pub rating: Option<i32>,
    /// Both 模式带 FTS score;Structured 模式为 None
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IndexStatusSummary {
    pub total: i64,
    pub indexed: i64,
    pub indexing: i64,
    pub failed: i64,
    pub pending: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NoteContent {
    pub path: String,
    pub frontmatter: serde_json::Value,
    pub content: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataCandidate {
    pub source: String,    // "ai" / "user" / …
    pub confidence: f32,   // 0.0–1.0
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub venue: String,
    pub doi: String,
    pub abstract_text: String,
    pub keywords: Vec<String>,
    /// P1: 解析得到的其它 identifier 列表（scheme, value）。
    /// 例如从 arXiv 解析时可能同时带 DOI；后续 `import_by_identifier`
    /// 会把这些全部写入 `identifiers` 表。
    #[serde(default)]
    pub identifiers: Vec<(String, String)>,
}

/// P1: 4 个 resolver 拉到的元数据。
///
/// `import_by_identifier` 拿到这个结构后做去重检测 + 入库。
/// 与 `MetadataCandidate` 不同：resolver 一定带 `identifiers` 列表，
/// 而 `MetadataCandidate` 是给 AI/手动填的更轻量结构。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PaperMetadata {
    pub title: String,
    pub authors: Vec<String>,
    pub year: Option<i32>,
    pub venue: String,
    pub doi: String,
    pub abstract_text: String,
    pub keywords: Vec<String>,
    /// 至少含调用方传入的 (scheme, value)；resolver 也可补足其它。
    pub identifiers: Vec<(String, String)>,
}

// ============================================================
// 命令返回 / 入参专用类型
// ============================================================

/// `get_vault_info` 命令的返回值。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultInfo {
    pub path: String,
    pub paper_count: i64,
    pub indexed_count: i64,
}

/// `import_pdf` / `import_pdfs_batch` 返回。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub paper: Paper,
    pub duplicates: Vec<DuplicateCandidate>,
}

/// `collections` 列表项。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    pub created_at: i64,
}

/// `ai_provider_config` 表 → 内存结构。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AIProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

/// `run_ai` 命令的返回值。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AIResult {
    pub raw: String,
    pub parsed: Option<serde_json::Value>,
    pub markdown: String,
}

/// `ai_skill_presets` 表 → 内存结构。
#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// AI 对话入参：单条历史消息（role: "user" | "assistant"）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageInput {
    pub role: String,
    pub content: String,
}
