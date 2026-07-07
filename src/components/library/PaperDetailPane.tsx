// 右侧论文详情：元数据编辑 + 打开阅读 / AI 入口
import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import {
  BookOpen,
  Trash2,
  Save,
  Sparkles,
  FileText,
  FileUp,
  Loader2,
  ExternalLink,
  RefreshCw,
  Tag as TagIcon,
  Star,
  X,
  Plus,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Badge } from "@/components/ui/badge";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Card } from "@/components/ui/card";
import { usePaperStore } from "@/stores/paper";
import { useUIStore } from "@/stores/ui";
import { api } from "@/lib/api";
import { isTauri, basename } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import type { PaperDetail, PaperStatus, MetadataCandidate } from "@/types";
import { AIPanel } from "@/components/ai/AIPanel";

export function PaperDetailPane({ paperId }: { paperId: string }) {
  const navigate = useNavigate();
  const getPaper = usePaperStore((s) => s.getPaper);
  const updatePaper = usePaperStore((s) => s.updatePaper);
  const removePaper = usePaperStore((s) => s.removePaper);
  const collections = usePaperStore((s) => s.collections);
  const loadCollections = usePaperStore((s) => s.loadCollections);
  const showToast = useUIStore((s) => s.showToast);
  const [detail, setDetail] = useState<PaperDetail | null>(null);
  // savedDetail: 最近一次保存（或加载）的快照，用于 dirty 判断
  const [savedDetail, setSavedDetail] = useState<PaperDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [candidate, setCandidate] = useState<MetadataCandidate | null>(null);
  const [extracting, setExtracting] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const [showCollectionPicker, setShowCollectionPicker] = useState(false);
  const [collectionBusy, setCollectionBusy] = useState<string | null>(null);

  const isDirty =
    detail != null && savedDetail != null
      ? JSON.stringify(detail) !== JSON.stringify(savedDetail)
      : false;

  useEffect(() => {
    setLoading(true);
    setCandidate(null);
    getPaper(paperId)
      .then((d) => {
        setDetail(d);
        setSavedDetail(d);
      })
      .catch((e) => showToast("error", `加载失败: ${(e as Error).message}`))
      .finally(() => setLoading(false));
    // 确保集合列表已加载（供加入集合下拉使用）
    void loadCollections();
  }, [paperId, getPaper, showToast, loadCollections]);

  // 已加入 / 未加入 的集合（detail.collections 存的是集合名字）
  const joinedCollections = collections.filter((c) =>
    detail?.collections.includes(c.name),
  );
  const availableCollections = collections.filter(
    (c) => !detail?.collections.includes(c.name),
  );

  async function handleAddToCollection(collectionId: string) {
    if (!detail) return;
    setCollectionBusy(collectionId);
    try {
      await api.addPaperToCollection(detail.id, collectionId);
      const refreshed = await getPaper(detail.id);
      setDetail(refreshed);
      setSavedDetail(refreshed);
      showToast("success", "已加入集合");
    } catch (e) {
      showToast("error", `加入失败: ${(e as Error).message}`);
    } finally {
      setCollectionBusy(null);
      setShowCollectionPicker(false);
    }
  }

  async function handleRemoveFromCollection(collectionId: string, name: string) {
    if (!detail) return;
    setCollectionBusy(collectionId);
    try {
      await api.removePaperFromCollection(detail.id, collectionId);
      const refreshed = await getPaper(detail.id);
      setDetail(refreshed);
      setSavedDetail(refreshed);
      showToast("success", `已从「${name}」移除`);
    } catch (e) {
      showToast("error", `移除失败: ${(e as Error).message}`);
    } finally {
      setCollectionBusy(null);
    }
  }

  async function handleSave() {
    if (!detail) return;
    setSaving(true);
    try {
      const { reading_progress: _rp, index_status: _is, collections: _cs, ...paper } = detail;
      void _rp; void _is; void _cs;
      const updated = await updatePaper(paperId, paper);
      const merged = { ...detail, ...updated };
      setDetail(merged);
      setSavedDetail(merged);
      showToast("success", "已保存");
    } catch (e) {
      showToast("error", `保存失败: ${(e as Error).message}`);
    } finally {
      setSaving(false);
    }
  }

  async function handleExtract() {
    setExtracting(true);
    try {
      const c = await api.extractMetadata(paperId);
      setCandidate(c);

      // 提取成功后直接把候选元数据合并到 detail 并保存到数据库
      // （避免用户还需要手动点"采纳并填入" + "保存"两步）
      if (detail) {
        const merged: PaperDetail = {
          ...detail,
          title: c.title || detail.title,
          authors: c.authors.length ? c.authors : detail.authors,
          year: c.year ?? detail.year,
          venue: c.venue || detail.venue,
          doi: c.doi || detail.doi,
          abstract_text: c.abstract_text || detail.abstract_text,
          keywords: c.keywords.length ? c.keywords : detail.keywords,
        };
        const { reading_progress: _rp, index_status: _is, collections: _cs, ...patch } = merged;
        void _rp; void _is; void _cs;
        try {
          const updated = await updatePaper(paperId, patch);
          const refreshed = { ...merged, ...updated };
          setDetail(refreshed);
          setSavedDetail(refreshed);

          // 根据来源显示不同的 toast，让用户知道是否走了联网搜索
          const sourceDesc = describeSource(c.source);
          if (c.source === "pdf-text") {
            showToast(
              "warning",
              `已提取元数据（${sourceDesc}）。联网未命中，年份/期刊/摘要/关键词可能为空。建议手动编辑标题后重试，或检查 DOI 是否正确。`,
            );
          } else {
            showToast(
              "success",
              `已提取并保存元数据（${sourceDesc}）`,
            );
          }
        } catch (e) {
          // 保存失败时仍保留候选卡片，让用户可以手动采纳
          showToast("warning", `提取成功但保存失败: ${(e as Error).message}，请手动点击保存`);
        }
      } else {
        showToast("info", "已生成候选元数据");
      }
    } catch (e) {
      showToast("error", `提取失败: ${(e as Error).message}`);
    } finally {
      setExtracting(false);
    }
  }

  /** 把 candidate.source 转为可读的来源描述。 */
  function describeSource(source: string): string {
    if (source.startsWith("network-")) {
      if (source === "network-crossref-search") {
        return "Crossref 按标题联网搜索";
      }
      const scheme = source.slice("network-".length);
      const map: Record<string, string> = {
        doi: "DOI 联网获取（Crossref）",
        arxiv: "arXiv 联网获取",
        pmid: "PubMed 联网获取",
        isbn: "OpenLibrary 联网获取",
      };
      return map[scheme] ?? `${scheme} 联网获取`;
    }
    if (source === "pdf-text") {
      return "PDF 本地提取（未联网）";
    }
    return source;
  }

  async function handleOpen() {
    if (!detail) return;
    if (!isTauri()) {
      showToast("warning", "请在 Tauri 桌面应用中打开 PDF");
      return;
    }
    try {
      await api.openPdf(detail.id);
    } catch (e) {
      showToast("error", `打开失败: ${String(e)}`);
    }
  }

  async function handleRead() {
    if (!detail) return;
    navigate(`/reader/${detail.id}`);
  }

  async function handleImportNote() {
    if (!detail) return;
    if (!isTauri()) {
      showToast("warning", "请在 Tauri 桌面应用中导入 Markdown 笔记");
      return;
    }
    try {
      const selected = await open({
        multiple: false,
        directory: false,
        filters: [{ name: "Markdown", extensions: ["md", "markdown"] }],
        title: "选择已有 Markdown 笔记",
      });
      if (typeof selected !== "string") return;
      await api.importNote(detail.id, selected);
      const refreshed = await api.getPaper(detail.id);
      setDetail(refreshed);
      showToast("success", "Markdown 笔记已导入");
    } catch (e) {
      showToast("error", `导入笔记失败: ${(e as Error).message}`);
    }
  }

  async function handleDelete() {
    if (!detail) return;
    if (!confirmDelete) {
      setConfirmDelete(true);
      setTimeout(() => setConfirmDelete(false), 4000);
      return;
    }
    try {
      await removePaper(detail.id);
      showToast("success", "已删除");
    } catch (e) {
      showToast("error", `删除失败: ${(e as Error).message}`);
    }
  }

  if (loading || !detail) {
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        <Loader2 className="mr-2 h-4 w-4 animate-spin" />
        加载中…
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-border bg-card p-3">
        <Button size="sm" onClick={handleRead}>
          <BookOpen className="mr-1.5 h-4 w-4" />
          阅读
        </Button>
        <Button size="sm" variant="outline" onClick={handleOpen}>
          <ExternalLink className="mr-1.5 h-4 w-4" />
          打开 PDF
        </Button>
        <Button size="sm" variant="outline" onClick={handleExtract} disabled={extracting}>
          <Sparkles className="mr-1.5 h-4 w-4" />
          {extracting ? "提取中…" : "提取元数据"}
        </Button>
        <Button size="sm" variant="outline" onClick={handleImportNote}>
          <FileUp className="mr-1.5 h-4 w-4" />
          导入笔记
        </Button>
        <div className="ml-auto flex items-center gap-1">
          <Button
            size="sm"
            variant={isDirty ? "default" : "ghost"}
            onClick={handleSave}
            disabled={saving}
            className={isDirty ? "animate-pulse" : ""}
          >
            <Save className="mr-1.5 h-4 w-4" />
            {saving ? "保存中…" : isDirty ? "保存 *" : "保存"}
          </Button>
        </div>
      </div>

      <Tabs defaultValue="meta" className="flex flex-1 flex-col overflow-hidden">
        <TabsList className="mx-3 mt-2 grid w-auto grid-cols-3">
          <TabsTrigger value="meta">元数据</TabsTrigger>
          <TabsTrigger value="ai">AI 工具</TabsTrigger>
          <TabsTrigger value="danger">更多</TabsTrigger>
        </TabsList>

        <TabsContent value="meta" className="flex-1 overflow-y-auto p-3">
          {candidate && (
            <Card className="mb-3 border-primary/50 bg-primary/5 p-3">
              <div className="mb-1 flex items-center gap-1.5 text-xs font-medium text-primary">
                <Sparkles className="h-3 w-3" />
                已提取元数据（{describeSource(candidate.source)} · 置信度: {candidate.confidence}）
              </div>
              <div className="mb-1 text-sm font-medium">{candidate.title}</div>
              <div className="mb-1 text-xs text-muted-foreground">
                {candidate.authors.join(", ")} · {candidate.year ?? "—"} · {candidate.venue}
              </div>
              {candidate.doi && (
                <div className="mb-1 text-xs">DOI: {candidate.doi}</div>
              )}
              {candidate.abstract_text && (
                <div className="mb-2 line-clamp-4 text-xs text-muted-foreground">
                  {candidate.abstract_text}
                </div>
              )}
              <div className="flex flex-wrap gap-1">
                <Button size="sm" variant="ghost" onClick={() => setCandidate(null)}>
                  关闭
                </Button>
              </div>
            </Card>
          )}

          <div className="space-y-3">
            <Field label="标题">
              <Input
                value={detail.title}
                onChange={(e) => setDetail({ ...detail, title: e.target.value })}
              />
            </Field>
            <Field label="作者（逗号分隔）">
              <Input
                value={detail.authors.join(", ")}
                onChange={(e) =>
                  setDetail({
                    ...detail,
                    authors: e.target.value
                      .split(/[,，]/)
                      .map((s) => s.trim())
                      .filter(Boolean),
                  })
                }
              />
            </Field>
            <div className="grid grid-cols-2 gap-2">
              <Field label="年份">
                <Input
                  type="number"
                  value={detail.year ?? ""}
                  onChange={(e) =>
                    setDetail({
                      ...detail,
                      year: e.target.value ? Number(e.target.value) : null,
                    })
                  }
                />
              </Field>
              <Field label="期刊/会议">
                <Input
                  value={detail.venue}
                  onChange={(e) => setDetail({ ...detail, venue: e.target.value })}
                />
              </Field>
            </div>
            <Field label="DOI">
              <Input
                value={detail.doi}
                onChange={(e) => setDetail({ ...detail, doi: e.target.value })}
                placeholder="10.xxxx/..."
              />
            </Field>
            <Field label="关键词（逗号分隔）">
              <Input
                value={detail.keywords.join(", ")}
                onChange={(e) =>
                  setDetail({
                    ...detail,
                    keywords: e.target.value
                      .split(/[,，]/)
                      .map((s) => s.trim())
                      .filter(Boolean),
                  })
                }
              />
            </Field>
            <Field label="摘要">
              <Textarea
                rows={5}
                value={detail.abstract_text}
                onChange={(e) => setDetail({ ...detail, abstract_text: e.target.value })}
              />
            </Field>
            <div className="grid grid-cols-2 gap-2">
              <Field label="状态">
                <select
                  value={detail.status}
                  onChange={(e) =>
                    setDetail({ ...detail, status: e.target.value as PaperStatus })
                  }
                  className="h-9 w-full rounded border border-input bg-background px-2 text-sm"
                >
                  <option value="unread">未读</option>
                  <option value="reading">阅读中</option>
                  <option value="read">已读</option>
                </select>
              </Field>
              <Field label="评分 (0-5)">
                <div className="flex items-center gap-1">
                  {[1, 2, 3, 4, 5].map((n) => (
                    <button
                      key={n}
                      type="button"
                      onClick={() =>
                        setDetail({
                          ...detail,
                          rating: detail.rating === n ? null : n,
                        })
                      }
                      className="p-0.5"
                    >
                      <Star
                        className={cn(
                          "h-4 w-4",
                          (detail.rating ?? 0) >= n
                            ? "fill-yellow-400 text-yellow-400"
                            : "text-muted-foreground",
                        )}
                      />
                    </button>
                  ))}
                </div>
              </Field>
            </div>
            <div className="rounded border border-border bg-muted/30 p-2 text-xs">
              <div className="mb-1 flex items-center gap-1 text-muted-foreground">
                <FileText className="h-3 w-3" />
                PDF
              </div>
              <div className="break-all">{basename(detail.pdf_path) || "—"}</div>
              {detail.reading_progress && (
                <div className="mt-1 text-muted-foreground">
                  阅读进度: 第 {detail.reading_progress.current_page} / {detail.reading_progress.total_pages} 页
                  （{Math.round(detail.reading_progress.progress_percent)}%）
                </div>
              )}
              <div className="mt-1 text-muted-foreground">
                索引状态: {detail.index_status}
              </div>
            </div>
          </div>
        </TabsContent>

        <TabsContent value="ai" className="flex-1 overflow-y-auto p-3">
          <AIPanel paperId={paperId} hasNote={!!detail.note_path} />
        </TabsContent>

        <TabsContent value="danger" className="flex-1 overflow-y-auto p-3">
          <Card className="border-destructive/40 p-3">
            <div className="mb-2 flex items-center gap-2 text-sm font-medium text-destructive">
              <Trash2 className="h-4 w-4" />
              删除论文
            </div>
            <p className="mb-2 text-xs text-muted-foreground">
              P0 仅支持硬删：删除数据库条目，PDF 和 Markdown 笔记作为用户资产保留。
            </p>
            <div className="space-y-2">
              <Button
                variant={confirmDelete ? "destructive" : "outline"}
                size="sm"
                className="w-full"
                onClick={() => handleDelete()}
              >
                {confirmDelete ? "再次点击确认" : "删除条目"}
              </Button>
            </div>
            <p className="mt-2 text-[10px] text-muted-foreground">
              如需同时清理 PDF / 笔记，请到 vault 目录手动删除。
            </p>
          </Card>

          <Card className="mt-3 p-3">
            <div className="mb-2 flex items-center gap-2 text-sm font-medium">
              <TagIcon className="h-4 w-4" />
              所属集合
            </div>
            {joinedCollections.length === 0 ? (
              <p className="mb-2 text-xs text-muted-foreground">
                此论文尚未加入任何集合
              </p>
            ) : (
              <div className="mb-2 flex flex-wrap gap-1">
                {joinedCollections.map((c) => (
                  <Badge
                    key={c.id}
                    variant="secondary"
                    className="flex items-center gap-1 pr-1"
                  >
                    {c.name}
                    <button
                      onClick={() => handleRemoveFromCollection(c.id, c.name)}
                      disabled={collectionBusy === c.id}
                      className="ml-0.5 rounded-full p-0.5 hover:bg-background/80 disabled:opacity-40"
                      title="从该集合移除"
                    >
                      <X className="h-3 w-3" />
                    </button>
                  </Badge>
                ))}
              </div>
            )}

            {/* 加入集合下拉 */}
            <div className="relative">
              {showCollectionPicker ? (
                <div className="rounded-md border border-border bg-popover p-1 shadow-md">
                  {availableCollections.length === 0 ? (
                    <div className="px-2 py-1.5 text-xs text-muted-foreground">
                      没有可加入的集合（全部已加入或无集合）
                    </div>
                  ) : (
                    <div className="max-h-40 overflow-y-auto">
                      {availableCollections.map((c) => (
                        <button
                          key={c.id}
                          onClick={() => handleAddToCollection(c.id)}
                          disabled={!!collectionBusy}
                          className="flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-xs hover:bg-accent disabled:opacity-50"
                        >
                          <Plus className="h-3 w-3" />
                          {c.name}
                        </button>
                      ))}
                    </div>
                  )}
                  <button
                    onClick={() => setShowCollectionPicker(false)}
                    className="mt-1 w-full rounded px-2 py-1 text-center text-xs text-muted-foreground hover:bg-accent"
                  >
                    取消
                  </button>
                </div>
              ) : (
                <Button
                  size="sm"
                  variant="outline"
                  className="w-full"
                  onClick={() => setShowCollectionPicker(true)}
                  disabled={collections.length === 0}
                >
                  <Plus className="mr-1 h-3.5 w-3.5" />
                  加入集合…
                </Button>
              )}
            </div>
            {collections.length === 0 && (
              <p className="mt-1 text-[10px] text-muted-foreground">
                请先在左侧集合栏新建一个集合
              </p>
            )}
          </Card>

          <Card className="mt-3 p-3">
            <div className="mb-2 flex items-center gap-2 text-sm font-medium">
              <RefreshCw className="h-4 w-4" />
              全文索引
            </div>
            <p className="mb-2 text-xs text-muted-foreground">
              状态: {detail.index_status}
            </p>
            <Button
              size="sm"
              variant="outline"
              onClick={async () => {
                try {
                  await api.reindexPaper(detail.id);
                  showToast("info", "已加入重新索引队列");
                } catch (e) {
                  showToast("error", `触发失败: ${(e as Error).message}`);
                }
              }}
            >
              重新索引
            </Button>
          </Card>
        </TabsContent>
      </Tabs>
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div>
      <Label className="mb-1 block text-xs">{label}</Label>
      {children}
    </div>
  );
}
