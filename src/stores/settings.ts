// 设置 / AI 配置 / 预设管理

import { create } from "zustand";
import type { AIProviderConfig, AISkillPreset } from "@/types";
import { api } from "@/lib/api";

interface SettingsState {
  aiConfig: AIProviderConfig;
  presets: AISkillPreset[];
  isLoading: boolean;
  loadConfig: () => Promise<void>;
  saveConfig: (cfg: AIProviderConfig) => Promise<void>;
  loadPresets: () => Promise<void>;
  savePreset: (id: string, p: AISkillPreset) => Promise<void>;
  resetPreset: (id: string) => Promise<void>;
}

export const useSettingsStore = create<SettingsState>((set) => ({
  aiConfig: { base_url: "", api_key: "", model: "" },
  presets: [],
  isLoading: false,
  loadConfig: async () => {
    set({ isLoading: true });
    try {
      const cfg = await api.getAiConfig();
      set({ aiConfig: cfg, isLoading: false });
    } catch (e) {
      console.error(e);
      set({ isLoading: false });
    }
  },
  saveConfig: async (cfg) => {
    set({ isLoading: true });
    try {
      const saved = await api.updateAiConfig(cfg);
      set({ aiConfig: saved, isLoading: false });
    } catch (e) {
      console.error(e);
      set({ isLoading: false });
      throw e;
    }
  },
  loadPresets: async () => {
    set({ isLoading: true });
    try {
      const presets = await api.getAiPresets();
      set({ presets, isLoading: false });
    } catch (e) {
      console.error(e);
      set({ isLoading: false });
    }
  },
  savePreset: async (id, p) => {
    set({ isLoading: true });
    try {
      await api.updateAiPreset(id, p);
      const presets = await api.getAiPresets();
      set({ presets, isLoading: false });
    } catch (e) {
      console.error(e);
      set({ isLoading: false });
      throw e;
    }
  },
  resetPreset: async (id) => {
    set({ isLoading: true });
    try {
      await api.resetAiPreset(id);
      const presets = await api.getAiPresets();
      set({ presets, isLoading: false });
    } catch (e) {
      console.error(e);
      set({ isLoading: false });
      throw e;
    }
  },
}));
