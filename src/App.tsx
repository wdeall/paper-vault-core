import { useEffect, useState } from "react";
import { HashRouter, Navigate, Route, Routes } from "react-router-dom";
import { open } from "@tauri-apps/plugin-dialog";
import { api, ApiError } from "@/lib/api";
import { isTauri } from "@/lib/tauri";
import { useUIStore } from "@/stores/ui";
import { usePaperStore } from "@/stores/paper";
import { LibraryPage } from "@/routes/LibraryPage";
import { ReaderPage } from "@/routes/ReaderPage";
import { SettingsPage } from "@/routes/SettingsPage";
import { VaultInitDialog } from "@/components/VaultInitDialog";
import { Toaster } from "@/components/Toaster";

export function App() {
  const [vaultReady, setVaultReady] = useState<boolean | null>(null);
  const showToast = useUIStore((s) => s.showToast);

  useEffect(() => {
    checkVault();
  }, []);

  async function checkVault() {
    if (!isTauri()) {
      // 浏览器预览模式，跳过 vault 检查
      setVaultReady(false);
      return;
    }
    try {
      await api.getVaultInfo();
      setVaultReady(true);
      await usePaperStore.getState().loadPapers();
    } catch (e) {
      if (e instanceof ApiError && e.kind === "config") {
        setVaultReady(false);
      } else {
        showToast("error", `初始化失败: ${(e as Error).message}`);
        setVaultReady(false);
      }
    }
  }

  async function handleInit(path: string) {
    try {
      await api.initVault(path);
      setVaultReady(true);
      showToast("success", "库已创建");
    } catch (e) {
      showToast("error", `建库失败: ${(e as Error).message}`);
    }
  }

  async function pickAndInit() {
    if (!isTauri()) {
      showToast("warning", "请在 Tauri 桌面应用中执行此操作");
      return;
    }
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: "选择 PaperVault 库目录",
      });
      if (typeof selected === "string") {
        await handleInit(selected);
      }
    } catch (e) {
      showToast("error", `选择目录失败: ${(e as Error).message}`);
    }
  }

  if (vaultReady === null) {
    return (
      <div className="flex h-screen items-center justify-center text-muted-foreground">
        加载中…
      </div>
    );
  }

  if (!vaultReady) {
    return <VaultInitDialog onInit={handleInit} onPick={pickAndInit} />;
  }

  return (
    <HashRouter>
      <Routes>
        <Route path="/" element={<Navigate to="/library" replace />} />
        <Route path="/library" element={<LibraryPage />} />
        <Route path="/reader/:id" element={<ReaderPage />} />
        <Route path="/settings" element={<SettingsPage />} />
      </Routes>
      <Toaster />
    </HashRouter>
  );
}
