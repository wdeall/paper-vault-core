// 批注侧边栏：列表 / 筛选 / 编辑 / 删除 / 同步到笔记
import { useMemo, useState } from "react";
import {
  Trash2,
  Pencil,
  Check,
  X,
  RefreshCw,
  Filter,
  Underline,
  Strikethrough,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { Textarea } from "@/components/ui/textarea";
import { api } from "@/lib/api";
import { useUIStore } from "@/stores/ui";
import { cn } from "@/lib/utils";
import type { Annotation, AnnotationRect } from "@/types";

// 颜色映射（与 PDFViewer 保持一致）
const COLOR_DOT: Record<string, string> = {
  yellow: "bg-yellow-400",
  red: "bg-red-400",
  green: "bg-green-400",
  blue: "bg-blue-400",
  purple: "bg-purple-400",
};

const COLOR_LABEL: Record<string, string> = {
  yellow: "黄",
  red: "红",
  green: "绿",
  blue: "蓝",
  purple: "紫",
};

interface Props {
  paperId: string;
  annotations: Annotation[];
  onAnnotationChange: () => void;
  onJumpToAnnotation: (page: number, rect: AnnotationRect) => void;
}

export function AnnotationSidebar({
  paperId,
  annotations,
  onAnnotationChange,
  onJumpToAnnotation,
}: Props) {
  const showToast = useUIStore((s) => s.showToast);
  const [pageFilter, setPageFilter] = useState<string>("all");
  const [colorFilter, setColorFilter] = useState<string>("all");
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingComment, setEditingComment] = useState<string>("");
  const [savingId, setSavingId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [syncing, setSyncing] = useState(false);

  // 可用页码列表（用于筛选下拉）
  const pageOptions = useMemo(() => {
    const pages = new Set<number>();
    annotations.forEach((a) => {
      if (a.page != null) pages.add(a.page);
    });
    return Array.from(pages).sort((x, y) => x - y);
  }, [annotations]);

  // 过滤后的批注列表（按页 + 按颜色）
  const filtered = useMemo(() => {
    return annotations.filter((a) => {
      if (pageFilter !== "all" && String(a.page) !== pageFilter) return false;
      if (colorFilter !== "all" && a.color !== colorFilter) return false;
      return true;
    });
  }, [annotations, pageFilter, colorFilter]);

  // 点击批注 → 跳转到对应页
  function handleClickAnnotation(a: Annotation) {
    if (a.page != null && a.rect && a.rect.length > 0) {
      onJumpToAnnotation(a.page, a.rect[0]);
    }
  }

  // 进入编辑评论模式
  function startEdit(a: Annotation) {
    setEditingId(a.id);
    setEditingComment(a.comment ?? "");
  }

  function cancelEdit() {
    setEditingId(null);
    setEditingComment("");
  }

  // 保存评论
  async function saveEdit(id: string) {
    setSavingId(id);
    try {
      await api.updateAnnotation({
        id,
        comment: editingComment || null,
      });
      showToast("success", "评论已更新");
      setEditingId(null);
      setEditingComment("");
      onAnnotationChange();
    } catch (e) {
      showToast("error", `更新失败: ${(e as Error).message}`);
    } finally {
      setSavingId(null);
    }
  }

  // 确认删除
  async function confirmDelete() {
    if (!deletingId) return;
    const id = deletingId;
    setDeletingId(null);
    try {
      await api.deleteAnnotation(id);
      showToast("success", "已删除批注");
      onAnnotationChange();
    } catch (e) {
      showToast("error", `删除失败: ${(e as Error).message}`);
    }
  }

  // 同步到笔记
  async function handleSync() {
    setSyncing(true);
    try {
      await api.syncAnnotationsToNote(paperId);
      showToast("success", "已同步批注到笔记");
    } catch (e) {
      showToast("error", `同步失败: ${(e as Error).message}`);
    } finally {
      setSyncing(false);
    }
  }

  return (
    <aside className="flex h-full w-full flex-col border-r border-border bg-card">
      {/* 顶部：筛选 */}
      <div className="shrink-0 border-b border-border p-2">
        <div className="mb-1 flex items-center gap-1 text-xs text-muted-foreground">
          <Filter className="h-3 w-3" />
          筛选
        </div>
        <div className="flex gap-1">
          <select
            value={pageFilter}
            onChange={(e) => setPageFilter(e.target.value)}
            className="h-7 flex-1 rounded border border-input bg-background px-1 text-xs"
            title="按页筛选"
          >
            <option value="all">全部页</option>
            {pageOptions.map((p) => (
              <option key={p} value={String(p)}>
                第 {p} 页
              </option>
            ))}
          </select>
          <select
            value={colorFilter}
            onChange={(e) => setColorFilter(e.target.value)}
            className="h-7 flex-1 rounded border border-input bg-background px-1 text-xs"
            title="按颜色筛选"
          >
            <option value="all">全部颜色</option>
            {Object.entries(COLOR_LABEL).map(([name, label]) => (
              <option key={name} value={name}>
                {label}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* 中部：批注列表 */}
      <div className="flex-1 overflow-y-auto p-2">
        {filtered.length === 0 ? (
          <div className="flex h-full items-center justify-center p-4 text-center text-xs text-muted-foreground">
            {annotations.length === 0 ? "暂无批注" : "无匹配批注"}
          </div>
        ) : (
          <ul className="space-y-2">
            {filtered.map((a) => {
              const isEditing = editingId === a.id;
              return (
                <li
                  key={a.id}
                  className="group rounded-md border border-border bg-background p-2 text-xs transition-colors hover:border-primary/40"
                >
                  {/* 头部：kind 图标 + 页码 + 时间 + 操作 */}
                  <div className="mb-1 flex items-center gap-1.5">
                    {a.kind === "underline" ? (
                      <span title="下划线" className="shrink-0">
                        <Underline className="h-3 w-3" />
                      </span>
                    ) : a.kind === "strike" ? (
                      <span title="删除线" className="shrink-0">
                        <Strikethrough className="h-3 w-3" />
                      </span>
                    ) : (
                      <span
                        className={cn(
                          "h-2.5 w-2.5 shrink-0 rounded-full",
                          a.color ? COLOR_DOT[a.color] : "bg-muted",
                        )}
                        title={a.color ? COLOR_LABEL[a.color] : "无颜色"}
                      />
                    )}
                    {a.page != null && (
                      <Badge variant="secondary" className="px-1.5 py-0 text-[10px]">
                        P.{a.page}
                      </Badge>
                    )}
                    <span className="text-[10px] text-muted-foreground">
                      {formatRelativeTime(a.created_at)}
                    </span>
                    <div className="ml-auto flex items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100">
                      {!isEditing && (
                        <>
                          <Button
                            size="icon"
                            variant="ghost"
                            className="h-6 w-6"
                            onClick={() => startEdit(a)}
                            title="编辑评论"
                          >
                            <Pencil className="h-3 w-3" />
                          </Button>
                          <Button
                            size="icon"
                            variant="ghost"
                            className="h-6 w-6 text-destructive hover:text-destructive"
                            onClick={() => setDeletingId(a.id)}
                            title="删除"
                          >
                            <Trash2 className="h-3 w-3" />
                          </Button>
                        </>
                      )}
                    </div>
                  </div>

                  {/* 选中文本 */}
                  {a.text && (
                    <div
                      className="mb-1 line-clamp-2 cursor-pointer text-foreground/90"
                      onClick={() => !isEditing && handleClickAnnotation(a)}
                      title="点击跳转到该页"
                    >
                      {a.text}
                    </div>
                  )}

                  {/* 评论：展示或编辑 */}
                  {isEditing ? (
                    <div className="mt-1 space-y-1">
                      <Textarea
                        value={editingComment}
                        onChange={(e) => setEditingComment(e.target.value)}
                        placeholder="输入评论..."
                        className="min-h-[60px] resize-none text-xs"
                        autoFocus
                      />
                      <div className="flex justify-end gap-1">
                        <Button
                          size="sm"
                          variant="ghost"
                          className="h-6 px-2 text-xs"
                          onClick={cancelEdit}
                          disabled={savingId === a.id}
                        >
                          <X className="mr-1 h-3 w-3" />
                          取消
                        </Button>
                        <Button
                          size="sm"
                          className="h-6 px-2 text-xs"
                          onClick={() => saveEdit(a.id)}
                          disabled={savingId === a.id}
                        >
                          <Check className="mr-1 h-3 w-3" />
                          保存
                        </Button>
                      </div>
                    </div>
                  ) : (
                    a.comment && (
                      <div className="mt-0.5 line-clamp-3 italic text-muted-foreground">
                        {a.comment}
                      </div>
                    )
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {/* 底部：同步到笔记 */}
      <div className="shrink-0 border-t border-border p-2">
        <Button
          className="w-full"
          size="sm"
          variant="outline"
          onClick={handleSync}
          disabled={syncing || annotations.length === 0}
        >
          <RefreshCw className={cn("mr-1.5 h-3.5 w-3.5", syncing && "animate-spin")} />
          {syncing ? "同步中..." : "同步到笔记"}
        </Button>
      </div>

      {/* 删除确认弹窗 */}
      {deletingId && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
          onClick={() => setDeletingId(null)}
        >
          <div
            className="w-72 rounded-lg border border-border bg-background p-4 shadow-lg"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="mb-3 text-sm font-medium">确认删除该批注？</div>
            <div className="mb-4 text-xs text-muted-foreground">
              删除后无法恢复。
            </div>
            <div className="flex justify-end gap-2">
              <Button
                size="sm"
                variant="ghost"
                onClick={() => setDeletingId(null)}
              >
                取消
              </Button>
              <Button size="sm" variant="destructive" onClick={confirmDelete}>
                删除
              </Button>
            </div>
          </div>
        </div>
      )}
    </aside>
  );
}

// 相对时间格式化（如 "刚刚"、"5 分钟前"、"2 小时前"、"3 天前"）
function formatRelativeTime(ts: number): string {
  const now = Date.now();
  const diff = Math.max(0, now - ts);
  const sec = Math.floor(diff / 1000);
  if (sec < 60) return "刚刚";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min} 分钟前`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr} 小时前`;
  const day = Math.floor(hr / 24);
  if (day < 30) return `${day} 天前`;
  const month = Math.floor(day / 30);
  if (month < 12) return `${month} 个月前`;
  const year = Math.floor(month / 12);
  return `${year} 年前`;
}
