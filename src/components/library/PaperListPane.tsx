// 中间论文列表 + 搜索 + 排序
import { useEffect, useMemo, useState } from "react";
import { FileText, Loader2 } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { Card } from "@/components/ui/card";
import { usePaperStore } from "@/stores/paper";
import { useSearchStore } from "@/stores/search";
import { useUIStore } from "@/stores/ui";
import { cn, formatAuthors, formatDate } from "@/lib/utils";
import { PAPER_STATUS_LABELS, type Paper } from "@/types";

type SortKey = "updated_at" | "created_at" | "year" | "title";

export function PaperListPane() {
  const papers = usePaperStore((s) => s.papers);
  const selectedPaperId = usePaperStore((s) => s.selectedPaperId);
  const selectPaper = usePaperStore((s) => s.selectPaper);
  const statusFilter = usePaperStore((s) => s.statusFilter);
  const activeCollectionId = usePaperStore((s) => s.activeCollectionId);
  const smartView = usePaperStore((s) => s.smartView);
  const loadPapers = usePaperStore((s) => s.loadPapers);
  const showToast = useUIStore((s) => s.showToast);
  const searchHits = useSearchStore((s) => s.hitResults);
  const [localQuery, setLocalQuery] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("updated_at");
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setLoading(true);
    loadPapers()
      .catch((e) => showToast("error", `加载失败: ${(e as Error).message}`))
      .finally(() => setLoading(false));
  }, [statusFilter, activeCollectionId, smartView, loadPapers, showToast]);

  const filtered = useMemo(() => {
    const list = localQuery.trim()
      ? papers.filter((p) =>
          (p.title + p.authors.join(" ") + p.abstract_text + p.keywords.join(" "))
            .toLowerCase()
            .includes(localQuery.toLowerCase()),
        )
      : papers;
    return [...list].sort((a, b) => {
      switch (sortKey) {
        case "title":
          return a.title.localeCompare(b.title);
        case "year":
          return (b.year ?? 0) - (a.year ?? 0);
        case "created_at":
          return b.created_at - a.created_at;
        case "updated_at":
        default:
          return b.updated_at - a.updated_at;
      }
    });
  }, [papers, localQuery, sortKey]);

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-border bg-card p-2">
        <Input
          value={localQuery}
          onChange={(e) => setLocalQuery(e.target.value)}
          placeholder="在结果中过滤…"
          className="h-8"
        />
        <select
          value={sortKey}
          onChange={(e) => setSortKey(e.target.value as SortKey)}
          className="h-8 rounded border border-input bg-background px-2 text-xs"
        >
          <option value="updated_at">最近修改</option>
          <option value="created_at">最近添加</option>
          <option value="year">年份</option>
          <option value="title">标题</option>
        </select>
        <div className="text-xs text-muted-foreground">{filtered.length} 篇</div>
      </div>

      {searchHits.length > 0 && (
        <div className="shrink-0 border-b border-border bg-muted/40 p-2 text-xs">
          <div className="mb-1 text-muted-foreground">
            全文搜索命中 {searchHits.length} 条
          </div>
        </div>
      )}

      <div className="flex-1 overflow-y-auto">
        {loading ? (
          <div className="flex h-32 items-center justify-center text-muted-foreground">
            <Loader2 className="mr-2 h-4 w-4 animate-spin" />
            加载中…
          </div>
        ) : filtered.length === 0 ? (
          <div className="flex h-32 flex-col items-center justify-center gap-2 text-muted-foreground">
            <FileText className="h-8 w-8 opacity-50" />
            <div className="text-sm">暂无论文</div>
            <div className="text-xs">点击右上角“导入 PDF”开始</div>
          </div>
        ) : (
          <ul>
            {filtered.map((p) => (
              <li key={p.id}>
                <PaperRow
                  paper={p}
                  selected={p.id === selectedPaperId}
                  onClick={() => selectPaper(p.id)}
                />
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}

function PaperRow({
  paper,
  selected,
  onClick,
}: {
  paper: Paper;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <Card
      onClick={onClick}
      className={cn(
        "m-2 cursor-pointer border-border p-3 transition-colors hover:bg-accent",
        selected && "border-primary bg-accent",
      )}
    >
      <div className="mb-1 line-clamp-2 text-sm font-medium">{paper.title || "（无标题）"}</div>
      <div className="mb-1 text-xs text-muted-foreground">
        {formatAuthors(paper.authors)} · {paper.year ?? "—"} · {paper.venue || "—"}
      </div>
      <div className="flex flex-wrap items-center gap-1">
        <Badge variant="secondary" className="text-[10px]">
          {PAPER_STATUS_LABELS[paper.status]}
        </Badge>
        {paper.keywords.slice(0, 3).map((k) => (
          <Badge key={k} variant="outline" className="text-[10px]">
            {k}
          </Badge>
        ))}
      </div>
      <div className="mt-1 text-[10px] text-muted-foreground">
        {formatDate(paper.updated_at)}
      </div>
    </Card>
  );
}
