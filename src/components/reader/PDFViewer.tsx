// PDF 阅读器（pdf.js 渲染 + 自适应容器宽度）
import { useEffect, useState, useRef, useCallback } from "react";
import {
  Loader2,
  ZoomIn,
  ZoomOut,
  ExternalLink,
  ChevronLeft,
  ChevronRight,
  Maximize,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import { basename } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useUIStore } from "@/stores/ui";

import * as pdfjsLib from "pdfjs-dist";
import type { PDFDocumentProxy, RenderTask } from "pdfjs-dist";
import PdfWorker from "pdfjs-dist/build/pdf.worker.min.mjs?url";

pdfjsLib.GlobalWorkerOptions.workerSrc = PdfWorker;

interface Props {
  paperId: string;
  src: string;
  initialPage: number;
  onPageChange: (page: number) => void;
  onTotalPages: (n: number) => void;
}

export function PDFViewer({
  paperId,
  src,
  initialPage,
  onPageChange,
  onTotalPages,
}: Props) {
  const showToast = useUIStore((s) => s.showToast);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(100);
  const [currentPage, setCurrentPage] = useState(initialPage);
  const [totalPages, setTotalPages] = useState(0);
  const [pageInput, setPageInput] = useState(initialPage);
  const [docReady, setDocReady] = useState(false);
  // 自适应容器宽度
  const [containerWidth, setContainerWidth] = useState(0);
  const [fitWidth, setFitWidth] = useState(true); // 默认自适应

  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const pdfDocRef = useRef<PDFDocumentProxy | null>(null);
  const renderTaskRef = useRef<RenderTask | null>(null);
  // 记录当前页原始尺寸（scale=1 时的 viewport），用于自适应计算
  const pageBaseRef = useRef<{ width: number; height: number } | null>(null);

  // 回调 ref，避免触发 effect 重新执行
  const onTotalPagesRef = useRef(onTotalPages);
  onTotalPagesRef.current = onTotalPages;
  const onPageChangeRef = useRef(onPageChange);
  onPageChangeRef.current = onPageChange;

  // 同步 initialPage → currentPage（用于跳转）
  useEffect(() => {
    setCurrentPage((prev) => (prev !== initialPage ? initialPage : prev));
    setPageInput((prev) => (prev !== initialPage ? initialPage : prev));
  }, [initialPage]);

  // ResizeObserver：监听容器宽度变化
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    setContainerWidth(el.clientWidth);
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) {
        setContainerWidth(e.contentRect.width);
      }
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

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
      if (pdfDocRef.current) {
        void pdfDocRef.current.destroy();
        pdfDocRef.current = null;
      }
    };
  }, [paperId, src]);

  // 计算实际渲染 scale
  const effectiveScale = (() => {
    if (fitWidth && containerWidth > 0 && pageBaseRef.current) {
      return containerWidth / pageBaseRef.current.width;
    }
    return zoom / 100;
  })();

  // 渲染当前页
  useEffect(() => {
    if (!docReady) return;
    const doc = pdfDocRef.current;
    const canvas = canvasRef.current;
    if (!doc || !canvas) return;

    let cancelled = false;

    if (renderTaskRef.current) {
      renderTaskRef.current.cancel();
      renderTaskRef.current = null;
    }

    (async () => {
      try {
        const page = await doc.getPage(currentPage);
        if (cancelled) return;

        // 记录原始尺寸（scale=1）
        const baseViewport = page.getViewport({ scale: 1 });
        pageBaseRef.current = {
          width: baseViewport.width,
          height: baseViewport.height,
        };

        // 若开启自适应且 containerWidth 已就绪，按容器宽度算 scale
        let scale = effectiveScale;
        if (fitWidth && containerWidth > 0) {
          scale = containerWidth / baseViewport.width;
        }

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
      } catch (e) {
        if (cancelled) return;
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
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [docReady, currentPage, effectiveScale, fitWidth, containerWidth]);

  const goToPage = useCallback(
    (page: number) => {
      const next = Math.max(1, Math.min(totalPages || 1, page));
      setCurrentPage(next);
      setPageInput(next);
      onPageChangeRef.current(next);
    },
    [totalPages],
  );

  // 手动 zoom：关闭自适应
  const handleZoomOut = useCallback(() => {
    setFitWidth(false);
    setZoom((z) => Math.max(50, z - 10));
  }, []);

  const handleZoomIn = useCallback(() => {
    setFitWidth(false);
    setZoom((z) => Math.min(300, z + 10));
  }, []);

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
        <Button size="icon" variant="ghost" onClick={handleZoomOut}>
          <ZoomOut className="h-4 w-4" />
        </Button>
        <span className="w-12 text-center text-xs">
          {fitWidth ? "适配" : `${zoom}%`}
        </span>
        <Button size="icon" variant="ghost" onClick={handleZoomIn}>
          <ZoomIn className="h-4 w-4" />
        </Button>
        <Button
          size="icon"
          variant={fitWidth ? "default" : "ghost"}
          onClick={() => setFitWidth(true)}
          title="适配宽度"
        >
          <Maximize className="h-4 w-4" />
        </Button>
        <div className="ml-auto">
          <Button size="icon" variant="ghost" onClick={openNative} disabled={!src}>
            <ExternalLink className="h-4 w-4" />
          </Button>
        </div>
      </div>

      {/* PDF 渲染区 */}
      <div
        ref={containerRef}
        className="relative flex-1 overflow-auto bg-muted/30"
      >
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
          <div className={cn("flex min-h-full items-start justify-center")}>
            <div className="pdf-page">
              <canvas ref={canvasRef} />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
