// 引用导出工具
import { useState } from "react";
import { Download, Copy, Loader2, FileText, Save } from "lucide-react";
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
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

// M-E P5：标准化导出格式 - 支持 BibTeX / RIS / CSL-JSON / Markdown
type ExportFormat = "bibtex" | "ris" | "csl" | "markdown";

const FORMATS: { key: ExportFormat; label: string; ext: string }[] = [
  { key: "bibtex", label: "BibTeX", ext: "bib" },
  { key: "ris", label: "RIS", ext: "ris" },
  { key: "csl", label: "CSL-JSON", ext: "json" },
  { key: "markdown", label: "Markdown", ext: "md" },
];

export function ExportPanel({ selectedIds, onClearSelection }: Props) {
  const showToast = useUIStore((s) => s.showToast);
  const papers = usePaperStore((s) => s.papers);
  const [output, setOutput] = useState("");
  const [busy, setBusy] = useState<ExportFormat | null>(null);
  // 显式选择非空时使用 selectedIds；否则根据 exportAll 导出全部
  const [exportAll, setExportAll] = useState(selectedIds.length === 0);

  // 统一导出函数：根据格式调用对应后端命令，返回导出文本
  async function doExport(format: ExportFormat): Promise<string> {
    const exportIds = selectedIds.length > 0
      ? selectedIds
      : exportAll
        ? papers.map((p) => p.id)
        : [];
    if (exportIds.length === 0) {
      showToast("warning", "暂无论文可导出");
      throw new Error("no papers");
    }
    switch (format) {
      case "bibtex":
        return api.exportBibtex(exportIds);
      case "ris":
        return api.exportRis(exportIds);
      case "csl":
        return api.exportCslJson(exportIds);
      case "markdown":
        return api.exportMarkdownCitation(exportIds);
    }
  }

  // 点击格式按钮：导出到输出框
  async function handleExport(format: ExportFormat) {
    setBusy(format);
    try {
      const text = await doExport(format);
      setOutput(text);
      const label = FORMATS.find((f) => f.key === format)?.label ?? format;
      const count = selectedIds.length > 0 ? selectedIds.length : papers.length;
      showToast("success", `已导出 ${count} 条 ${label}`);
    } catch (e) {
      // doExport 内部已对 "暂无论文可导出" 提示，这里只处理其他错误
      if ((e as Error).message !== "no papers") {
        showToast("error", `导出失败: ${(e as Error).message}`);
      }
    } finally {
      setBusy(null);
    }
  }

  // 保存到文件：调用 Tauri save 对话框 + writeTextFile
  async function handleSave(format: ExportFormat) {
    setBusy(format);
    try {
      const text = await doExport(format);
      const ext = FORMATS.find((f) => f.key === format)?.ext ?? "txt";
      const path = await save({
        defaultPath: `papers.${ext}`,
        filters: [{ name: format.toUpperCase(), extensions: [ext] }],
      });
      if (path) {
        await writeTextFile(path, text);
        showToast("success", `已保存到 ${path}`);
      }
    } catch (e) {
      if ((e as Error).message !== "no papers") {
        showToast("error", `保存失败: ${(e as Error).message}`);
      }
    } finally {
      setBusy(null);
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
        v2 支持 BibTeX / RIS / CSL-JSON 三种标准格式 + Markdown 引用。
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
        {FORMATS.map((f) => {
          const isBusy = busy === f.key;
          return (
            <div key={f.key} className="flex items-center gap-1">
              <Button
                onClick={() => handleExport(f.key)}
                disabled={busy !== null}
                variant={f.key === "bibtex" ? "default" : "outline"}
              >
                {isBusy ? (
                  <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
                ) : (
                  <Download className="mr-1.5 h-4 w-4" />
                )}
                {f.label}
              </Button>
              <Button
                onClick={() => handleSave(f.key)}
                disabled={busy !== null}
                variant="ghost"
                size="icon"
                title={`保存 ${f.label} 到文件`}
              >
                {isBusy ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Save className="h-4 w-4" />
                )}
              </Button>
            </div>
          );
        })}
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
