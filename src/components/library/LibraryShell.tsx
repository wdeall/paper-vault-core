// Library 三栏布局：左集合 / 中论文列表 / 右详情
// 240px | flex 1 | 360px
import { useEffect } from "react";
import { CollectionsPane } from "./CollectionsPane";
import { PaperListPane } from "./PaperListPane";
import { PaperDetailPane } from "./PaperDetailPane";
import { TopBar } from "./TopBar";
import { usePaperStore } from "@/stores/paper";
import { useUIStore } from "@/stores/ui";

export function LibraryShell() {
  const selectedPaperId = usePaperStore((s) => s.selectedPaperId);
  const loadCollections = usePaperStore((s) => s.loadCollections);
  const showToast = useUIStore((s) => s.showToast);

  useEffect(() => {
    loadCollections().catch((e) => {
      showToast("error", `加载集合失败: ${(e as Error).message}`);
    });
  }, [loadCollections, showToast]);

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <TopBar />
      <div className="flex flex-1 overflow-hidden">
        <aside className="w-60 shrink-0 border-r border-border overflow-y-auto">
          <CollectionsPane />
        </aside>
        <main className="flex-1 overflow-y-auto">
          <PaperListPane />
        </main>
        <aside className="w-[360px] shrink-0 border-l border-border overflow-y-auto">
          {selectedPaperId ? (
            <PaperDetailPane paperId={selectedPaperId} />
          ) : (
            <div className="flex h-full items-center justify-center p-6 text-sm text-muted-foreground">
              请从左侧选择一篇论文
            </div>
          )}
        </aside>
      </div>
    </div>
  );
}
