// Reader 工作台：PDF 阅读器 + Markdown 笔记 + AI 对话侧边栏
import { useEffect, useState, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowLeft, Save, FilePlus2, BookOpen, PanelLeftClose, PanelLeftOpen, PanelRightOpen, Sparkles } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { PDFViewer } from "./PDFViewer";
import { NoteEditor } from "@/components/notes/NoteEditor";
import { AgentChatSidebar } from "@/components/ai/AgentChatSidebar";
import { api } from "@/lib/api";
import { useUIStore } from "@/stores/ui";
import type { PaperDetail } from "@/types";

interface Props {
  paperId: string;
}

export function ReaderShell({ paperId }: Props) {
  const navigate = useNavigate();
  const showToast = useUIStore((s) => s.showToast);
  const [detail, setDetail] = useState<PaperDetail | null>(null);
  const [hasNote, setHasNote] = useState(false);
  const [currentPage, setCurrentPage] = useState(1);
  const [totalPages, setTotalPages] = useState(0);
  const [savingProgress, setSavingProgress] = useState(false);
  // 笔记栏可收起
  const [notesCollapsed, setNotesCollapsed] = useState(false);
  // AI 对话栏可收起（默认展开，作为独立侧边栏）
  const [aiCollapsed, setAiCollapsed] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .getPaper(paperId)
      .then((d) => {
        if (cancelled) return;
        setDetail(d);
        setHasNote(!!d.note_path);
        if (d.reading_progress) {
          setCurrentPage(d.reading_progress.current_page);
          setTotalPages(d.reading_progress.total_pages);
        }
      })
      .catch((e) => {
        showToast("error", `加载论文失败: ${(e as Error).message}`);
      });
    return () => {
      cancelled = true;
    };
  }, [paperId, showToast]);

  // 防抖保存阅读进度
  const saveProgress = useCallback(
    async (page: number, total: number) => {
      try {
        setSavingProgress(true);
        await api.updateProgress(paperId, page, total);
      } catch (e) {
        console.error("save progress", e);
      } finally {
        setSavingProgress(false);
      }
    },
    [paperId],
  );

  const handlePageChange = useCallback(
    (page: number) => {
      setCurrentPage(page);
      if (totalPages > 0) {
        void saveProgress(page, totalPages);
      }
    },
    [saveProgress, totalPages],
  );

  const handleTotalPages = useCallback(
    (n: number) => {
      setTotalPages(n);
      if (currentPage > 0) {
        void saveProgress(currentPage, n);
      }
    },
    [currentPage, saveProgress],
  );

  async function handleCreateNote() {
    try {
      const path = await api.createNote(paperId);
      showToast("success", "笔记已创建");
      setHasNote(true);
      console.log("note path:", path);
    } catch (e) {
      showToast("error", `创建失败: ${(e as Error).message}`);
    }
  }

  if (!detail) {
    return (
      <div className="flex h-screen items-center justify-center text-muted-foreground">
        加载中…
      </div>
    );
  }

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="flex h-12 shrink-0 items-center gap-3 border-b border-border bg-card px-3">
        <Button variant="ghost" size="icon" onClick={() => navigate("/library")}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <BookOpen className="h-4 w-4 text-muted-foreground" />
        <div className="line-clamp-1 max-w-md text-sm font-medium">
          {detail.title || "（无标题）"}
        </div>
        <div className="text-xs text-muted-foreground">
          {formatAuthors(detail.authors)} · {detail.year ?? "—"}
        </div>
        <Badge variant="secondary" className="ml-2">
          {detail.status}
        </Badge>
        <div className="ml-auto flex items-center gap-2 text-xs text-muted-foreground">
          {savingProgress ? (
            <>
              <Save className="h-3 w-3 animate-pulse" />
              保存进度…
            </>
          ) : totalPages > 0 ? (
            <>
              第 {currentPage} / {totalPages} 页
            </>
          ) : null}
          <Button
            variant={aiCollapsed ? "ghost" : "secondary"}
            size="sm"
            className="h-7 gap-1 text-xs"
            onClick={() => setAiCollapsed((c) => !c)}
            title={aiCollapsed ? "展开 AI 对话" : "收起 AI 对话"}
          >
            <Sparkles className="h-3 w-3" />
            AI 对话
          </Button>
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        {/* PDF 阅读区（占比更大，不收起） */}
        <section className="flex-[3_1_0%] min-w-[400px] border-r border-border bg-muted/30">
          <PDFViewer
            paperId={paperId}
            src={detail.pdf_path}
            initialPage={currentPage}
            onPageChange={handlePageChange}
            onTotalPages={handleTotalPages}
          />
        </section>
        {/* 笔记编辑区（可收起） */}
        <section
          className={
            notesCollapsed
              ? "w-[40px] shrink-0 border-l border-border"
              : "flex-[1_1_0%] min-w-[320px] border-l border-border"
          }
        >
          {notesCollapsed ? (
            <button
              className="flex h-full w-full flex-col items-center justify-center text-muted-foreground hover:text-foreground"
              onClick={() => setNotesCollapsed(false)}
              title="展开笔记"
            >
              <PanelLeftOpen className="h-4 w-4" />
            </button>
          ) : (
            <div className="flex h-full flex-col">
              <div className="flex h-8 shrink-0 items-center justify-between border-b border-border px-2">
                <span className="text-xs font-medium">笔记</span>
                <Button
                  size="icon"
                  variant="ghost"
                  className="h-6 w-6"
                  onClick={() => setNotesCollapsed(true)}
                  title="收起笔记"
                >
                  <PanelLeftClose className="h-3.5 w-3.5" />
                </Button>
              </div>
              <div className="flex-1 overflow-y-auto">
                {hasNote ? (
                  <NoteEditor paperId={paperId} />
                ) : (
                  <div className="flex h-full flex-col items-center justify-center gap-3 p-6 text-center text-muted-foreground">
                    <p className="text-sm">这篇论文还没有 Markdown 笔记。</p>
                    <Button onClick={handleCreateNote}>
                      <FilePlus2 className="mr-1.5 h-4 w-4" />
                      创建空白笔记
                    </Button>
                    <p className="text-xs">
                      也可以在 AI 对话中使用"自动建笔记"快捷功能。
                    </p>
                  </div>
                )}
              </div>
            </div>
          )}
        </section>
        {/* AI 对话侧边栏（独立第三栏，可收起） */}
        <section
          className={
            aiCollapsed
              ? "w-[40px] shrink-0 border-l border-border"
              : "flex-[1_1_0%] min-w-[360px] border-l border-border"
          }
        >
          {aiCollapsed ? (
            <button
              className="flex h-full w-full flex-col items-center justify-center gap-2 text-muted-foreground hover:text-foreground"
              onClick={() => setAiCollapsed(false)}
              title="展开 AI 对话"
            >
              <PanelRightOpen className="h-4 w-4" />
              <span className="text-[10px] [writing-mode:vertical-rl]">AI 对话</span>
            </button>
          ) : (
            <div className="flex h-full flex-col">
              <AgentChatSidebar
                paperId={paperId}
                onClose={() => setAiCollapsed(true)}
              />
            </div>
          )}
        </section>
      </div>
    </div>
  );
}

function formatAuthors(authors: string[]): string {
  if (authors.length === 0) return "（未填）";
  if (authors.length <= 3) return authors.join(", ");
  return authors.slice(0, 3).join(", ") + " et al.";
}
