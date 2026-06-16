// AI Skill 预设管理
import { useEffect, useState } from "react";
import {
  Save,
  Loader2,
  RotateCcw,
  ChevronDown,
  ChevronRight,
  Sparkles,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { Card } from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { useSettingsStore } from "@/stores/settings";
import { useUIStore } from "@/stores/ui";
import { cn } from "@/lib/utils";
import type { AISkillPreset } from "@/types";

export function PresetManager() {
  const presets = useSettingsStore((s) => s.presets);
  const loadPresets = useSettingsStore((s) => s.loadPresets);
  const savePreset = useSettingsStore((s) => s.savePreset);
  const resetPreset = useSettingsStore((s) => s.resetPreset);
  const showToast = useUIStore((s) => s.showToast);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [editing, setEditing] = useState<AISkillPreset | null>(null);
  const [saving, setSaving] = useState(false);
  const [resetting, setResetting] = useState<string | null>(null);

  useEffect(() => {
    loadPresets();
  }, [loadPresets]);

  async function handleSave() {
    if (!editing) return;
    setSaving(true);
    try {
      await savePreset(editing.id, editing);
      showToast("success", "预设已保存");
      setEditing(null);
    } catch (e) {
      showToast("error", `保存失败: ${(e as Error).message}`);
    } finally {
      setSaving(false);
    }
  }

  async function handleReset(id: string) {
    setResetting(id);
    try {
      await resetPreset(id);
      showToast("success", "已恢复默认");
      if (editing?.id === id) setEditing(null);
    } catch (e) {
      showToast("error", `重置失败: ${(e as Error).message}`);
    } finally {
      setResetting(null);
    }
  }

  return (
    <Card className="p-4">
      <div className="mb-3">
        <h2 className="text-lg font-medium">AI Skill 预设</h2>
        <p className="text-xs text-muted-foreground">
          每个预设包含一个 system prompt 和一个 user prompt 模板。用户修改的版本会单独保存，可随时恢复默认。
        </p>
      </div>
      <ul className="space-y-2">
        {presets.map((p) => {
          const isExpanded = expanded === p.id;
          return (
            <li
              key={p.id}
              className={cn(
                "rounded border border-border bg-card p-3",
                isExpanded && "border-primary/40",
              )}
            >
              <button
                className="flex w-full items-center gap-2 text-left"
                onClick={() => setExpanded(isExpanded ? null : p.id)}
              >
                {isExpanded ? (
                  <ChevronDown className="h-4 w-4" />
                ) : (
                  <ChevronRight className="h-4 w-4" />
                )}
                <Sparkles className="h-4 w-4 text-primary" />
                <span className="font-medium">{p.name}</span>
                <Badge variant="secondary" className="text-[10px]">
                  {p.bound_action}
                </Badge>
                {p.is_builtin ? (
                  <Badge variant="outline" className="text-[10px]">
                    内置
                  </Badge>
                ) : (
                  <Badge className="text-[10px]">已自定义</Badge>
                )}
              </button>

              {isExpanded && (
                <div className="mt-3 space-y-2 text-xs">
                  <p className="text-muted-foreground">{p.skill}</p>
                  <div>
                    <div className="mb-1 text-muted-foreground">System Prompt</div>
                    <pre className="max-h-32 overflow-auto whitespace-pre-wrap rounded bg-muted/40 p-2 text-[11px]">
                      {p.system_prompt}
                    </pre>
                  </div>
                  <div>
                    <div className="mb-1 text-muted-foreground">User Template</div>
                    <pre className="max-h-32 overflow-auto whitespace-pre-wrap rounded bg-muted/40 p-2 text-[11px]">
                      {p.user_template}
                    </pre>
                  </div>
                  <div className="flex gap-2 pt-1">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => setEditing({ ...p })}
                    >
                      编辑
                    </Button>
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={!p.is_builtin || resetting === p.id}
                      onClick={() => handleReset(p.id)}
                    >
                      {resetting === p.id ? (
                        <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                      ) : (
                        <RotateCcw className="mr-1.5 h-3.5 w-3.5" />
                      )}
                      恢复默认
                    </Button>
                  </div>
                </div>
              )}
            </li>
          );
        })}
      </ul>

      {editing && (
        <div className="mt-4 rounded border border-primary/40 bg-primary/5 p-3">
          <h3 className="mb-2 text-sm font-medium">编辑: {editing.name}</h3>
          <div className="space-y-2">
            <div>
              <Label className="mb-1 block text-xs">名称</Label>
              <Input
                value={editing.name}
                onChange={(e) => setEditing({ ...editing, name: e.target.value })}
              />
            </div>
            <div>
              <Label className="mb-1 block text-xs">System Prompt</Label>
              <Textarea
                rows={5}
                value={editing.system_prompt}
                onChange={(e) =>
                  setEditing({ ...editing, system_prompt: e.target.value })
                }
              />
            </div>
            <div>
              <Label className="mb-1 block text-xs">User Template</Label>
              <Textarea
                rows={5}
                value={editing.user_template}
                onChange={(e) =>
                  setEditing({ ...editing, user_template: e.target.value })
                }
              />
            </div>
            <div className="flex items-center gap-2 text-xs">
              <label className="flex items-center gap-1">
                <input
                  type="checkbox"
                  checked={editing.auto_write}
                  onChange={(e) =>
                    setEditing({ ...editing, auto_write: e.target.checked })
                  }
                />
                自动写入笔记 AI 区块
              </label>
              <span className="text-muted-foreground">
                输出格式: {editing.output_format}
              </span>
            </div>
            <div className="flex gap-2 pt-1">
              <Button onClick={handleSave} disabled={saving} size="sm">
                {saving ? (
                  <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                ) : (
                  <Save className="mr-1.5 h-3.5 w-3.5" />
                )}
                保存
              </Button>
              <Button variant="ghost" size="sm" onClick={() => setEditing(null)}>
                取消
              </Button>
            </div>
          </div>
        </div>
      )}
    </Card>
  );
}
