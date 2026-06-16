// P2：重复合并确认对话框。
// 显示 src vs dst 字段差异 + 合并后字段预览 + "应用合并" 按钮。
// 应用后弹 toast 提示可在 5 分钟内撤销。
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import { isTauri } from "@/lib/tauri";
import { usePaperStore } from "@/stores/paper";
import { useUIStore } from "@/stores/ui";
import type { Paper } from "@/types";

interface MergeDialogProps {
  src: Paper;
  dst: Paper;
  onConfirm: () => void;
  onCancel: () => void;
}

const UNDO_WINDOW_SEC = 5 * 60;

function pickDiff(src: Paper, dst: Paper) {
  const fields: Array<{
    name: string;
    src: string;
    dst: string;
    resolved: string;
  }> = [];

  const rows: [string, string, string][] = [
    ["title", src.title, dst.title],
    ["year", src.year ? String(src.year) : "", dst.year ? String(dst.year) : ""],
    ["venue", src.venue, dst.venue],
    [
      "authors",
      (src.authors ?? []).join("; "),
      (dst.authors ?? []).join("; "),
    ],
    [
      "keywords",
      (src.keywords ?? []).join("; "),
      (dst.keywords ?? []).join("; "),
    ],
  ];

  for (const [name, s, d] of rows) {
    if (s === d) {
      // 完全相同：仍列出以提示用户
      fields.push({ name, src: s, dst: d, resolved: d });
      continue;
    }
    if (d.trim() === "") {
      fields.push({ name, src: s, dst: d, resolved: s });
    } else if (s.trim() === "") {
      fields.push({ name, src: s, dst: d, resolved: d });
    } else {
      // 都非空：dst 优先（保留）
      fields.push({ name, src: s, dst: d, resolved: d });
    }
  }
  return fields;
}

export function MergeDialog({ src, dst, onConfirm, onCancel }: MergeDialogProps) {
  const loadPapers = usePaperStore((s) => s.loadPapers);
  const showToast = useUIStore((s) => s.showToast);
  const [busy, setBusy] = useState(false);
  const diffs = pickDiff(src, dst);

  async function handleConfirm() {
    if (!isTauri()) {
      showToast("warning", "请在 Tauri 桌面应用中合并");
      return;
    }
    setBusy(true);
    try {
      const r = await api.mergePapers(src.id, dst.id);
      await loadPapers();
      showToast(
        "success",
        `已合并到《${dst.title || src.title}》`,
        // 撤销按钮
        {
          label: "撤销",
          onClick: async () => {
            try {
              await api.undoMerge(r.merge_id);
              await loadPapers();
              showToast("info", "已撤销合并");
            } catch (e) {
              showToast("error", `撤销失败: ${(e as Error).message}`);
            }
          },
          ttlSec: UNDO_WINDOW_SEC,
        },
      );
      onConfirm();
    } catch (e) {
      showToast("error", `合并失败: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50">
      <div className="w-[640px] max-h-[80vh] overflow-y-auto rounded-lg border border-border bg-card p-5 shadow-lg">
        <div className="mb-2 text-base font-semibold">合并重复论文</div>
        <p className="mb-3 text-xs text-muted-foreground">
          重复论文合并后会按字段级策略（dst 优先，仅在 dst 为空时使用 src 补齐；keywords/creators/attachments/annotations 走并集）。
          可在 5 分钟内撤销。
        </p>

        <div className="mb-3 grid grid-cols-2 gap-2 text-xs">
          <div className="rounded border border-border bg-background p-2">
            <div className="mb-1 text-[10px] text-muted-foreground">src（将删除）</div>
            <div className="font-medium">{src.title || "（无标题）"}</div>
            <div className="text-muted-foreground">
              {(src.authors ?? []).join(", ")} · {src.year ?? "—"}
            </div>
          </div>
          <div className="rounded border border-border bg-background p-2">
            <div className="mb-1 text-[10px] text-muted-foreground">dst（保留）</div>
            <div className="font-medium">{dst.title || "（无标题）"}</div>
            <div className="text-muted-foreground">
              {(dst.authors ?? []).join(", ")} · {dst.year ?? "—"}
            </div>
          </div>
        </div>

        <table className="w-full border-collapse text-xs">
          <thead>
            <tr className="border-b border-border bg-muted/40 text-left text-[10px] uppercase text-muted-foreground">
              <th className="px-2 py-1">字段</th>
              <th className="px-2 py-1">src</th>
              <th className="px-2 py-1">dst</th>
              <th className="px-2 py-1">合并后</th>
            </tr>
          </thead>
          <tbody>
            {diffs.map((d) => (
              <tr key={d.name} className="border-b border-border">
                <td className="px-2 py-1 font-mono">{d.name}</td>
                <td className="px-2 py-1 align-top">
                  <div className="line-clamp-3 max-w-[180px] break-words text-muted-foreground">
                    {d.src || <span className="italic">（空）</span>}
                  </div>
                </td>
                <td className="px-2 py-1 align-top">
                  <div className="line-clamp-3 max-w-[180px] break-words">
                    {d.dst || <span className="italic text-muted-foreground">（空）</span>}
                  </div>
                </td>
                <td className="px-2 py-1 align-top font-medium">
                  {d.resolved || <span className="italic text-muted-foreground">（空）</span>}
                </td>
              </tr>
            ))}
          </tbody>
        </table>

        <div className="mt-4 flex justify-end gap-2">
          <Button variant="ghost" size="sm" onClick={onCancel} disabled={busy}>
            取消
          </Button>
          <Button size="sm" onClick={handleConfirm} disabled={busy}>
            {busy ? "合并中…" : "应用合并"}
          </Button>
        </div>
      </div>
    </div>
  );
}
