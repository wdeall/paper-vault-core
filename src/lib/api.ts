// Tauri IPC 客户端封装
// 所有命令的 typed 包装，统一错误处理

import { invoke } from "@tauri-apps/api/core";
import type {
  AIProviderConfig,
  AIResult,
  AISkillPreset,
  Collection,
  DuplicateCandidate,
  ImportResult,
  IndexStatusSummary,
  MergeResult,
  MetadataCandidate,
  NoteContent,
  Paper,
  PaperDetail,
  ReadingProgress,
  SearchHit,
  VaultInfo,
} from "@/types";

export class ApiError extends Error {
  constructor(public kind: string, message: string) {
    super(message);
    this.name = "ApiError";
  }
}

// 后端 AppError 的 thiserror 模板会给 message 加 "未找到: " / "其他: " 之类前缀，
// 分类已经在 kind 里体现了，前端只需要保留原始内容避免重复。
const KIND_MESSAGE_PREFIX: Record<string, string> = {
  db: "数据库错误: ",
  io: "IO 错误: ",
  pdf: "PDF 错误: ",
  markdown: "Markdown 错误: ",
  ai: "AI 错误: ",
  config: "配置错误: ",
  invalid: "参数错误: ",
  not_found: "未找到: ",
  other: "其他: ",
};

function friendlyMessage(kind: string, message: string): string {
  const prefix = KIND_MESSAGE_PREFIX[kind];
  if (prefix && message.startsWith(prefix)) {
    return message.slice(prefix.length);
  }
  return message;
}

async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    if (e && typeof e === "object" && "kind" in e && "message" in e) {
      const kind = (e as { kind: string }).kind;
      const message = friendlyMessage(
        kind,
        (e as { message: string }).message,
      );
      throw new ApiError(kind, message);
    }
    throw new ApiError("unknown", String(e));
  }
}

// === Init / Vault ===
export const api = {
  initVault: (path: string) => call<void>("init_vault", { path }),
  getVaultInfo: () => call<VaultInfo>("get_vault_info"),
  openVaultFolder: () => call<void>("open_vault_folder"),
  backupDatabase: () => call<string>("backup_database"),
  loadSeedData: () => call<string[]>("load_seed_data"),

  // === Papers ===
  importPdf: (sourcePath: string) =>
    call<ImportResult>("import_pdf", { sourcePath }),
  importPdfsBatch: (sourcePaths: string[]) =>
    call<ImportResult[]>("import_pdfs_batch", { sourcePaths }),
  // P1: 按 identifier（DOI / arXiv / PMID / ISBN）导入。
  // 入参 raw 支持纯标识符或 URL，由后端解析。
  importByIdentifier: (raw: string) =>
    call<ImportResult>("import_by_identifier", { raw }),
  // P2: 合并 src → dst；返回的 merge_id 5 分钟内可撤销。
  mergePapers: (srcId: string, dstId: string) =>
    call<MergeResult>("merge_papers", { srcId, dstId }),
  undoMerge: (mergeId: number) => call<void>("undo_merge", { mergeId }),
  // P0: tag 过滤参数已移除（tags 不再保留）。status 接收英文枚举字符串
  // （"unread" | "reading" | "read"），后端会兜底为 "unread"。
  listPapers: (params?: {
    status?: string;
    collectionId?: string;
  }) =>
    call<Paper[]>("list_papers", {
      status: params?.status ?? null,
      collectionId: params?.collectionId ?? null,
    }),
  getPaper: (id: string) => call<PaperDetail>("get_paper", { id }),
  updatePaper: (id: string, patch: Paper) =>
    call<Paper>("update_paper", { id, patch }),
  // P0: 删除模式固定为硬删（PLAN §3.5：删除始终硬删；P3 引入软删除）。
  deletePaper: (id: string) => call<void>("delete_paper", { id }),
  updateProgress: (id: string, currentPage: number, totalPages?: number) =>
    call<ReadingProgress>("update_progress", {
      id,
      currentPage,
      totalPages: totalPages ?? null,
    }),
  checkDuplicates: (params: {
    doi?: string;
    title?: string;
    authors?: string[];
    year?: number;
  }) =>
    call<DuplicateCandidate[]>("check_duplicates", {
      doi: params.doi ?? null,
      title: params.title ?? null,
      authors: params.authors ?? null,
      year: params.year ?? null,
    }),
  extractMetadata: (id: string) =>
    call<MetadataCandidate>("extract_metadata", { id }),
  readPdfBytes: (id: string) =>
    call<number[]>("read_pdf_bytes", { id }),
  openPdf: (id: string) =>
    call<void>("open_pdf", { id }),

  // === Collections / Tags / Keywords ===
  listCollections: () => call<Collection[]>("list_collections"),
  createCollection: (name: string, parentId?: string) =>
    call<Collection>("create_collection", { name, parentId: parentId ?? null }),
  addPaperToCollection: (paperId: string, collectionId: string) =>
    call<void>("add_paper_to_collection", { paperId, collectionId }),
  removePaperFromCollection: (paperId: string, collectionId: string) =>
    call<void>("remove_paper_from_collection", { paperId, collectionId }),
  // P0: 后端 list_keywords 返回 [(name, count)] 元组。
  listKeywords: () => call<Array<[string, number]>>("list_keywords"),
  // tags 端点保留为兼容性 stub，始终返回空数组（PLAN §3.5）。
  listTags: () => call<string[]>("list_tags"),

  // === Notes ===
  createNote: (id: string) => call<string>("create_note", { id }),
  importNote: (id: string, sourcePath: string) =>
    call<string>("import_note", { id, sourcePath }),
  getNote: (id: string) => call<NoteContent>("get_note", { id }),
  updateNote: (id: string, content: string) =>
    call<void>("update_note", { id, content }),
  updateNoteAiBlock: (id: string, block: "summary" | "key_points", content: string) =>
    call<void>("update_note_ai_block", { id, block, content }),

  // === Search ===
  search: (query: string, scopes?: string[]) =>
    call<SearchHit[]>("search", { query, scopes: scopes ?? null }),
  reindexPaper: (id: string) => call<void>("reindex_paper", { id }),
  reindexAll: () => call<void>("reindex_all"),
  getFtsStatus: () => call<IndexStatusSummary>("get_fts_status"),

  // === AI ===
  getAiPresets: () => call<AISkillPreset[]>("get_ai_presets"),
  updateAiPreset: (id: string, patch: AISkillPreset) =>
    call<AISkillPreset>("update_ai_preset", { id, patch }),
  resetAiPreset: (id: string) => call<AISkillPreset>("reset_ai_preset", { id }),
  runAi: (presetId: string, paperId?: string, input?: string) =>
    call<AIResult>("run_ai", {
      presetId,
      paperId: paperId ?? null,
      input: input ?? null,
    }),
  getAiConfig: () => call<AIProviderConfig>("get_ai_config"),
  updateAiConfig: (patch: AIProviderConfig) =>
    call<AIProviderConfig>("update_ai_config", { patch }),

  // === Export ===
  exportBibtex: (ids: string[]) => call<string>("export_bibtex", { ids }),
  exportMarkdownCitation: (ids: string[]) =>
    call<string>("export_markdown_citation", { ids }),
};
