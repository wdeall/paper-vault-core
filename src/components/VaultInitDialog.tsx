// 库选择对话框（启动时若未选库）

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { api } from "@/lib/api";
import { useUIStore } from "@/stores/ui";

interface Props {
  onInit: (path: string) => Promise<void>;
  onPick: () => Promise<void>;
}

export function VaultInitDialog({ onInit, onPick }: Props) {
  const [path, setPath] = useState("");
  const [busy, setBusy] = useState(false);
  const showToast = useUIStore((s) => s.showToast);

  async function loadDemo() {
    if (!path.trim()) {
      showToast("warning", "请先选择库目录");
      return;
    }
    setBusy(true);
    try {
      await onInit(path);
      await api.loadSeedData();
      showToast("success", "示例数据已加载");
    } catch (e) {
      showToast("error", `加载示例失败: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex h-screen flex-col items-center justify-center gap-6 bg-background p-8">
      <div className="text-center">
        <h1 className="mb-2 text-3xl font-bold">PaperVault</h1>
        <p className="text-muted-foreground">本地论文阅读与 Markdown 笔记</p>
      </div>
      <div className="w-full max-w-md space-y-3 rounded-lg border bg-card p-6 shadow">
        <p className="text-sm text-muted-foreground">
          请选择一个文件夹作为 PaperVault 库目录。所有 PDF、笔记、数据库将存放在该目录下。
        </p>
        <div className="flex gap-2">
          <Input
            value={path}
            onChange={(e) => setPath(e.target.value)}
            placeholder="D:\Documents\PaperVault"
            className="flex-1"
          />
          <Button onClick={onPick} variant="outline">
            浏览
          </Button>
        </div>
        <div className="flex gap-2">
          <Button
            onClick={() => path.trim() && onInit(path)}
            disabled={!path.trim() || busy}
            className="flex-1"
          >
            创建 / 打开库
          </Button>
          <Button onClick={loadDemo} disabled={!path.trim() || busy} variant="secondary">
            加载示例数据
          </Button>
        </div>
      </div>
      <p className="text-xs text-muted-foreground">
        库目录可随时在设置中更换。建议放在云盘同步目录便于备份。
      </p>
    </div>
  );
}
