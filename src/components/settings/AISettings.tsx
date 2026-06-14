// AI 提供商配置（OpenAI 兼容）
import { useEffect, useState } from "react";
import { Save, Loader2, Eye, EyeOff } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Card } from "@/components/ui/card";
import { useSettingsStore } from "@/stores/settings";
import { useUIStore } from "@/stores/ui";

export function AISettings() {
  const aiConfig = useSettingsStore((s) => s.aiConfig);
  const loadConfig = useSettingsStore((s) => s.loadConfig);
  const saveConfig = useSettingsStore((s) => s.saveConfig);
  const showToast = useUIStore((s) => s.showToast);
  const [baseUrl, setBaseUrl] = useState("");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState("");
  const [saving, setSaving] = useState(false);
  const [showKey, setShowKey] = useState(false);

  useEffect(() => {
    loadConfig();
  }, [loadConfig]);

  useEffect(() => {
    setBaseUrl(aiConfig.base_url);
    setApiKey(aiConfig.api_key);
    setModel(aiConfig.model);
  }, [aiConfig]);

  async function handleSave() {
    setSaving(true);
    try {
      await saveConfig({ base_url: baseUrl, api_key: apiKey, model });
      showToast("success", "AI 配置已保存");
    } catch (e) {
      showToast("error", `保存失败: ${(e as Error).message}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Card className="p-4">
      <div className="mb-3">
        <h2 className="text-lg font-medium">AI 提供商</h2>
        <p className="text-xs text-muted-foreground">
          使用 OpenAI 兼容的 API。配置仅保存在本地 <code>papers.db</code> 中。
        </p>
      </div>
      <div className="space-y-3">
        <div>
          <Label className="mb-1 block text-xs">Base URL</Label>
          <Input
            value={baseUrl}
            onChange={(e) => setBaseUrl(e.target.value)}
            placeholder="https://api.openai.com/v1"
          />
        </div>
        <div>
          <Label className="mb-1 block text-xs">API Key</Label>
          <div className="flex gap-1">
            <Input
              type={showKey ? "text" : "password"}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
              placeholder="sk-..."
            />
            <Button
              type="button"
              variant="ghost"
              size="icon"
              onClick={() => setShowKey((s) => !s)}
            >
              {showKey ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            </Button>
          </div>
        </div>
        <div>
          <Label className="mb-1 block text-xs">模型</Label>
          <Input
            value={model}
            onChange={(e) => setModel(e.target.value)}
            placeholder="gpt-4o-mini / claude-3-5-sonnet / ..."
          />
        </div>
        <Button onClick={handleSave} disabled={saving}>
          {saving ? (
            <>
              <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
              保存中…
            </>
          ) : (
            <>
              <Save className="mr-1.5 h-4 w-4" />
              保存
            </>
          )}
        </Button>
      </div>
    </Card>
  );
}
