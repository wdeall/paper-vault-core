// 引用导出工具
import { useState } from "react";
import { Download, Copy, Loader2, FileText } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Textarea } from "@/components/ui/textarea";
import { useUIStore } from "@/stores/ui";
import { usePaperStore } from "@/stores/paper";
import { api } from "@/lib/api";
import { copyToClipboard } from "@/lib/utils";

interface Props {
  selectedIds: string[];
  onClearSelection: () => void;
}

export function ExportPanel({ selectedIds, onClearSelection }: Props) {
  const showToast = useUIStore((s) => s.showToast);
  const papers = usePaperStore((s) => s.papers);
  const [output, setOutput] = useState("");
  const [busy, setBusy] = useState(false);
  // 显式选择非空时使用 selectedIds；否则根据 exportAll 导出全部
  const [exportAll, setExportAll] = useState(selectedIds.length === 0);

  async function handleBibtex() {
    const exportIds = selectedIds.length > 0
      ? selectedIds
      : exportAll
        ? papers.map((p) => p.id)
        : [];
    if (exportIds.length === 0) {
      showToast("warning", "暂无论文可导出");
      return;
    }
    setBusy(true);
    try {
      const text = await api.exportBibtex(exportIds);
      setOutput(text);
      showToast("success", `已导出 ${exportIds.length} 条 BibTeX`);
    } catch (e) {
      showToast("error", `导出失败: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleMarkdown() {
    const exportIds = selectedIds.length > 0
      ? selectedIds
      : exportAll
        ? papers.map((p) => p.id)
        : [];
    if (exportIds.length === 0) {
      showToast("warning", "暂无论文可导出");
      return;
    }
    setBusy(true);
    try {
      const text = await api.exportMarkdownCitation(exportIds);
      setOutput(text);
      showToast("success", `已导出 ${exportIds.length} 条 Markdown 引用`);
    } catch (e) {
      showToast("error", `导出失败: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleCopy() {
    if (!output) return;
    await copyToClipboard(output);
    showToast("success", "已复制到剪贴板");
  }

  return (
    <Card className="p-4">
      <div className="mb-3 flex items-center gap-2">
        <FileText className="h-4 w-4" />
        <h2 className="text-lg font-medium">引用导出</h2>
      </div>
      <p className="mb-3 text-xs text-muted-foreground">
        v1 支持 BibTeX 和 Markdown 引用。RIS 留待 v1.5。
        {selectedIds.length > 0 ? `当前选中 ${selectedIds.length} 篇。` : ""}
      </p>
      <div className="mb-3 flex items-center gap-2 text-xs">
        <label className="flex items-center gap-1">
          <input
            type="checkbox"
            checked={exportAll}
            onChange={(e) => setExportAll(e.target.checked)}
          />
          导出全部论文（{papers.length} 篇）
        </label>
      </div>
      <div className="mb-3 flex flex-wrap gap-2">
        <Button onClick={handleBibtex} disabled={busy}>
          {busy ? <Loader2 className="mr-1.5 h-4 w-4 animate-spin" /> : <Download className="mr-1.5 h-4 w-4" />}
          导出 BibTeX
        </Button>
        <Button onClick={handleMarkdown} disabled={busy} variant="outline">
          导出 Markdown 引用
        </Button>
        {selectedIds.length > 0 && (
          <Button onClick={onClearSelection} variant="ghost" size="sm">
            清除选择
          </Button>
        )}
      </div>
      {output && (
        <div>
          <div className="mb-1 flex items-center justify-between">
            <Label className="text-xs">输出</Label>
            <Button size="sm" variant="ghost" onClick={handleCopy}>
              <Copy className="mr-1.5 h-3.5 w-3.5" />
              复制
            </Button>
          </div>
          <Textarea rows={10} value={output} readOnly className="font-mono text-xs" />
        </div>
      )}
    </Card>
  );
}

function Label({ children, className }: { children: React.ReactNode; className?: string }) {
  return <label className={className}>{children}</label>;
}
