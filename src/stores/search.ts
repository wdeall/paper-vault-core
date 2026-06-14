// 搜索 store
import { create } from "zustand";
import type { SearchHit } from "@/types";
import { api } from "@/lib/api";

interface SearchState {
  query: string;
  hits: SearchHit[];
  isSearching: boolean;
  setQuery: (q: string) => void;
  run: () => Promise<void>;
  clear: () => void;
}

export const useSearchStore = create<SearchState>((set, get) => ({
  query: "",
  hits: [],
  isSearching: false,

  setQuery: (q) => set({ query: q }),

  run: async () => {
    const { query } = get();
    set({ isSearching: true });
    try {
      const hits = query.trim() ? await api.search(query) : [];
      set({ hits, isSearching: false });
    } catch (e) {
      set({ isSearching: false, hits: [] });
      throw e;
    }
  },

  clear: () => set({ query: "", hits: [] }),
}));
