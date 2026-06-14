import { useEffect, useState } from "react";
import { ExternalLink, Loader2, Search, ZoomIn, ZoomOut } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import { basename } from "@/lib/tauri";
import { cn } from "@/lib/utils";
import { useUIStore } from "@/stores/ui";

interface Props {
  paperId: string;
  src: string;
  initialPage: number;
  onPageChange: (page: number) => void;
  onTotalPages: (n: number) => void;
}

export function PDFViewer({ paperId, src, initialPage, onPageChange, onTotalPages }: Props) {
  const showToast = useUIStore((s) => s.showToast);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(100);
  const [pageInput, setPageInput] = useState(initialPage);
  const [search, setSearch] = useState("");
  const [blobUrl, setBlobUrl] = useState("");

  useEffect(() => {
    setPageInput(initialPage);
  }, [initialPage]);

  useEffect(() => {
    setLoading(true);
    setError(null);
    onTotalPages(0);
  }, [src]);

  useEffect(() => {
    let revokedUrl = "";
    let cancelled = false;
    let fallbackTimer: ReturnType<typeof setTimeout> | null = null;

    setBlobUrl("");
    setLoading(true);
    setError(null);

    (async () => {
      try {
        if (!src) {
          throw new Error("PDF 路径为空");
        }
        const bytes = await api.readPdfBytes(paperId);
        if (cancelled) return;
        const blob = new Blob([new Uint8Array(bytes)], { type: "application/pdf" });
        revokedUrl = URL.createObjectURL(blob);
        setBlobUrl(revokedUrl);
        setLoading(false);
        fallbackTimer = setTimeout(() => {
          if (!cancelled) {
            setLoading(false);
          }
        }, 800);
      } catch (e) {
        if (cancelled) return;
        setError((e as Error).message || String(e));
        setLoading(false);
      }
    })();

    return () => {
      cancelled = true;
      if (fallbackTimer) {
        clearTimeout(fallbackTimer);
      }
      if (revokedUrl) {
        URL.revokeObjectURL(revokedUrl);
      }
    };
  }, [paperId, src]);

  function handleIframeLoad() {
    setLoading(false);
  }

  function handleIframeError() {
    setLoading(false);
    setError("内置 PDF 预览加载失败");
  }

  function applyPage() {
    const next = Math.max(1, pageInput || 1);
    onPageChange(next);
    showToast("info", "当前预览模式暂不支持精确页码跳转");
  }

  function applySearch() {
    if (!search.trim()) return;
    showToast("info", "当前预览模式暂不支持页内搜索");
  }

  function openNative() {
    void api.openPdf(paperId).catch((e) => {
      showToast("error", `打开失败: ${String(e)}`);
    });
  }

  return (
    <div className="flex h-full flex-col">
      <div className="flex shrink-0 items-center gap-2 border-b border-border bg-card p-2">
        <Input
          type="number"
          value={pageInput}
          onChange={(e) => setPageInput(Number(e.target.value))}
          onBlur={applyPage}
          className="h-7 w-16 text-center text-xs"
        />
        <span className="text-xs text-muted-foreground">页</span>
        <div className="mx-2 h-5 w-px bg-border" />
        <Button size="icon" variant="ghost" onClick={() => setZoom((z) => Math.max(50, z - 10))}>
          <ZoomOut className="h-4 w-4" />
        </Button>
        <span className="w-12 text-center text-xs">{zoom}%</span>
        <Button size="icon" variant="ghost" onClick={() => setZoom((z) => Math.min(200, z + 10))}>
          <ZoomIn className="h-4 w-4" />
        </Button>
        <div className="ml-2 flex-1">
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") applySearch();
            }}
            placeholder="页内搜索（暂不支持）"
            className="h-7"
          />
        </div>
        <Button size="icon" variant="ghost" onClick={applySearch}>
          <Search className="h-4 w-4" />
        </Button>
        <Button size="icon" variant="ghost" onClick={openNative} disabled={!src}>
          <ExternalLink className="h-4 w-4" />
        </Button>
      </div>

      <div className={cn("relative flex-1 overflow-hidden bg-muted/30")}>
        {loading && (
          <div className="absolute inset-0 z-10 flex items-center justify-center gap-2 bg-background/80 text-muted-foreground">
            <Loader2 className="h-4 w-4 animate-spin" />
            加载 PDF...
          </div>
        )}

        {error ? (
          <div className="m-4 rounded border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
            加载失败: {error}
            <div className="mt-1 text-xs">文件: {basename(src)}</div>
          </div>
        ) : blobUrl ? (
          <iframe
            title={basename(src) || "pdf-preview"}
            src={blobUrl}
            onLoad={handleIframeLoad}
            onError={handleIframeError}
            className="h-full w-full border-0"
            style={{ zoom: `${zoom}%` }}
          />
        ) : (
          <div className="m-4 rounded border border-destructive/40 bg-destructive/5 p-3 text-sm text-destructive">
            加载失败: PDF 路径为空
          </div>
        )}
      </div>
    </div>
  );
}
