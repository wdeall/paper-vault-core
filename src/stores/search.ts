// 搜索 store - M-C P3 双通道搜索
// 支持三种模式：structured（结构化）/ fulltext（全文）/ both（双通道）
import { create } from "zustand";
import type { SearchHit, PaperSummary, StructuredQuery } from "@/types";
import { api } from "@/lib/api";

export type SearchMode = "structured" | "fulltext" | "both";

// 初始 structured 查询条件：所有字段为 null
const EMPTY_STRUCTURED: StructuredQuery = {
  title: null,
  author: null,
  year: null,
  venue: null,
  doi: null,
  status: null,
  keyword: null,
};

interface SearchState {
  mode: SearchMode;
  // Structured 查询条件
  structured: StructuredQuery;
  // Fulltext 查询词
  fulltextQuery: string;
  // 结果（两种类型互斥）
  paperResults: PaperSummary[]; // structured / both 模式
  hitResults: SearchHit[]; // fulltext 模式
  isSearching: boolean;
  // actions
  setMode: (m: SearchMode) => void;
  setStructured: (patch: Partial<StructuredQuery>) => void;
  setFulltextQuery: (q: string) => void;
  run: () => Promise<void>;
  clear: () => void;
}

export const useSearchStore = create<SearchState>((set, get) => ({
  mode: "fulltext",
  structured: { ...EMPTY_STRUCTURED },
  fulltextQuery: "",
  paperResults: [],
  hitResults: [],
  isSearching: false,

  setMode: (m) =>
    set({
      mode: m,
      paperResults: [],
      hitResults: [],
    }),

  setStructured: (patch) =>
    set((s) => ({ structured: { ...s.structured, ...patch } })),

  setFulltextQuery: (q) => set({ fulltextQuery: q }),

  run: async () => {
    const { mode, structured, fulltextQuery } = get();
    set({ isSearching: true });
    try {
      if (mode === "structured") {
        const paperResults = await api.searchStructured(structured);
        set({ paperResults, hitResults: [], isSearching: false });
      } else if (mode === "fulltext") {
        const trimmed = fulltextQuery.trim();
        const hitResults = trimmed ? await api.searchFulltext(trimmed) : [];
        set({ hitResults, paperResults: [], isSearching: false });
      } else {
        // both 模式：结构化条件 + 全文查询
        const fts = fulltextQuery.trim();
        const paperResults = await api.searchBoth(structured, fts);
        set({ paperResults, hitResults: [], isSearching: false });
      }
    } catch (e) {
      set({ isSearching: false, paperResults: [], hitResults: [] });
      throw e;
    }
  },

  clear: () =>
    set({
      structured: { ...EMPTY_STRUCTURED },
      fulltextQuery: "",
      paperResults: [],
      hitResults: [],
    }),
}));
