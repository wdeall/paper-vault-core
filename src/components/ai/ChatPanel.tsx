// AI 对话面板：多轮对话，以论文元数据为上下文
import { useState, useRef, useEffect } from "react";
import { Send, Loader2, Trash2, MessageSquare } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Card } from "@/components/ui/card";
import { api } from "@/lib/api";
import { useUIStore } from "@/stores/ui";
import { cn } from "@/lib/utils";

interface Props {
  paperId: string;
}

interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  timestamp: number;
}

export function ChatPanel({ paperId }: Props) {
  const showToast = useUIStore((s) => s.showToast);
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const messagesEndRef = useRef<HTMLDivElement | null>(null);

  // 切换论文时清空历史
  useEffect(() => {
    setMessages([]);
    setInput("");
  }, [paperId]);

  // 自动滚动到底部
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  async function handleSend() {
    const text = input.trim();
    if (!text || loading) return;
    const userMsg: ChatMessage = {
      role: "user",
      content: text,
      timestamp: Date.now(),
    };
    setMessages((m) => [...m, userMsg]);
    setInput("");
    setLoading(true);
    try {
      const history = messages.map((m) => ({ role: m.role, content: m.content }));
      const result = await api.chatWithPaper(paperId, text, history);
      const aiMsg: ChatMessage = {
        role: "assistant",
        content: result,
        timestamp: Date.now(),
      };
      setMessages((m) => [...m, aiMsg]);
    } catch (e) {
      showToast("error", `对话失败: ${(e as Error).message}`);
    } finally {
      setLoading(false);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      void handleSend();
    }
  }

  function handleClear() {
    setMessages([]);
  }

  return (
    <Card className="flex flex-col p-0">
      <div className="flex h-8 shrink-0 items-center justify-between border-b border-border px-3">
        <div className="flex items-center gap-1.5 text-xs font-medium">
          <MessageSquare className="h-3.5 w-3.5" />
          AI 对话
        </div>
        {messages.length > 0 && (
          <Button
            size="icon"
            variant="ghost"
            className="h-6 w-6"
            onClick={handleClear}
            title="清空对话"
          >
            <Trash2 className="h-3 w-3" />
          </Button>
        )}
      </div>

      <div className="flex max-h-[400px] min-h-[200px] flex-1 flex-col gap-2 overflow-y-auto p-3">
        {messages.length === 0 ? (
          <div className="flex flex-1 items-center justify-center text-center text-xs text-muted-foreground">
            <div>
              <MessageSquare className="mx-auto mb-2 h-8 w-8 opacity-30" />
              <p>向 AI 提问关于这篇论文的任何问题</p>
              <p className="mt-1 opacity-70">Enter 发送 / Shift+Enter 换行</p>
            </div>
          </div>
        ) : (
          messages.map((m, i) => (
            <div
              key={i}
              className={cn(
                "flex",
                m.role === "user" ? "justify-end" : "justify-start",
              )}
            >
              <div
                className={cn(
                  "max-w-[85%] whitespace-pre-wrap break-words rounded-md px-3 py-1.5 text-xs",
                  m.role === "user"
                    ? "bg-primary text-primary-foreground"
                    : "bg-muted text-foreground",
                )}
              >
                {m.content}
              </div>
            </div>
          ))
        )}
        {loading && (
          <div className="flex justify-start">
            <div className="flex items-center gap-1.5 rounded-md bg-muted px-3 py-1.5 text-xs text-muted-foreground">
              <Loader2 className="h-3 w-3 animate-spin" />
              AI 思考中…
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      <div className="border-t border-border p-2">
        <Textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="提问关于这篇论文的问题…"
          className="min-h-[60px] resize-none text-xs"
          disabled={loading}
        />
        <div className="mt-1 flex justify-end">
          <Button
            size="sm"
            onClick={handleSend}
            disabled={loading || !input.trim()}
          >
            <Send className="mr-1 h-3 w-3" />
            发送
          </Button>
        </div>
      </div>
    </Card>
  );
}
