// 搜索结果弹窗
import { useNavigate } from "react-router-dom";
import { Search, Loader2 } from "lucide-react";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useSearchStore } from "@/stores/search";
import { usePaperStore } from "@/stores/paper";

const SOURCE_LABELS: Record<string, { label: string; color: string }> = {
  title: { label: "标题", color: "bg-blue-500" },
  authors: { label: "作者", color: "bg-purple-500" },
  doi: { label: "DOI", color: "bg-pink-500" },
  keywords: { label: "关键词", color: "bg-green-500" },
  abstract: { label: "摘要", color: "bg-yellow-500" },
  notes: { label: "笔记", color: "bg-orange-500" },
  pdf: { label: "PDF", color: "bg-red-500" },
};

interface Props {
  onClose: () => void;
}

export function SearchResults({ onClose }: Props) {
  const navigate = useNavigate();
  const hits = useSearchStore((s) => s.hits);
  const query = useSearchStore((s) => s.query);
  const isSearching = useSearchStore((s) => s.isSearching);
  const clear = useSearchStore((s) => s.clear);
  const selectPaper = usePaperStore((s) => s.selectPaper);

  async function handlePick(paperId: string) {
    selectPaper(paperId);
    // 直接跳到 reader
    navigate(`/reader/${paperId}`);
    onClose();
    clear();
  }

  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center bg-black/40 p-8 pt-20">
      <Card className="max-h-[80vh] w-full max-w-3xl overflow-hidden p-0">
        <div className="flex items-center gap-2 border-b border-border p-3">
          <Search className="h-4 w-4 text-muted-foreground" />
          <div className="text-sm">
            搜索: <span className="font-medium">{query}</span>
          </div>
          <div className="ml-auto text-xs text-muted-foreground">
            {hits.length} 条命中
          </div>
          <Button size="sm" variant="ghost" onClick={() => { clear(); onClose(); }}>
            关闭
          </Button>
        </div>
        <div className="max-h-[60vh] overflow-y-auto p-3">
          {isSearching ? (
            <div className="flex items-center gap-2 p-4 text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              搜索中…
            </div>
          ) : hits.length === 0 ? (
            <div className="p-6 text-center text-sm text-muted-foreground">
              {query.trim() ? "无匹配结果" : "输入关键词开始搜索"}
            </div>
          ) : (
            <ul className="space-y-2">
              {hits.map((h, i) => {
                const src = SOURCE_LABELS[h.source_type] ?? { label: h.source_type, color: "bg-gray-500" };
                return (
                  <li key={i}>
                    <button
                      onClick={() => handlePick(h.paper_id)}
                      className="w-full rounded border border-border bg-card p-3 text-left transition-colors hover:bg-accent"
                    >
                      <div className="mb-1 flex items-center gap-2 text-xs">
                        <Badge className="text-[10px]">
                          {src.label}
                        </Badge>
                        {h.page !== null && (
                          <span className="text-muted-foreground">第 {h.page} 页</span>
                        )}
                        <span className="ml-auto text-[10px] text-muted-foreground">
                          score {h.score.toFixed(2)}
                        </span>
                      </div>
                      <div
                        className="line-clamp-3 text-sm"
                        dangerouslySetInnerHTML={{ __html: highlight(h.snippet) }}
                      />
                    </button>
                  </li>
                );
              })}
            </ul>
          )}
        </div>
      </Card>
    </div>
  );
}

function highlight(snippet: string): string {
  // 后端已经返回带 <mark> 的片段
  return snippet;
}
