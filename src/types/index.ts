// 与 Rust 共享的核心数据类型
// 字段名与后端 serde 自动转换对齐

export type PaperStatus = "未读" | "阅读中" | "已读" | "重点重读";
export type IndexStatus = "未索引" | "索引中" | "已索引" | "索引失败";

export interface Paper {
  id: string;
  title: string;
  authors: string[];
  year: number | null;
  venue: string;
  doi: string;
  abstract_text: string;
  keywords: string[];
  tags: string[];
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

export interface SearchHit {
  paper_id: string;
  source_type:
    | "title"
    | "authors"
    | "abstract"
    | "keywords"
    | "doi"
    | "notes"
    | "pdf";
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
