// 左侧集合树 + 快捷筛选
import { useState } from "react";
import {
  ChevronRight,
  Folder,
  FolderPlus,
  Tag as TagIcon,
  Hash,
  Bookmark,
  CheckCircle2,
  Clock,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { usePaperStore } from "@/stores/paper";
import { useUIStore } from "@/stores/ui";
import { cn } from "@/lib/utils";
import type { PaperStatus } from "@/types";

// P0: status 改用英文枚举（后端持久化值），标签用于显示。
const STATUS_OPTIONS: ReadonlyArray<{
  value: PaperStatus | null;
  label: string;
  icon: typeof Bookmark;
}> = [
  { value: null, label: "全部", icon: Bookmark },
  { value: "unread", label: "未读", icon: Clock },
  { value: "reading", label: "阅读中", icon: Bookmark },
  { value: "read", label: "已读", icon: CheckCircle2 },
];

const SMART_VIEWS = [
  { key: "all", label: "全部论文" },
  { key: "recent", label: "最近阅读" },
  { key: "modified", label: "最近修改" },
] as const;

export function CollectionsPane() {
  const collections = usePaperStore((s) => s.collections);
  const activeCollectionId = usePaperStore((s) => s.activeCollectionId);
  const statusFilter = usePaperStore((s) => s.statusFilter);
  const smartView = usePaperStore((s) => s.smartView);
  const setActiveCollection = usePaperStore((s) => s.setActiveCollection);
  const setStatusFilter = usePaperStore((s) => s.setStatusFilter);
  const setSmartView = usePaperStore((s) => s.setSmartView);
  const addCollection = usePaperStore((s) => s.addCollection);
  const showToast = useUIStore((s) => s.showToast);
  const [newName, setNewName] = useState("");
  const [showAdd, setShowAdd] = useState(false);

  async function handleAdd() {
    const name = newName.trim();
    if (!name) return;
    try {
      await addCollection(name);
      setNewName("");
      setShowAdd(false);
    } catch (e) {
      showToast("error", `创建失败: ${(e as Error).message}`);
    }
  }

  return (
    <div className="p-2 text-sm">
      <div className="mb-2 px-2 pt-1 text-xs font-medium text-muted-foreground">
        智能视图
      </div>
      {SMART_VIEWS.map((v) => (
        <button
          key={v.key}
          onClick={() => {
            setSmartView(v.key);
            setStatusFilter(null);
            setActiveCollection(null);
          }}
          className={cn(
            "flex w-full items-center gap-2 rounded px-2 py-1.5 text-left hover:bg-accent",
            smartView === v.key && !statusFilter && !activeCollectionId
              ? "bg-accent"
              : "",
          )}
        >
          <Folder className="h-4 w-4" />
          {v.label}
        </button>
      ))}

      <div className="mb-2 mt-4 flex items-center justify-between px-2 pt-1 text-xs font-medium text-muted-foreground">
        <span>阅读状态</span>
      </div>
      {STATUS_OPTIONS.map((opt) => {
        const Icon = opt.icon;
        return (
          <button
            key={opt.label}
            onClick={() => {
              setStatusFilter(opt.value);
              setActiveCollection(null);
            }}
            className={cn(
              "flex w-full items-center gap-2 rounded px-2 py-1.5 text-left hover:bg-accent",
              statusFilter === opt.value ? "bg-accent" : "",
            )}
          >
            <Icon className="h-4 w-4" />
            {opt.label}
          </button>
        );
      })}

      <div className="mb-2 mt-4 flex items-center justify-between px-2 pt-1 text-xs font-medium text-muted-foreground">
        <span>集合</span>
        <button
          onClick={() => setShowAdd((s) => !s)}
          className="rounded p-0.5 hover:bg-accent"
          title="新建集合"
        >
          <FolderPlus className="h-3.5 w-3.5" />
        </button>
      </div>

      {showAdd && (
        <div className="mb-2 flex gap-1 px-2">
          <Input
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") void handleAdd();
              if (e.key === "Escape") setShowAdd(false);
            }}
            placeholder="集合名"
            className="h-7 text-xs"
            autoFocus
          />
          <Button size="sm" variant="ghost" onClick={handleAdd}>
            +
          </Button>
        </div>
      )}

      {collections.length === 0 ? (
        <div className="px-2 py-2 text-xs text-muted-foreground">暂无集合</div>
      ) : (
        collections.map((c) => (
          <button
            key={c.id}
            onClick={() => {
              setActiveCollection(c.id);
              setStatusFilter(null);
            }}
            className={cn(
              "flex w-full items-center gap-2 rounded px-2 py-1.5 text-left hover:bg-accent",
              activeCollectionId === c.id ? "bg-accent" : "",
            )}
          >
            <ChevronRight className="h-3 w-3 text-muted-foreground" />
            <Folder className="h-4 w-4" />
            {c.name}
          </button>
        ))
      )}

      <div className="mt-4 px-2 text-xs text-muted-foreground">
        <div className="mb-1 flex items-center gap-1">
          <TagIcon className="h-3 w-3" />
          标签和关键词通过论文详情面板管理
        </div>
        <div className="flex items-center gap-1">
          <Hash className="h-3 w-3" />
          v1.5 将支持按标签和关键词筛选
        </div>
      </div>
    </div>
  );
}
