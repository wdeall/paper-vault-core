// 全局 Toast 渲染

import { useUIStore } from "@/stores/ui";
import { cn } from "@/lib/utils";

const COLORS = {
  info: "bg-blue-500",
  success: "bg-green-500",
  warning: "bg-yellow-500",
  error: "bg-red-500",
};

export function Toaster() {
  const toasts = useUIStore((s) => s.toasts);
  const remove = useUIStore((s) => s.removeToast);
  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2">
      {toasts.map((t) => (
        <div
          key={t.id}
          className={cn(
            "flex min-w-72 max-w-md items-center gap-2 rounded-md px-4 py-3 text-sm text-white shadow-lg",
            COLORS[t.type],
          )}
        >
          <span className="flex-1">{t.message}</span>
          {t.action && (
            <button
              className="rounded bg-white/20 px-2 py-1 text-xs font-medium hover:bg-white/30"
              onClick={() => {
                t.action!.onClick();
                remove(t.id);
              }}
            >
              {t.action.label}
            </button>
          )}
          <button
            className="text-white/70 hover:text-white"
            onClick={() => remove(t.id)}
            aria-label="关闭"
          >
            ×
          </button>
        </div>
      ))}
    </div>
  );
}
