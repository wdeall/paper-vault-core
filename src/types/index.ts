// 与 Rust 共享的核心数据类型
// 字段名与后端 serde 自动转换对齐
//
// v2.0 P0：status 在前后端之间统一为英文枚举
// (unread | reading | read)；前端显示中文通过 PAPER_STATUS_LABELS 映射。
// tags 已移除（PLAN §3.5: tags 本轮不延续到新 schema）。

export type PaperStatus = "unread" | "reading" | "read";
export type IndexStatus = "未索引" | "索引中" | "已索引" | "索引失败";

// 状态 → 显示标签的映射
export const PAPER_STATUS_LABELS: Record<PaperStatus, string> = {
  unread: "未读",
  reading: "阅读中",
  read: "已读",
};

export interface Paper {
  id: string;
  title: string;
  authors: string[];
  year: number | null;
  venue: string;
  doi: string;
  abstract_text: string;
  keywords: string[];
  status: PaperStatus;
  rating: number | null;
  pdf_path: string;
  note_path: string;
  created_at: number;
  updated_at: number;
}

export interface ReadingProgress {
  paper_id: string;
  current_page: number;
  total_pages: number;
  progress_percent: number;
  last_read_at: number | null;
}

export interface PaperDetail extends Paper {
  reading_progress: ReadingProgress | null;
  index_status: IndexStatus;
  collections: string[];
}

export interface MetadataCandidate {
  title: string;
  authors: string[];
  year: number | null;
  venue: string;
  doi: string;
  abstract_text: string;
  keywords: string[];
  source: "doi" | "pdf-text" | "filename" | "ai" | "manual";
  confidence: "high" | "medium" | "low";
}

export interface DuplicateCandidate {
  paper_id: string;
  title: string;
  reason: string;
  confidence: "high" | "medium" | "low";
}

export interface AISkillPreset {
  id: string;
  name: string;
  bound_action: string;
  skill: "pdf" | "research-lookup" | "literature-review" | "none";
  system_prompt: string;
  user_template: string;
  output_format: "json" | "markdown";
  auto_write: boolean;
  is_builtin: boolean;
  updated_at: number;
}

export interface AIProviderConfig {
  base_url: string;
  api_key: string;
  model: string;
}

// M-C P3：双通道搜索的查询与结果类型
// StructuredQuery 字段对齐后端 Rust struct，模糊字段传字符串、精确字段传字符串/数字
export interface StructuredQuery {
  title?: string | null;
  author?: string | null;
  year?: number | null;
  venue?: string | null;
  doi?: string | null;
  status?: string | null;
  keyword?: string | null;
}

// PaperSummary：结构化 / 双通道搜索返回的论文摘要
// score 在 both 模式下携带 FTS 分数；structured 模式为 null
export interface PaperSummary {
  id: string;
  title: string;
  year: number | null;
  authors: string[];
  venue: string;
  status: string;
  rating: number | null;
  score: number | null;
}

export interface SearchHit {
  paper_id: string;
  source_type:
    | "title"
    | "authors"
    | "abstract"
    | "keywords"
    | "venue"
    | "doi";
  snippet: string;
  page: number | null;
  score: number;
}

export interface NoteContent {
  path: string;
  frontmatter: Record<string, unknown>;
  content: string;
}

export interface ImportResult {
  paper: Paper;
  duplicates: DuplicateCandidate[];
}

/** P2 合并结果。`merge_id` 在 5 分钟内可作为 `undoMerge` 参数。 */
export interface MergeResult {
  merge_id: number;
  canonical_id: string;
  duplicate_id: string;
  fields_merged: string[];
  snapshot?: string;
  merged_at: number;
}

export interface Collection {
  id: string;
  name: string;
  parent_id: string | null;
  created_at: number;
}

export interface VaultInfo {
  path: string;
  paper_count: number;
  indexed_count: number;
}

export interface IndexStatusSummary {
  total: number;
  indexed: number;
  indexing: number;
  failed: number;
  pending: number;
}

export interface AIResult {
  raw: string;
  parsed: Record<string, unknown> | null;
  markdown: string;
}

export interface AppError {
  kind: string;
  message: string;
}

// M-D P4：PDF 批注
// rect 为归一化坐标 (0-1)，相对于页面宽高
// 后端存储为 JSON 字符串，前端类型用对象，api.ts 做反/序列化
export interface AnnotationRect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Annotation {
  id: string;
  paper_id: string;
  attachment_id: string | null;
  kind: string; // "highlight" | "note" | "underline" | "strike"
  page: number | null; // 1-indexed
  rect: AnnotationRect[] | null; // 多 rect（跨行选区）；兼容旧单 rect 数据
  color: string | null; // "yellow" | "red" | "green" | "blue" | "purple"
  text: string | null;
  comment: string | null;
  created_at: number;
  modified_at: number | null;
}

/** 后端 scan_all 返回的重复对（与 Rust duplicates::DuplicatePair 对齐）。 */
export interface DuplicatePair {
  paper_id_a: string;
  title_a: string;
  paper_id_b: string;
  title_b: string;
  reason: string;
  confidence: string;
}

// ============================================================
// AI 对话历史持久化（agent 风格侧边栏）
// ============================================================

export interface AIConversation {
  id: string;
  paper_id: string | null;
  title: string;
  created_at: number;
  updated_at: number;
  /** 对话历史压缩摘要（超限后由 LLM 生成） */
  summary: string | null;
  /** 摘要覆盖到的最后一条消息 id（不含此条） */
  summary_up_to: string | null;
  /** 被总结消息的原始总字符数 */
  summary_chars: number;
}

export interface AIMessage {
  id: string;
  conversation_id: string;
  role: string; // "user" | "assistant" | "system"
  content: string;
  thinking: string | null;
  context: string | null;
  tool_calls: string | null;
  preset_id: string | null;
  created_at: number;
}

/** ai-chat-delta 事件载荷。 */
export interface AIChatDeltaEvent {
  conversation_id: string;
  message_id?: string;
  delta: string;
  thinking: string;
  done: boolean;
}

/** ai-chat-summarized 事件载荷：触发自动总结压缩时由后端推送。 */
export interface AIChatSummarizedEvent {
  conversation_id: string;
  message: string;
}
