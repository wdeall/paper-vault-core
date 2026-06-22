// 搜索面板 - M-C P3 双通道搜索
// 支持三模式切换：结构化 / 全文 / 双通道
import { useState } from "react";
import { Search } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { useSearchStore, type SearchMode } from "@/stores/search";
import { useUIStore } from "@/stores/ui";
import { SearchResults } from "./SearchResults";

// 模式切换标签
const MODE_OPTIONS: { value: SearchMode; label: string }[] = [
  { value: "structured", label: "结构化" },
  { value: "fulltext", label: "全文" },
  { value: "both", label: "双通道" },
];

// status 选项（中文标签）
const STATUS_OPTIONS = [
  { value: "", label: "全部" },
  { value: "unread", label: "未读" },
  { value: "reading", label: "阅读中" },
  { value: "read", label: "已读" },
];

export function SearchPanel() {
  const mode = useSearchStore((s) => s.mode);
  const setMode = useSearchStore((s) => s.setMode);
  const structured = useSearchStore((s) => s.structured);
  const setStructured = useSearchStore((s) => s.setStructured);
  const fulltextQuery = useSearchStore((s) => s.fulltextQuery);
  const setFulltextQuery = useSearchStore((s) => s.setFulltextQuery);
  const run = useSearchStore((s) => s.run);
  const showToast = useUIStore((s) => s.showToast);
  const [open, setOpen] = useState(false);

  async function handleRun() {
    try {
      await run();
      setOpen(true);
    } catch (e) {
      showToast("error", `搜索失败: ${(e as Error).message}`);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      void handleRun();
    }
  }

  // 判断是否可搜索：structured 需至少一个字段非空；fulltext 需非空字符串；both 两者皆需
  function canSearch(): boolean {
    if (mode === "fulltext") return fulltextQuery.trim().length > 0;
    if (mode === "structured") {
      return Object.values(structured).some((v) => v !== null && v !== "");
    }
    // both：结构化条件或全文任一非空即可
    return (
      Object.values(structured).some((v) => v !== null && v !== "") ||
      fulltextQuery.trim().length > 0
    );
  }

  return (
    <Card className="m-3 flex flex-col gap-2 border-border bg-card p-3">
      {/* 模式切换 */}
      <div className="flex items-center gap-1 rounded-md border border-border p-1">
        {MODE_OPTIONS.map((opt) => (
          <button
            key={opt.value}
            onClick={() => setMode(opt.value)}
            className={
              "flex-1 rounded px-3 py-1 text-xs font-medium transition-colors " +
              (mode === opt.value
                ? "bg-primary text-primary-foreground"
                : "text-muted-foreground hover:bg-accent")
            }
          >
            {opt.label}
          </button>
        ))}
      </div>

      {/* 条件展开 */}
      {(mode === "structured" || mode === "both") && (
        <div className="grid grid-cols-2 gap-2">
          <FieldLabel label="标题">
            <Input
              value={structured.title ?? ""}
              onChange={(e) => setStructured({ title: e.target.value || null })}
              onKeyDown={handleKeyDown}
              placeholder="模糊匹配标题"
              className="h-8"
            />
          </FieldLabel>
          <FieldLabel label="作者">
            <Input
              value={structured.author ?? ""}
              onChange={(e) => setStructured({ author: e.target.value || null })}
              onKeyDown={handleKeyDown}
              placeholder="模糊匹配作者"
              className="h-8"
            />
          </FieldLabel>
          <FieldLabel label="年份">
            <Input
              type="number"
              value={structured.year ?? ""}
              onChange={(e) =>
                setStructured({
                  year: e.target.value ? Number(e.target.value) : null,
                })
              }
              onKeyDown={handleKeyDown}
              placeholder="精确匹配年份"
              className="h-8"
            />
          </FieldLabel>
          <FieldLabel label="出处">
            <Input
              value={structured.venue ?? ""}
              onChange={(e) => setStructured({ venue: e.target.value || null })}
              onKeyDown={handleKeyDown}
              placeholder="模糊匹配出处"
              className="h-8"
            />
          </FieldLabel>
          <FieldLabel label="DOI">
            <Input
              value={structured.doi ?? ""}
              onChange={(e) => setStructured({ doi: e.target.value || null })}
              onKeyDown={handleKeyDown}
              placeholder="精确匹配 DOI"
              className="h-8"
            />
          </FieldLabel>
          <FieldLabel label="状态">
            <select
              value={structured.status ?? ""}
              onChange={(e) =>
                setStructured({ status: e.target.value || null })
              }
              className="h-8 w-full rounded-md border border-input bg-transparent px-2 text-sm"
            >
              {STATUS_OPTIONS.map((opt) => (
                <option key={opt.value} value={opt.value}>
                  {opt.label}
                </option>
              ))}
            </select>
          </FieldLabel>
          <FieldLabel label="关键词" className="col-span-2">
            <Input
              value={structured.keyword ?? ""}
              onChange={(e) =>
                setStructured({ keyword: e.target.value || null })
              }
              onKeyDown={handleKeyDown}
              placeholder="模糊匹配关键词"
              className="h-8"
            />
          </FieldLabel>
        </div>
      )}

      {(mode === "fulltext" || mode === "both") && (
        <div className="flex items-center gap-2">
          <Search className="h-4 w-4 shrink-0 text-muted-foreground" />
          <Input
            value={fulltextQuery}
            onChange={(e) => setFulltextQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="全文搜索 标题 / 作者 / DOI / 关键词 / 摘要 / 出处"
            className="h-8"
          />
        </div>
      )}

      <Button
        size="sm"
        onClick={handleRun}
        disabled={!canSearch()}
        className="self-end"
      >
        搜索
      </Button>

      {open && <SearchResults onClose={() => setOpen(false)} />}
    </Card>
  );
}

function FieldLabel({
  label,
  children,
  className,
}: {
  label: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <label className={"flex flex-col gap-1 " + (className ?? "")}>
      <span className="text-xs text-muted-foreground">{label}</span>
      {children}
    </label>
  );
}
