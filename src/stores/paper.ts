// 论文 / 集合 状态管理
import { create } from "zustand";
import type { Collection, ImportResult, Paper, PaperDetail } from "@/types";
import { api } from "@/lib/api";

export type SmartView = "all" | "recent" | "modified";

interface PaperState {
  papers: Paper[];
  collections: Collection[];
  selectedPaperId: string | null;
  statusFilter: string | null;
  activeCollectionId: string | null;
  smartView: SmartView;
  isLoading: boolean;
  loadPapers: () => Promise<void>;
  loadCollections: () => Promise<void>;
  getPaper: (id: string) => Promise<PaperDetail>;
  importPdfs: (paths: string[]) => Promise<ImportResult[]>;
  addCollection: (name: string, parentId?: string) => Promise<Collection>;
  selectPaper: (id: string | null) => void;
  setStatusFilter: (s: string | null) => void;
  setActiveCollection: (id: string | null) => void;
  setSmartView: (v: SmartView) => void;
  updatePaper: (id: string, patch: Paper) => Promise<Paper>;
  removePaper: (id: string, mode: "entry" | "entry+pdf" | "entry+pdf+note") => Promise<void>;
}

export const usePaperStore = create<PaperState>((set, get) => ({
  papers: [],
  collections: [],
  selectedPaperId: null,
  statusFilter: null,
  activeCollectionId: null,
  smartView: "all",
  isLoading: false,

  loadPapers: async () => {
    set({ isLoading: true });
    try {
      const { statusFilter, activeCollectionId } = get();
      const papers = await api.listPapers({
        status: statusFilter ?? undefined,
        collectionId: activeCollectionId ?? undefined,
      });
      // 排序：本地再按 updated_at 降序兜底
      papers.sort((a, b) => b.updated_at - a.updated_at);
      set({ papers, isLoading: false });
    } catch (e) {
      set({ isLoading: false });
      throw e;
    }
  },

  loadCollections: async () => {
    try {
      const collections = await api.listCollections();
      set({ collections });
    } catch (e) {
      throw e;
    }
  },

  getPaper: async (id) => {
    return api.getPaper(id);
  },

  importPdfs: async (paths) => {
    const results = await api.importPdfsBatch(paths);
    await get().loadPapers();
    return results;
  },

  addCollection: async (name, parentId) => {
    const c = await api.createCollection(name, parentId);
    await get().loadCollections();
    return c;
  },

  selectPaper: (id) => set({ selectedPaperId: id }),

  setStatusFilter: (s) => set({ statusFilter: s, activeCollectionId: null }),
  setActiveCollection: (id) => set({ activeCollectionId: id, statusFilter: null }),
  setSmartView: (v) => set({ smartView: v, statusFilter: null, activeCollectionId: null }),

  updatePaper: async (id, patch) => {
    const updated = await api.updatePaper(id, patch);
    set((s) => ({
      papers: s.papers.map((p) => (p.id === id ? updated : p)),
    }));
    return updated;
  },

  removePaper: async (id, mode) => {
    await api.deletePaper(id, mode);
    set((s) => ({
      papers: s.papers.filter((p) => p.id !== id),
      selectedPaperId: s.selectedPaperId === id ? null : s.selectedPaperId,
    }));
  },
}));
