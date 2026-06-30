// PDF 阅读器（pdf.js 渲染 + 文本选区 + 批注高亮）
import { useEffect, useState, useRef, useCallback } from "react";
import {
  Loader2,
  ZoomIn,
  ZoomOut,
  ExternalLink,
  ChevronLeft,
  ChevronRight,
  MessageSquare,
  Underline,
  Strikethrough,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import { basename } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useUIStore } from "@/stores/ui";
import type { Annotation, AnnotationRect } from "@/types";

import * as pdfjsLib from "pdfjs-dist";
import type { PDFDocumentProxy, RenderTask } from "pdfjs-dist";
import PdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";

pdfjsLib.GlobalWorkerOptions.workerSrc = PdfWorker;

// 高亮颜色映射（name → tailwind class + rgba 用于覆盖层）
const COLORS = [
  { name: "yellow", class: "bg-yellow-400", rgba: "rgba(250, 204, 21, 0.35)" },
  { name: "red", class: "bg-red-400", rgba: "rgba(248, 113, 113, 0.35)" },
  { name: "green", class: "bg-green-400", rgba: "rgba(74, 222, 128, 0.35)" },
  { name: "blue", class: "bg-blue-400", rgba: "rgba(96, 165, 250, 0.35)" },
  { name: "purple", class: "bg-purple-400", rgba: "rgba(192, 132, 252, 0.35)" },
];

// 不透明色映射（underline / strike 用）
const SOLID_COLORS: Record<string, string> = {
  yellow: "rgb(250, 204, 21)",
  red: "rgb(248, 113, 113)",
  green: "rgb(74, 222, 128)",
  blue: "rgb(96, 165, 250)",
  purple: "rgb(192, 132, 252)",
};

function colorRgba(name: string | null): string {
  const c = COLORS.find((co) => co.name === name);
  return c ? c.rgba : "rgba(250, 204, 21, 0.35)";
}

function colorSolid(name: string | null): string {
  return SOLID_COLORS[name ?? "yellow"] ?? SOLID_COLORS.yellow;
}

// 按 kind 生成批注覆盖层样式
function renderStyle(
  kind: string,
  r: AnnotationRect,
  color: string | null,
): React.CSSProperties {
  const base: React.CSSProperties = {
    left: `${r.x * 100}%`,
    top: `${r.y * 100}%`,
    width: `${r.w * 100}%`,
  };
  if (kind === "underline") {
    return {
      ...base,
      height: "2px",
      top: `${(r.y + r.h) * 100}%`,
      backgroundColor: colorSolid(color),
    };
  }
  if (kind === "strike") {
    return {
      ...base,
      height: "2px",
      top: `${(r.y + r.h / 2) * 100}%`,
      backgroundColor: colorSolid(color),
    };
  }
  // highlight / note 默认
  return {
    ...base,
    height: `${r.h * 100}%`,
    backgroundColor: colorRgba(color),
  };
}

interface SelectionState {
  text: string;
  rect: AnnotationRect[]; // 多 rect（跨行选区）
  x: number; // 相对于 .pdf-page 的像素位置（用于定位 toolbar）
  y: number;
}

interface Props {
  paperId: string;
  src: string;
  initialPage: number;
  onPageChange: (page: number) => void;
  onTotalPages: (n: number) => void;
  annotations: Annotation[];
  onAnnotationChange: () => void;
}

export function PDFViewer({
  paperId,
  src,
  initialPage,
  onPageChange,
  onTotalPages,
  annotations,
  onAnnotationChange,
}: Props) {
  const showToast = useUIStore((s) => s.showToast);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(100);
  const [currentPage, setCurrentPage] = useState(initialPage);
  const [totalPages, setTotalPages] = useState(0);
  const [pageInput, setPageInput] = useState(initialPage);
  const [selectionState, setSelectionState] = useState<SelectionState | null>(null);
  const [commentMode, setCommentMode] = useState(false);
  const [commentText, setCommentText] = useState("");
  const [creating, setCreating] = useState(false);
  const [docReady, setDocReady] = useState(false);

  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const textLayerRef = useRef<HTMLDivElement | null>(null);
  const pdfDocRef = useRef<PDFDocumentProxy | null>(null);
  const renderTaskRef = useRef<RenderTask | null>(null);
  const textLayerInstanceRef = useRef<InstanceType<typeof pdfjsLib.TextLayer> | null>(null);

  // 回调 ref，避免触发 effect 重新执行
  const onTotalPagesRef = useRef(onTotalPages);
  onTotalPagesRef.current = onTotalPages;
  const onPageChangeRef = useRef(onPageChange);
  onPageChangeRef.current = onPageChange;
  const onAnnotationChangeRef = useRef(onAnnotationChange);
  onAnnotationChangeRef.current = onAnnotationChange;

  // 同步 initialPage → currentPage（用于跳转到批注）
  useEffect(() => {
    setCurrentPage((prev) => (prev !== initialPage ? initialPage : prev));
    setPageInput((prev) => (prev !== initialPage ? initialPage : prev));
  }, [initialPage]);

  // 加载 PDF 文档
  useEffect(() => {
    let cancelled = false;
    setDocReady(false);
    setLoading(true);
    setError(null);
    onTotalPagesRef.current(0);

    (async () => {
      try {
        if (!src) {
          throw new Error("PDF 路径为空");
        }
        const bytes = await api.readPdfBytes(paperId);
        if (cancelled) return;
        const data = new Uint8Array(bytes);
        const loadingTask = pdfjsLib.getDocument({ data });
        const doc = await loadingTask.promise;
        if (cancelled) {
          void doc.destroy();
          return;
        }
        pdfDocRef.current = doc;
        setTotalPages(doc.numPages);
        onTotalPagesRef.current(doc.numPages);
        setDocReady(true);
        setLoading(false);
      } catch (e) {
        if (cancelled) return;
        setError((e as Error).message || String(e));
        setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
      if (renderTaskRef.current) {
        renderTaskRef.current.cancel();
        renderTaskRef.current = null;
      }
      if (textLayerInstanceRef.current) {
        textLayerInstanceRef.current.cancel();
        textLayerInstanceRef.current = null;
      }
      if (pdfDocRef.current) {
        void pdfDocRef.current.destroy();
        pdfDocRef.current = null;
      }
    };
  }, [paperId, src]);

  // 渲染当前页（canvas + 文本层）
  useEffect(() => {
    if (!docReady) return;
    const doc = pdfDocRef.current;
    const canvas = canvasRef.current;
    const textLayerDiv = textLayerRef.current;
    if (!doc || !canvas || !textLayerDiv) return;

    let cancelled = false;

    // 取消上一次渲染
    if (renderTaskRef.current) {
      renderTaskRef.current.cancel();
      renderTaskRef.current = null;
    }
    if (textLayerInstanceRef.current) {
      textLayerInstanceRef.current.cancel();
      textLayerInstanceRef.current = null;
    }

    (async () => {
      try {
        const page = await doc.getPage(currentPage);
        if (cancelled) return;

        const scale = zoom / 100;
        const viewport = page.getViewport({ scale });
        const outputScale = window.devicePixelRatio || 1;

        canvas.width = Math.floor(viewport.width * outputScale);
        canvas.height = Math.floor(viewport.height * outputScale);
        canvas.style.width = `${Math.floor(viewport.width)}px`;
        canvas.style.height = `${Math.floor(viewport.height)}px`;

        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        const transform =
          outputScale !== 1 ? [outputScale, 0, 0, outputScale, 0, 0] : undefined;
        const renderTask = page.render({ canvasContext: ctx, viewport, transform });
        renderTaskRef.current = renderTask;
        await renderTask.promise;
        if (cancelled) return;

        // 渲染文本层（用于文本选区）
        textLayerDiv.innerHTML = "";
        textLayerDiv.style.width = `${Math.floor(viewport.width)}px`;
        textLayerDiv.style.height = `${Math.floor(viewport.height)}px`;

        try {
          const textContent = await page.getTextContent();
          if (cancelled) return;
          const textLayer = new pdfjsLib.TextLayer({
            textContentSource: textContent,
            container: textLayerDiv,
            viewport,
          });
          textLayerInstanceRef.current = textLayer;
          await textLayer.render();
        } catch (e) {
          if (cancelled) return;
          // 文本层渲染失败不影响 PDF 查看，仅无法选中文本
          console.error("text layer render", e);
        }
      } catch (e) {
        if (cancelled) return;
        // 渲染取消等非致命错误
        if (e instanceof Error && e.name === "RenderingCancelledException") return;
        console.error("page render", e);
      }
    })();

    return () => {
      cancelled = true;
      if (renderTaskRef.current) {
        renderTaskRef.current.cancel();
        renderTaskRef.current = null;
      }
      if (textLayerInstanceRef.current) {
        textLayerInstanceRef.current.cancel();
        textLayerInstanceRef.current = null;
      }
    };
  }, [docReady, currentPage, zoom]);

  // 翻页/缩放时清除选区
  useEffect(() => {
    setSelectionState(null);
    setCommentMode(false);
    setCommentText("");
  }, [currentPage, zoom]);

  // 选区监听：mouseup 时检查是否有选中文本
  useEffect(() => {
    function handleMouseUp() {
      const selection = window.getSelection();
      if (!selection || selection.isCollapsed) {
        setSelectionState(null);
        setCommentMode(false);
        return;
      }
      const textLayer = textLayerRef.current;
      if (!textLayer) return;
      const range = selection.getRangeAt(0);
      if (!textLayer.contains(range.commonAncestorContainer)) {
        return;
      }
      const text = selection.toString().trim();
      if (!text) {
        setSelectionState(null);
        return;
      }
      // 用 getClientRects() 取逐行 rect，避免跨行选区返回并集矩形过大
      const rawRects = Array.from(range.getClientRects());
      if (rawRects.length === 0) return;
      const layerRect = textLayer.getBoundingClientRect();
      const normRects: AnnotationRect[] = rawRects
        .filter((r) => r.width > 0 && r.height > 0)
        .map((r) => ({
          x: (r.left - layerRect.left) / layerRect.width,
          y: (r.top - layerRect.top) / layerRect.height,
          w: r.width / layerRect.width,
          h: r.height / layerRect.height,
        }));
      if (normRects.length === 0) return;
      // toolbar 定位用第一个 rect（相对于 .pdf-page 容器）
      const first = rawRects[0];
      const x = first.left - layerRect.left + first.width / 2;
      const y = first.top - layerRect.top;
      setSelectionState({ text, rect: normRects, x, y });
    }
    document.addEventListener("mouseup", handleMouseUp);
    return () => document.removeEventListener("mouseup", handleMouseUp);
  }, []);

  const goToPage = useCallback(
    (page: number) => {
      const next = Math.max(1, Math.min(totalPages || 1, page));
      setCurrentPage(next);
      setPageInput(next);
      onPageChangeRef.current(next);
    },
    [totalPages],
  );

  const handleCreateHighlight = useCallback(
    async (color: string) => {
      if (!selectionState) return;
      setCreating(true);
      try {
        await api.createAnnotation({
          paperId,
          kind: "highlight",
          page: currentPage,
          rect: selectionState.rect,
          color,
          text: selectionState.text,
          comment: null,
        });
        showToast("success", "已添加高亮");
        window.getSelection()?.removeAllRanges();
        setSelectionState(null);
        onAnnotationChangeRef.current();
      } catch (e) {
        showToast("error", `添加失败: ${(e as Error).message}`);
      } finally {
        setCreating(false);
      }
    },
    [selectionState, paperId, currentPage, showToast],
  );

  const handleCreateNote = useCallback(async () => {
    if (!selectionState) return;
    setCreating(true);
    try {
      await api.createAnnotation({
        paperId,
        kind: "note",
        page: currentPage,
        rect: selectionState.rect,
        color: null,
        text: selectionState.text,
        comment: commentText || null,
      });
      showToast("success", "已添加笔记");
      window.getSelection()?.removeAllRanges();
      setSelectionState(null);
      setCommentMode(false);
      setCommentText("");
      onAnnotationChangeRef.current();
    } catch (e) {
      showToast("error", `添加失败: ${(e as Error).message}`);
    } finally {
      setCreating(false);
    }
  }, [selectionState, paperId, currentPage, commentText, showToast]);

  // underline / strike 批注（默认黄色实色线）
  const handleCreateAnnotation = useCallback(
    async (kind: "underline" | "strike") => {
      if (!selectionState) return;
      setCreating(true);
      try {
        await api.createAnnotation({
          paperId,
          kind,
          page: currentPage,
          rect: selectionState.rect,
          color: "yellow",
          text: selectionState.text,
          comment: null,
        });
        showToast("success", kind === "underline" ? "已添加下划线" : "已添加删除线");
        window.getSelection()?.removeAllRanges();
        setSelectionState(null);
        onAnnotationChangeRef.current();
      } catch (e) {
        showToast("error", `添加失败: ${(e as Error).message}`);
      } finally {
        setCreating(false);
      }
    },
    [selectionState, paperId, currentPage, showToast],
  );

  function openNative() {
    void api.openPdf(paperId).catch((e) => {
      showToast("error", `打开失败: ${String(e)}`);
    });
  }

  return (
    <div className="flex h-full flex-col">
      {/* 工具栏 */}
      <div className="flex shrink-0 items-center gap-1 border-b border-border bg-card p-2">
        <Button
          size="icon"
          variant="ghost"
          onClick={() => goToPage(currentPage - 1)}
          disabled={currentPage <= 1}
        >
          <ChevronLeft className="h-4 w-4" />
        </Button>
        <Input
          type="number"
          value={pageInput}
          onChange={(e) => setPageInput(Number(e.target.value))}
          onBlur={() => goToPage(pageInput)}
          onKeyDown={(e) => {
            if (e.key === "Enter") goToPage(pageInput);
          }}
          className="h-7 w-16 text-center text-xs"
        />
        <span className="text-xs text-muted-foreground">/ {totalPages || "—"}</span>
        <Button
          size="icon"
          variant="ghost"
          onClick={() => goToPage(currentPage + 1)}
          disabled={currentPage >= totalPages}
        >
          <ChevronRight className="h-4 w-4" />
        </Button>
        <div className="mx-1 h-5 w-px bg-border" />
        <Button
          size="icon"
          variant="ghost"
          onClick={() => setZoom((z) => Math.max(50, z - 10))}
        >
          <ZoomOut className="h-4 w-4" />
        </Button>
        <span className="w-12 text-center text-xs">{zoom}%</span>
        <Button
          size="icon"
          variant="ghost"
          onClick={() => setZoom((z) => Math.min(300, z + 10))}
        >
          <ZoomIn className="h-4 w-4" />
        </Button>
        <div className="ml-auto">
          <Button size="icon" variant="ghost" onClick={openNative} disabled={!src}>
            <ExternalLink className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {/* PDF 渲染区 */}
      <div className="relative flex-1 overflow-auto bg-muted/30">
        {loading && (
          <div className="absolute inset-0 z-40 flex items-center justify-center gap-2 bg-background/80 text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            加载 PDF...
          </div>
        )}

        {error ? (
          <div className="m-4 rounded border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
            加载失败: {error}
            <div className="mt-1 text-xs">文件: {basename(src)}</div>
          </div>
        ) : (
          <div className="flex min-h-full items-start justify-center p-4">
            <div className="pdf-page">
              <canvas ref={canvasRef} />

              {/* 批注高亮覆盖层 */}
              <div className="pointer-events-none absolute inset-0 z-10">
                {annotations
                  .filter((a) => a.page === currentPage && a.rect && a.rect.length > 0)
                  .flatMap((a) =>
                    a.rect!.map((r, i) => (
                      <div
                        key={`${a.id}-${i}`}
                        className="absolute"
                        style={renderStyle(a.kind, r, a.color)}
                      />
                    )),
                  )}
              </div>

              {/* 文本层（用于选区） */}
              <div ref={textLayerRef} className="textLayer" />

              {/* 选区 mini toolbar */}
              {selectionState && !commentMode && (
                <div
                  className="absolute z-30 flex items-center gap-1 rounded-md border border-border bg-popover p-1 shadow-md"
                  style={{
                    left: `${selectionState.x}px`,
                    top: `${selectionState.y - 40}px`,
                    transform: "translateX(-50%)",
                  }}
                  onMouseDown={(e) => e.preventDefault()}
                >
                  {COLORS.map((c) => (
                    <button
                      key={c.name}
                      type="button"
                      disabled={creating}
                      className={cn(
                        "h-5 w-5 rounded-full border border-border transition-opacity hover:opacity-80 disabled:opacity-50",
                        c.class,
                      )}
                      title={c.name}
                      onClick={() => handleCreateHighlight(c.name)}
                    />
                  ))}
                  <div className="mx-1 h-4 w-px bg-border" />
                  <Button
                    size="icon"
                    variant="ghost"
                    className="h-6 w-6"
                    disabled={creating}
                    onClick={() => handleCreateAnnotation("underline")}
                    title="下划线"
                  >
                    <Underline className="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    size="icon"
                    variant="ghost"
                    className="h-6 w-6"
                    disabled={creating}
                    onClick={() => handleCreateAnnotation("strike")}
                    title="删除线"
                  >
                    <Strikethrough className="h-3.5 w-3.5" />
                  </Button>
                  <div className="mx-1 h-4 w-px bg-border" />
                  <Button
                    size="icon"
                    variant="ghost"
                    className="h-6 w-6"
                    disabled={creating}
                    onClick={() => setCommentMode(true)}
                    title="添加评论"
                  >
                    <MessageSquare className="h-3.5 w-3.5" />
                  </Button>
                </div>
              )}

              {/* 评论输入 */}
              {selectionState && commentMode && (
                <div
                  className="absolute z-40 w-56 rounded-md border border-border bg-popover p-2 shadow-lg"
                  style={{
                    left: `${selectionState.x}px`,
                    top: `${selectionState.y - 120}px`,
                    transform: "translateX(-50%)",
                  }}
                  onMouseDown={(e) => e.preventDefault()}
                >
                  <textarea
                    value={commentText}
                    onChange={(e) => setCommentText(e.target.value)}
                    placeholder="输入评论..."
                    className="mb-1 h-16 w-full resize-none rounded border border-input bg-background p-1 text-xs"
                    autoFocus
                  />
                  <div className="flex justify-end gap-1">
                    <Button
                      size="sm"
                      variant="ghost"
                      onClick={() => {
                        setCommentMode(false);
                        setCommentText("");
                      }}
                    >
                      取消
                    </Button>
                    <Button size="sm" onClick={handleCreateNote} disabled={creating}>
                      提交
                    </Button>
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
