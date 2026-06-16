// 全局 UI 状态

import { create } from "zustand";

export interface Toast {
  id: number;
  type: "info" | "success" | "warning" | "error";
  message: string;
  /** P2：可选的动作按钮（如"撤销"）。 */
  action?: { label: string; onClick: () => void };
  /** 停留时间（毫秒）。有 action 时建议 ≥ UNDO_WINDOW_SEC * 1000。 */
  ttlMs?: number;
}

interface UIState {
  sidebarOpen: boolean;
  theme: "light" | "dark";
  toasts: Toast[];
  showToast: (
    type: Toast["type"],
    message: string,
    options?: { label: string; onClick: () => void; ttlSec?: number },
  ) => void;
  removeToast: (id: number) => void;
  setTheme: (t: "light" | "dark") => void;
  toggleSidebar: () => void;
}

let nextId = 1;

export const useUIStore = create<UIState>((set) => ({
  sidebarOpen: true,
  theme: "dark",
  toasts: [],
  showToast: (type, message, options) => {
    const id = nextId++;
    const action = options ? { label: options.label, onClick: options.onClick } : undefined;
    const ttlMs = options?.ttlSec ? options.ttlSec * 1000 : 4000;
    set((s) => ({ toasts: [...s.toasts, { id, type, message, action, ttlMs }] }));
    setTimeout(() => {
      set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) }));
    }, ttlMs);
  },
  removeToast: (id) =>
    set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  setTheme: (t) => {
    set({ theme: t });
    document.documentElement.classList.toggle("dark", t === "dark");
  },
  toggleSidebar: () => set((s) => ({ sidebarOpen: !s.sidebarOpen })),
}));
