// 搜索面板
import { useState } from "react";
import { Search } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { useSearchStore } from "@/stores/search";
import { useUIStore } from "@/stores/ui";
import { SearchResults } from "./SearchResults";

export function SearchPanel() {
  const query = useSearchStore((s) => s.query);
  const setQuery = useSearchStore((s) => s.setQuery);
  const run = useSearchStore((s) => s.run);
  const showToast = useUIStore((s) => s.showToast);
  const [open, setOpen] = useState(false);

  async function handleRun() {
    if (!query.trim()) return;
    try {
      await run();
      setOpen(true);
    } catch (e) {
      showToast("error", `搜索失败: ${(e as Error).message}`);
    }
  }

  return (
    <div>
      <Card className="m-3 flex items-center gap-2 border-border bg-card p-2">
        <Search className="h-4 w-4 text-muted-foreground" />
        <Input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") void handleRun();
          }}
          placeholder="搜索 标题 / 作者 / DOI / 关键词 / 摘要 / 笔记 / PDF"
          className="h-7 border-0 shadow-none focus-visible:ring-0"
        />
        <Button size="sm" onClick={handleRun} disabled={!query.trim()}>
          搜索
        </Button>
      </Card>
      {open && <SearchResults onClose={() => setOpen(false)} />}
    </div>
  );
}
