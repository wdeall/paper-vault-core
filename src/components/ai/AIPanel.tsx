// AI 面板（论文详情页 AI 标签页）
//
// v2：preset 快捷功能和对话已移动到阅读器的 AI 对话侧边栏（AgentChatSidebar）。
// 此处仅保留提示与设置入口。
import { Sparkles, ArrowRight } from "lucide-react";
import { Card } from "@/components/ui/card";
import { useNavigate } from "react-router-dom";

interface Props {
  paperId: string;
  hasNote: boolean;
}

export function AIPanel({ paperId: _paperId, hasNote: _hasNote }: Props) {
  const navigate = useNavigate();
  return (
    <div className="space-y-3">
      <Card className="border-border bg-muted/30 p-3">
        <div className="mb-1 flex items-center gap-2 text-sm font-medium">
          <Sparkles className="h-4 w-4 text-primary" />
          AI 辅助整理
        </div>
        <p className="text-xs text-muted-foreground">
          AI 对话与快捷功能（总结论文、翻译摘要、自动建笔记、复现计划等）已移动到阅读器的 AI 对话侧边栏。
          打开论文阅读器后，右侧第三栏即为 AI 对话侧边栏。
        </p>
      </Card>

      <Card className="p-3 text-xs text-muted-foreground">
        <p>
          修改提示词请前往{" "}
          <button
            onClick={() => navigate("/settings")}
            className="text-primary underline-offset-2 hover:underline"
          >
            设置 {"->"} AI Skill 预设
          </button>
          。
        </p>
      </Card>

      <Card className="p-3 text-xs">
        <button
          onClick={() => navigate(`/reader/${_paperId}`)}
          className="flex w-full items-center justify-between text-primary hover:underline"
        >
          <span>打开阅读器使用 AI 对话</span>
          <ArrowRight className="h-3 w-3" />
        </button>
      </Card>
    </div>
  );
}
