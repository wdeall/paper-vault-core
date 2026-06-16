// 设置页：AI / 预设 / 库 / 导出
import { useNavigate } from "react-router-dom";
import { ArrowLeft } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { AISettings } from "@/components/settings/AISettings";
import { PresetManager } from "@/components/settings/PresetManager";
import { VaultSettings } from "@/components/settings/VaultSettings";
import { ExportPanel } from "@/components/settings/ExportPanel";

export function SettingsPage() {
  const navigate = useNavigate();

  return (
    <div className="flex h-screen flex-col bg-background text-foreground">
      <header className="flex h-12 shrink-0 items-center gap-2 border-b border-border bg-card px-3">
        <Button variant="ghost" size="icon" onClick={() => navigate("/library")}>
          <ArrowLeft className="h-4 w-4" />
        </Button>
        <h1 className="text-sm font-semibold">设置</h1>
      </header>
      <main className="flex-1 overflow-y-auto p-4">
        <Tabs defaultValue="vault" className="max-w-4xl">
          <TabsList>
            <TabsTrigger value="vault">库与备份</TabsTrigger>
            <TabsTrigger value="ai">AI 配置</TabsTrigger>
            <TabsTrigger value="presets">Skill 预设</TabsTrigger>
            <TabsTrigger value="export">引用导出</TabsTrigger>
          </TabsList>
          <TabsContent value="vault" className="mt-4">
            <VaultSettings />
          </TabsContent>
          <TabsContent value="ai" className="mt-4">
            <AISettings />
          </TabsContent>
          <TabsContent value="presets" className="mt-4">
            <PresetManager />
          </TabsContent>
          <TabsContent value="export" className="mt-4">
            <ExportPanel selectedIds={[]} onClearSelection={() => {}} />
          </TabsContent>
        </Tabs>
      </main>
    </div>
  );
}
