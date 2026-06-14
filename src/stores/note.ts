// 笔记 store

import { create } from "zustand";
import type { NoteContent } from "@/types";
import { api } from "@/lib/api";

interface NoteState {
  currentNote: NoteContent | null;
  isDirty: boolean;
  loadNote: (id: string) => Promise<void>;
  saveNote: (id: string, content: string) => Promise<void>;
  updateAIBlock: (id: string, block: "summary" | "key_points", content: string) => Promise<void>;
  setDirty: (d: boolean) => void;
}

export const useNoteStore = create<NoteState>((set) => ({
  currentNote: null,
  isDirty: false,
  loadNote: async (id: string) => {
    try {
      const nc = await api.getNote(id);
      set({ currentNote: nc, isDirty: false });
    } catch (e) {
      set({ currentNote: null, isDirty: false });
    }
  },
  saveNote: async (id: string, content: string) => {
    try {
      await api.updateNote(id, content);
      set({ isDirty: false });
    } catch (e) {
      console.error("saveNote", e);
      throw e;
    }
  },
  updateAIBlock: async (id, block, content) => {
    try {
      await api.updateNoteAiBlock(id, block, content);
    } catch (e) {
      console.error("updateAIBlock", e);
      throw e;
    }
  },
  setDirty: (d) => set({ isDirty: d }),
}));
