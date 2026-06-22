// Reader 工作台：PDF 阅读器 + 批注侧边栏 + Markdown 笔记
import { useEffect, useState, useCallback } from "react";
import { useNavigate } from "react-router-dom";
import { ArrowLeft, Save, FilePlus2, BookOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
import { PDFViewer } from "./PDFViewer";
import { AnnotationSidebar } from "./AnnotationSidebar";
import { NoteEditor } from "@/components/notes/NoteEditor";
import { api } from "@/lib/api";
import { useUIStore } from "@/stores/ui";
import type { Annotation, PaperDetail } from "@/types";

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
  // M-D P4：批注状态
  const [annotations, setAnnotations] = useState<Annotation[]>([]);
  const [annotationVersion, setAnnotationVersion] = useState(0);

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

  // 加载批注列表（annotationVersion 变化时重新加载）
  useEffect(() => {
    let cancelled = false;
    api
      .listAnnotations(paperId)
      .then((anns) => {
        if (cancelled) return;
        setAnnotations(anns);
      })
      .catch((e) => {
        console.error("list annotations", e);
      });
    return () => {
      cancelled = true;
    };
  }, [paperId, annotationVersion]);

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

  // 批注变更 → 重新加载批注列表
  const handleAnnotationChange = useCallback(() => {
    setAnnotationVersion((v) => v + 1);
  }, []);

  // 跳转到批注所在页（rect 已在 PDFViewer 的高亮覆盖层中渲染）
  const handleJumpToAnnotation = useCallback((page: number) => {
    setCurrentPage(page);
  }, []);

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
        </div>
      </header>

      <div className="flex flex-1 overflow-hidden">
        {/* PDF 阅读区 */}
        <section className="flex-1 border-r border-border bg-muted/30">
          <PDFViewer
            paperId={paperId}
            src={detail.pdf_path}
            initialPage={currentPage}
            onPageChange={handlePageChange}
            onTotalPages={handleTotalPages}
            annotations={annotations}
            onAnnotationChange={handleAnnotationChange}
          />
        </section>
        {/* 批注侧边栏 */}
        <section className="w-[240px] shrink-0">
          <AnnotationSidebar
            paperId={paperId}
            annotations={annotations}
            onAnnotationChange={handleAnnotationChange}
            onJumpToAnnotation={handleJumpToAnnotation}
          />
        </section>
        {/* 笔记编辑区 */}
        <section className="flex-1 min-w-[420px] overflow-y-auto">
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
                也可以在论文详情面板中使用 AI 自动创建结构化笔记。
              </p>
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
