import { useState } from "react";
import {
  Sparkles,
  FileText,
  Languages,
  ListChecks,
  Loader2,
  Wand2,
  CheckCircle2,
  Hash,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { useUIStore } from "@/stores/ui";
import { api } from "@/lib/api";
import type { AIResult, MetadataCandidate, PaperDetail } from "@/types";

interface Props {
  paperId: string;
  hasNote: boolean;
  onChange?: React.Dispatch<React.SetStateAction<PaperDetail | null>>;
}

interface PresetAction {
  presetId: string;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  description: string;
  writesAiBlock?: "summary" | "key_points";
}

const ACTIONS: PresetAction[] = [
  {
    presetId: "metadata_from_pdf",
    label: "提取元数据",
    icon: Wand2,
    description: "从 PDF 首页和正文识别标题、作者、DOI、摘要和关键词。",
  },
  {
    presetId: "abstract_translate",
    label: "翻译摘要",
    icon: Languages,
    description: "把当前论文摘要翻译成中文并保留关键术语。",
  },
  {
    presetId: "paper_summary",
    label: "总结论文",
    icon: FileText,
    description: "总结研究问题、方法、实验、结论和局限。",
    writesAiBlock: "summary",
  },
  {
    presetId: "create_reading_note",
    label: "自动建笔记",
    icon: ListChecks,
    description: "生成结构化阅读笔记并写入 Markdown。",
    writesAiBlock: "key_points",
  },
  {
    presetId: "related_papers_lookup",
    label: "查找相关论文",
    icon: Hash,
    description: "基于标题、DOI 和关键词查找相关研究。",
  },
];

export function AIPanel({ paperId, hasNote, onChange }: Props) {
  const showToast = useUIStore((s) => s.showToast);
  const [running, setRunning] = useState<string | null>(null);
  const [lastResult, setLastResult] = useState<{ label: string; result: AIResult } | null>(null);

  async function ensureNote(): Promise<boolean> {
    if (hasNote) return true;
    try {
      await api.createNote(paperId);
      const detail = await api.getPaper(paperId);
      onChange?.(detail);
      return true;
    } catch (e) {
      showToast("error", `创建笔记失败: ${(e as Error).message}`);
      return false;
    }
  }

  async function runAction(action: PresetAction) {
    setRunning(action.presetId);
    setLastResult(null);
    try {
      if (action.presetId === "metadata_from_pdf") {
        const result = await api.runAi(action.presetId, paperId);
        const candidate = extractMetadataCandidate(result);
        if (!candidate) {
          throw new Error("AI 未返回可识别的元数据 JSON");
        }
        const detail = await api.getPaper(paperId);
        await api.updatePaper(paperId, {
          ...detail,
          title: candidate.title || detail.title,
          authors: candidate.authors.length ? candidate.authors : detail.authors,
          year: candidate.year ?? detail.year,
          venue: candidate.venue || detail.venue,
          doi: candidate.doi || detail.doi,
          abstract_text: candidate.abstract_text || detail.abstract_text,
          keywords: candidate.keywords.length ? candidate.keywords : detail.keywords,
        });
        const refreshed = await api.getPaper(paperId);
        onChange?.(refreshed);
        setLastResult({ label: action.label, result });
        showToast("success", "提取元数据完成，已自动更新论文条目");
        return;
      }

      if (action.writesAiBlock || action.presetId === "create_reading_note") {
        const ok = await ensureNote();
        if (!ok) return;
      }

      const result = await api.runAi(action.presetId, paperId);
      setLastResult({ label: action.label, result });

      if (action.writesAiBlock) {
        const content = extractBlock(result);
        if (content) {
          await api.updateNoteAiBlock(paperId, action.writesAiBlock, content);
          showToast("success", `${action.label} 已写入笔记`);
        } else {
          showToast("success", `${action.label} 完成`);
        }
      } else {
        showToast("success", `${action.label} 完成`);
      }
    } catch (e) {
      showToast("error", `${action.label} 失败: ${(e as Error).message}`);
    } finally {
      setRunning(null);
    }
  }

  return (
    <div className="space-y-3">
      <Card className="border-border bg-muted/30 p-3">
        <div className="mb-1 flex items-center gap-2 text-sm font-medium">
          <Sparkles className="h-4 w-4 text-primary" />
          AI 辅助整理
        </div>
        <p className="text-xs text-muted-foreground">
          AI 结果默认显示在下方固定结果区；涉及笔记写入的操作只更新 AI 区块，不覆盖手写内容。
        </p>
      </Card>

      {ACTIONS.map((action) => {
        const Icon = action.icon;
        const isRunning = running === action.presetId;
        return (
          <Card key={action.presetId} className="p-3">
            <div className="mb-1 flex items-center gap-2">
              <Icon className="h-4 w-4" />
              <span className="text-sm font-medium">{action.label}</span>
            </div>
            <p className="mb-2 text-xs text-muted-foreground">{action.description}</p>
            <Button
              size="sm"
              variant="outline"
              disabled={!!running}
              onClick={() => runAction(action)}
              className="w-full"
            >
              {isRunning ? (
                <>
                  <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />
                  运行中
                </>
              ) : (
                <>运行</>
              )}
            </Button>
          </Card>
        );
      })}

      {lastResult && (
        <Card className="border-primary/40 bg-primary/5 p-3">
          <div className="mb-2 flex items-center gap-2 text-sm font-medium text-primary">
            <CheckCircle2 className="h-4 w-4" />
            {lastResult.label} 完成
          </div>
          <pre className="max-h-60 overflow-auto whitespace-pre-wrap break-words text-xs">
            {lastResult.result.markdown || lastResult.result.raw || "（无返回内容）"}
          </pre>
        </Card>
      )}

      <Card className="p-3 text-xs text-muted-foreground">
        <p>
          修改提示词请前往{" "}
          <a
            href="#/settings"
            className="text-primary underline-offset-2 hover:underline"
          >
            设置 {"->"} AI Skill 预设
          </a>
          。
        </p>
      </Card>
    </div>
  );
}

function extractBlock(result: AIResult): string {
  if (result.markdown && result.markdown.trim()) return result.markdown;
  if (result.raw && result.raw.trim()) return result.raw;
  if (result.parsed) {
    const keys = ["summary", "key_points", "content", "text"];
    for (const key of keys) {
      const value = result.parsed[key];
      if (typeof value === "string") return value;
    }
    return JSON.stringify(result.parsed, null, 2);
  }
  return "";
}

function extractMetadataCandidate(result: AIResult): MetadataCandidate | null {
  const parsed = result.parsed ?? parseJsonFromText(result.raw || result.markdown);
  if (!parsed || typeof parsed !== "object") return null;

  const obj = parsed as Record<string, unknown>;
  return {
    title: typeof obj.title === "string" ? obj.title : "",
    authors: Array.isArray(obj.authors)
      ? obj.authors.filter((value): value is string => typeof value === "string")
      : [],
    year: typeof obj.year === "number" ? obj.year : null,
    venue: typeof obj.venue === "string" ? obj.venue : "",
    doi: typeof obj.doi === "string" ? obj.doi : "",
    abstract_text: typeof obj.abstract_text === "string" ? obj.abstract_text : "",
    keywords: Array.isArray(obj.keywords)
      ? obj.keywords.filter((value): value is string => typeof value === "string")
      : [],
    source: "ai",
    confidence: "medium",
  };
}

function parseJsonFromText(text: string): Record<string, unknown> | null {
  if (!text.trim()) return null;
  const start = text.indexOf("{");
  const end = text.lastIndexOf("}");
  if (start < 0 || end <= start) return null;
  try {
    return JSON.parse(text.slice(start, end + 1)) as Record<string, unknown>;
  } catch {
    return null;
  }
}
