// 顶部工具栏：导入 / 搜索 / 打开设置
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { Settings, FilePlus2, Hash, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import { isTauri } from "@/lib/tauri";
import { usePaperStore } from "@/stores/paper";
import { useSearchStore } from "@/stores/search";
import { useUIStore } from "@/stores/ui";
import { ImportByIdDialog } from "@/components/library/ImportByIdDialog";

export function TopBar() {
  const navigate = useNavigate();
  const importPapers = usePaperStore((s) => s.importPdfs);
  const showToast = useUIStore((s) => s.showToast);
  // M-C P3：TopBar 仅作为全文搜索快捷入口，复用 fulltextQuery 字段
  const setFulltextQuery = useSearchStore((s) => s.setFulltextQuery);
  const setMode = useSearchStore((s) => s.setMode);
  const fulltextQuery = useSearchStore((s) => s.fulltextQuery);
  const runSearch = useSearchStore((s) => s.run);
  const [importing, setImporting] = useState(false);
  const [idDialogOpen, setIdDialogOpen] = useState(false);

  async function handleImport() {
    if (!isTauri()) {
      showToast("warning", "请在 Tauri 桌面应用中导入 PDF");
      return;
    }
    try {
      const selected = await open({
        multiple: true,
        directory: false,
        filters: [{ name: "PDF", extensions: ["pdf"] }],
        title: "选择要导入的 PDF",
      });
      if (!selected) return;
      const paths = Array.isArray(selected) ? selected : [selected];
      if (paths.length === 0) return;
      setImporting(true);
      const results = await importPapers(paths);
      const dupCount = results.filter((r) => r.duplicates.length > 0).length;
      if (dupCount > 0) {
        showToast("warning", `已导入 ${results.length} 篇，${dupCount} 篇疑似重复`);
      } else {
        showToast("success", `已导入 ${results.length} 篇论文`);
      }
    } catch (e) {
      showToast("error", `导入失败: ${(e as Error).message}`);
    } finally {
      setImporting(false);
    }
  }

  async function handleSeed() {
    if (!isTauri()) {
      showToast("warning", "请在 Tauri 桌面应用中加载示例数据");
      return;
    }
    try {
      const ids = await api.loadSeedData();
      showToast("success", `已创建 ${ids.length} 篇示例论文`);
      await usePaperStore.getState().loadPapers();
    } catch (e) {
      showToast("error", `加载示例失败: ${(e as Error).message}`);
    }
  }

  // TopBar 搜索：固定为 fulltext 模式，触发后跳转到 library 页面
  async function handleSearch() {
    if (!fulltextQuery.trim()) return;
    setMode("fulltext");
    try {
      await runSearch();
      navigate("/library");
    } catch (e) {
      showToast("error", `搜索失败: ${(e as Error).message}`);
    }
  }

  return (
    <header className="flex h-12 shrink-0 items-center gap-3 border-b border-border bg-card px-4">
      <div className="text-sm font-semibold">PaperVault</div>
      <Button
        variant="default"
        size="sm"
        onClick={handleImport}
        disabled={importing}
      >
        <FilePlus2 className="mr-1.5 h-4 w-4" />
        {importing ? "导入中…" : "导入 PDF"}
      </Button>
      <Button
        variant="outline"
        size="sm"
        onClick={() => setIdDialogOpen(true)}
        title="按 DOI / arXiv / PMID / ISBN 导入"
      >
        <Hash className="mr-1.5 h-4 w-4" />
        按 ID 导入
      </Button>
      <Button variant="ghost" size="sm" onClick={handleSeed}>
        示例数据
      </Button>
      <div className="ml-4 flex-1 max-w-xl">
        <div className="relative">
          <Search className="absolute left-2 top-2.5 h-4 w-4 text-muted-foreground" />
          <Input
            value={fulltextQuery}
            onChange={(e) => setFulltextQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleSearch();
            }}
            placeholder="全文搜索 标题 / 作者 / DOI / 关键词 / 摘要 / 出处"
            className="pl-8"
          />
        </div>
      </div>
      <Button
        variant="ghost"
        size="icon"
        onClick={() => navigate("/settings")}
        title="设置"
      >
        <Settings className="h-4 w-4" />
      </Button>
      <ImportByIdDialog open={idDialogOpen} onOpenChange={setIdDialogOpen} />
    </header>
  );
}
