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
            "min-w-72 max-w-md cursor-pointer rounded-md px-4 py-3 text-sm text-white shadow-lg",
            COLORS[t.type],
          )}
          onClick={() => remove(t.id)}
        >
          {t.message}
        </div>
      ))}
    </div>
  );
}
