// P1: 按 identifier（DOI / arXiv / PMID / ISBN）导入对话框。
// 输入支持纯标识符或 URL；后端会自动解析并调对应 resolver。
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import { isTauri } from "@/lib/tauri";
import { usePaperStore } from "@/stores/paper";
import { useUIStore } from "@/stores/ui";

interface ImportByIdDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function ImportByIdDialog({ open, onOpenChange }: ImportByIdDialogProps) {
  const [raw, setRaw] = useState("");
  const [busy, setBusy] = useState(false);
  const showToast = useUIStore((s) => s.showToast);
  const loadPapers = usePaperStore((s) => s.loadPapers);

  async function handleSubmit() {
    const value = raw.trim();
    if (!value) {
      showToast("warning", "请输入 identifier");
      return;
    }
    if (!isTauri()) {
      showToast("warning", "请在 Tauri 桌面应用中导入");
      return;
    }
    setBusy(true);
    try {
      const result = await api.importByIdentifier(value);
      await loadPapers();
      if (result.duplicates.length > 0) {
        showToast(
          "warning",
          `已导入《${result.paper.title || value}》，但发现 ${result.duplicates.length} 篇疑似重复`,
        );
      } else {
        showToast("success", `已导入《${result.paper.title || value}》`);
      }
      setRaw("");
      onOpenChange(false);
    } catch (e) {
      showToast("error", `导入失败: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  }

  if (!open) return null;
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-[420px] rounded-lg border border-border bg-card p-5 shadow-lg">
        <div className="mb-3 text-base font-semibold">按 ID 导入</div>
        <p className="mb-3 text-xs text-muted-foreground">
          支持 DOI / arXiv / PMID / ISBN，可直接粘贴整段 URL。
        </p>
        <Input
          autoFocus
          placeholder="例如：10.1109/CVPR.2020.01234 或 https://arxiv.org/abs/2401.01234"
          value={raw}
          onChange={(e) => setRaw(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !busy) handleSubmit();
            if (e.key === "Escape") onOpenChange(false);
          }}
          disabled={busy}
        />
        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={() => onOpenChange(false)} disabled={busy}>
            取消
          </Button>
          <Button size="sm" onClick={handleSubmit} disabled={busy}>
            {busy ? "导入中…" : "导入"}
          </Button>
        </div>
      </div>
    </div>
  );
}
