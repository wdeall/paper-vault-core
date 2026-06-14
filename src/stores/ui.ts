// 全局 UI 状态

import { create } from "zustand";

export interface Toast {
  id: number;
  type: "info" | "success" | "warning" | "error";
  message: string;
}

interface UIState {
  sidebarOpen: boolean;
  theme: "light" | "dark";
  toasts: Toast[];
  showToast: (type: Toast["type"], message: string) => void;
  removeToast: (id: number) => void;
  setTheme: (t: "light" | "dark") => void;
  toggleSidebar: () => void;
}

let nextId = 1;

export const useUIStore = create<UIState>((set) => ({
  sidebarOpen: true,
  theme: "dark",
  toasts: [],
  showToast: (type, message) => {
    const id = nextId++;
    set((s) => ({ toasts: [...s.toasts, { id, type, message }] }));
    setTimeout(() => {
      set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
    }, 4000);
  },
  removeToast: (id) =>
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  setTheme: (t) => {
    set({ theme: t });
    document.documentElement.classList.toggle("dark", t === "dark");
  },
  toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
}));
