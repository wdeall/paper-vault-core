// 库目录与备份管理
import { useEffect, useState } from "react";
import { FolderOpen, Save, Loader2, Database } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useUIStore } from "@/stores/ui";
import { api } from "@/lib/api";
import type { IndexStatusSummary, VaultInfo } from "@/types";

export function VaultSettings() {
  const showToast = useUIStore((s) => s.showToast);
  const [info, setInfo] = useState<VaultInfo | null>(null);
  const [fts, setFts] = useState<IndexStatusSummary | null>(null);
  const [loading, setLoading] = useState(true);
  const [backingUp, setBackingUp] = useState(false);
  const [reindexing, setReindexing] = useState(false);

  useEffect(() => {
    void (async () => {
      try {
        const [v, f] = await Promise.all([api.getVaultInfo(), api.getFtsStatus()]);
        setInfo(v);
        setFts(f);
      } catch (e) {
        showToast("error", `加载失败: ${(e as Error).message}`);
      } finally {
        setLoading(false);
      }
    })();
  }, [showToast]);

  async function handleOpen() {
    try {
      await api.openVaultFolder();
    } catch (e) {
      showToast("error", `打开失败: ${(e as Error).message}`);
    }
  }

  async function handleBackup() {
    setBackingUp(true);
    try {
      const path = await api.backupDatabase();
      showToast("success", `已备份: ${path}`);
    } catch (e) {
      showToast("error", `备份失败: ${(e as Error).message}`);
    } finally {
      setBackingUp(false);
    }
  }

  async function handleReindex() {
    setReindexing(true);
    try {
      await api.reindexAll();
      showToast("info", "已加入重建索引队列");
      const f = await api.getFtsStatus();
      setFts(f);
    } catch (e) {
      showToast("error", `重建失败: ${(e as Error).message}`);
    } finally {
      setReindexing(false);
    }
  }

  if (loading || !info) {
    return (
      <Card className="flex items-center gap-2 p-4 text-muted-foreground">
        <Loader2 className="h-4 w-4 animate-spin" />
        加载库信息…
      </Card>
    );
  }

  return (
    <Card className="p-4">
      <h2 className="mb-3 text-lg font-medium">库与备份</h2>
      <div className="space-y-3">
        <div className="rounded border border-border bg-muted/30 p-3 text-sm">
          <div className="mb-1 text-muted-foreground">库目录</div>
          <div className="break-all font-mono text-xs">{info.path}</div>
          <div className="mt-2 flex gap-2">
            <Badge variant="secondary">{info.paper_count} 篇论文</Badge>
            <Badge variant="secondary">{info.indexed_count} 篇已索引</Badge>
          </div>
        </div>

        {fts && (
          <div className="rounded border border-border bg-muted/30 p-3 text-sm">
            <div className="mb-1 text-muted-foreground">全文索引状态</div>
            <div className="flex flex-wrap gap-2">
              <Badge variant="outline">总计 {fts.total}</Badge>
              <Badge>已索引 {fts.indexed}</Badge>
              <Badge variant="secondary">索引中 {fts.indexing}</Badge>
              <Badge variant="outline">未索引 {fts.pending}</Badge>
              <Badge variant="destructive">失败 {fts.failed}</Badge>
            </div>
          </div>
        )}

        <div className="flex flex-wrap gap-2">
          <Button variant="outline" onClick={handleOpen}>
            <FolderOpen className="mr-1.5 h-4 w-4" />
            打开库目录
          </Button>
          <Button variant="outline" onClick={handleBackup} disabled={backingUp}>
            {backingUp ? (
              <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
            ) : (
              <Database className="mr-1.5 h-4 w-4" />
            )}
            备份数据库
          </Button>
          <Button variant="outline" onClick={handleReindex} disabled={reindexing}>
            {reindexing ? (
              <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
            ) : (
              <Save className="mr-1.5 h-4 w-4" />
            )}
            重建全文索引
          </Button>
        </div>

        <div className="rounded border border-border bg-muted/30 p-3 text-xs text-muted-foreground">
          <p>
            备份仅复制 SQLite 数据库，不重复复制 PDF 和 Markdown。
            数据库备份保存为 <code>backups/papers-YYYYMMDD-HHMMSS.db</code>。
          </p>
          <p className="mt-1">
            v1 不做自动云同步。备份或复制 PaperVault/ 整个目录即可完成迁移。
          </p>
        </div>
      </div>
    </Card>
  );
}
