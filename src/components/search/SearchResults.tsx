// 搜索结果弹窗 - M-C P3 双通道搜索
// 适配两种结果类型：SearchHit（fulltext） / PaperSummary（structured / both）
import { useNavigate } from "react-router-dom";
import { Search, Loader2 } from "lucide-react";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { useSearchStore } from "@/stores/search";
import { usePaperStore } from "@/stores/paper";
import { PAPER_STATUS_LABELS, type PaperStatus } from "@/types";

// SearchHit 来源标签（移除 notes / pdf，新增 venue）
const SOURCE_LABELS: Record<string, { label: string; color: string }> = {
  title: { label: "标题", color: "bg-blue-500" },
  authors: { label: "作者", color: "bg-purple-500" },
  doi: { label: "DOI", color: "bg-pink-500" },
  keywords: { label: "关键词", color: "bg-green-500" },
  abstract: { label: "摘要", color: "bg-yellow-500" },
  venue: { label: "出处", color: "bg-cyan-500" },
};

interface Props {
  onClose: () => void;
}

export function SearchResults({ onClose }: Props) {
  const navigate = useNavigate();
  const mode = useSearchStore((s) => s.mode);
  const fulltextQuery = useSearchStore((s) => s.fulltextQuery);
  const paperResults = useSearchStore((s) => s.paperResults);
  const hitResults = useSearchStore((s) => s.hitResults);
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

  // 头部展示的查询摘要
  const querySummary =
    mode === "fulltext"
      ? fulltextQuery
      : mode === "both"
        ? `双通道: ${fulltextQuery || "(空)"}`
        : "结构化查询";

  const totalCount = paperResults.length + hitResults.length;

  return (
    <div className="fixed inset-0 z-40 flex items-start justify-center bg-black/40 p-8 pt-20">
      <Card className="max-h-[80vh] w-full max-w-3xl overflow-hidden p-0">
        <div className="flex items-center gap-2 border-b border-border p-3">
          <Search className="h-4 w-4 text-muted-foreground" />
          <div className="text-sm">
            搜索: <span className="font-medium">{querySummary}</span>
          </div>
          <div className="ml-auto text-xs text-muted-foreground">
            {totalCount} 条命中
          </div>
          <Button
            size="sm"
            variant="ghost"
            onClick={() => {
              clear();
              onClose();
            }}
          >
            关闭
          </Button>
        </div>
        <div className="max-h-[60vh] overflow-y-auto p-3">
          {isSearching ? (
            <div className="flex items-center gap-2 p-4 text-muted-foreground">
              <Loader2 className="h-4 w-4 animate-spin" />
              搜索中…
            </div>
          ) : totalCount === 0 ? (
            <div className="p-6 text-center text-sm text-muted-foreground">
              无匹配结果
            </div>
          ) : hitResults.length > 0 ? (
            // fulltext 模式：SearchHit 列表
            <ul className="space-y-2">
              {hitResults.map((h, i) => {
                const src =
                  SOURCE_LABELS[h.source_type] ?? {
                    label: h.source_type,
                    color: "bg-gray-500",
                  };
                return (
                  <li key={i}>
                    <button
                      onClick={() => handlePick(h.paper_id)}
                      className="w-full rounded border border-border bg-card p-3 text-left transition-colors hover:bg-accent"
                    >
                      <div className="mb-1 flex items-center gap-2 text-xs">
                        <Badge className="text-[10px]">{src.label}</Badge>
                        {h.page !== null && (
                          <span className="text-muted-foreground">
                            第 {h.page} 页
                          </span>
                        )}
                        <span className="ml-auto text-[10px] text-muted-foreground">
                          score {h.score.toFixed(2)}
                        </span>
                      </div>
                      <div
                        className="line-clamp-3 text-sm"
                        dangerouslySetInnerHTML={{
                          __html: highlight(h.snippet),
                        }}
                      />
                    </button>
                  </li>
                );
              })}
            </ul>
          ) : (
            // structured / both 模式：PaperSummary 列表
            <ul className="space-y-2">
              {paperResults.map((p, i) => (
                <li key={i}>
                  <button
                    onClick={() => handlePick(p.id)}
                    className="w-full rounded border border-border bg-card p-3 text-left transition-colors hover:bg-accent"
                  >
                    <div className="mb-1 line-clamp-2 text-sm font-medium">
                      {p.title || "（无标题）"}
                    </div>
                    <div className="mb-1 text-xs text-muted-foreground">
                      {p.authors.join(", ") || "（无作者）"} ·{" "}
                      {p.year ?? "—"} · {p.venue || "—"}
                    </div>
                    <div className="flex flex-wrap items-center gap-1 text-[10px]">
                      <Badge variant="secondary">
                        {PAPER_STATUS_LABELS[p.status as PaperStatus] ??
                          p.status}
                      </Badge>
                      {p.rating !== null && (
                        <Badge variant="outline">评分 {p.rating}</Badge>
                      )}
                      {p.score !== null && (
                        <span className="ml-auto text-muted-foreground">
                          score {p.score.toFixed(2)}
                        </span>
                      )}
                    </div>
                  </button>
                </li>
              ))}
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
