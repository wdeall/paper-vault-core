// Agent 风格 AI 对话侧边栏（参考 opencode 等 agent 工具）
//
// 功能：
// - 多会话切换/创建/重命名/删除（按 paper_id 分组持久化）
// - 消息流：用户 / 助手消息，思考过程、上下文、preset 标记可折叠展示
// - 流式输出：监听 `ai-chat-delta` 事件，实时增量渲染
// - 快捷功能（preset）入口：点击后在对话中执行，展示发送内容/思考/上下文
// - 编辑笔记：preset 中标注 writesAiBlock 的会写入笔记 AI 区块
import { useEffect, useMemo, useRef, useState, useCallback } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  Send,
  Loader2,
  Plus,
  MessageSquare,
  Trash2,
  Pencil,
  Check,
  X,
  ChevronDown,
  ChevronRight,
  Brain,
  FileText,
  Languages,
  ListChecks,
  Wand2,
  Hash,
  FlaskConical,
  Sparkles,
  PanelLeftClose,
  ScrollText,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/textarea";
import { Input } from "@/components/ui/input";
import { Badge } from "@/components/ui/badge";
import { api } from "@/lib/api";
import { useUIStore } from "@/stores/ui";
import { useSettingsStore } from "@/stores/settings";
import { cn } from "@/lib/utils";
import type {
  AIChatDeltaEvent,
  AIChatSummarizedEvent,
  AIConversation,
  AIMessage,
} from "@/types";

interface Props {
  paperId: string;
  onClose?: () => void;
}

interface PresetAction {
  presetId: string;
  label: string;
  icon: React.ComponentType<{ className?: string }>;
  description: string;
  writesAiBlock?: "summary" | "key_points";
}

const PRESET_ACTIONS: PresetAction[] = [
  {
    presetId: "metadata_from_pdf",
    label: "提取元数据",
    icon: Wand2,
    description: "从 PDF 识别标题、作者、DOI、摘要和关键词",
  },
  {
    presetId: "abstract_translate",
    label: "翻译摘要",
    icon: Languages,
    description: "把摘要翻译成中文并保留关键术语",
  },
  {
    presetId: "paper_summary",
    label: "总结论文",
    icon: FileText,
    description: "总结研究问题、方法、实验、结论和局限",
    writesAiBlock: "summary",
  },
  {
    presetId: "create_reading_note",
    label: "自动建笔记",
    icon: ListChecks,
    description: "生成结构化阅读笔记并写入 Markdown",
    writesAiBlock: "key_points",
  },
  {
    presetId: "related_papers_lookup",
    label: "查找相关论文",
    icon: Hash,
    description: "基于标题、DOI 和关键词查找相关研究",
  },
  {
    presetId: "reproduction_plan",
    label: "复现计划",
    icon: FlaskConical,
    description: "基于论文方法部分制定代码复现计划",
    writesAiBlock: "summary",
  },
];

// 流式占位消息通过 streamingContent / streamingThinking 状态渲染（无独立 id）

export function AgentChatSidebar({ paperId, onClose }: Props) {
  const showToast = useUIStore((s) => s.showToast);
  const presets = useSettingsStore((s) => s.presets);
  const loadPresets = useSettingsStore((s) => s.loadPresets);

  const [conversations, setConversations] = useState<AIConversation[]>([]);
  const [activeConvId, setActiveConvId] = useState<string | null>(null);
  const [messages, setMessages] = useState<AIMessage[]>([]);
  const [input, setInput] = useState("");
  const [loading, setLoading] = useState(false);
  const [summarizing, setSummarizing] = useState(false);
  const [streamingContent, setStreamingContent] = useState("");
  const [streamingThinking, setStreamingThinking] = useState("");
  const [showConvList, setShowConvList] = useState(false);
  const [renamingId, setRenamingId] = useState<string | null>(null);
  const [renameValue, setRenameValue] = useState("");
  const messagesEndRef = useRef<HTMLDivElement | null>(null);

  // 加载 preset 列表（用于显示名称）
  useEffect(() => {
    void loadPresets();
  }, [loadPresets]);

  // preset id → name 映射
  const presetNameMap = useMemo(() => {
    const m = new Map<string, string>();
    for (const p of presets) m.set(p.id, p.name);
    for (const a of PRESET_ACTIONS) m.set(a.presetId, a.label);
    return m;
  }, [presets]);

  // 加载会话列表
  const loadConversations = useCallback(async () => {
    try {
      const list = await api.listAiConversations(paperId);
      setConversations(list);
      // 若无活跃会话，自动选最近的；都没有则不创建（等用户首次发送再创建）
      if (!activeConvId && list.length > 0) {
        setActiveConvId(list[0].id);
      }
    } catch (e) {
      showToast("error", `加载会话失败: ${(e as Error).message}`);
    }
  }, [paperId, activeConvId, showToast]);

  useEffect(() => {
    void loadConversations();
    // 切换论文时清空活跃会话，由 loadConversations 重新选取
    setActiveConvId(null);
    setMessages([]);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [paperId]);

  // 加载活跃会话的消息
  useEffect(() => {
    if (!activeConvId) {
      setMessages([]);
      return;
    }
    api
      .listAiMessages(activeConvId)
      .then((msgs) => setMessages(msgs))
      .catch((e) => showToast("error", `加载消息失败: ${(e as Error).message}`));
  }, [activeConvId, showToast]);

  // 监听流式 delta
  useEffect(() => {
    if (!activeConvId) return;
    const unlisten = listen<AIChatDeltaEvent>("ai-chat-delta", (event) => {
      const payload = event.payload;
      if (payload.conversation_id !== activeConvId) return;
      if (payload.delta) {
        setStreamingContent((s) => s + payload.delta);
      }
      if (payload.thinking) {
        setStreamingThinking((s) => s + payload.thinking);
      }
      if (payload.done) {
        // 完成后重新加载消息以拿到持久化的最终消息
        api
          .listAiMessages(activeConvId)
          .then((msgs) => {
            setMessages(msgs);
            setStreamingContent("");
            setStreamingThinking("");
          })
          .catch(() => {
            setStreamingContent("");
            setStreamingThinking("");
          });
      }
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [activeConvId]);

  // 监听自动总结压缩事件：后端在历史超限时触发 LLM 总结会推送此事件
  useEffect(() => {
    const unlisten = listen<AIChatSummarizedEvent>("ai-chat-summarized", (event) => {
      const payload = event.payload;
      if (payload.conversation_id !== activeConvId) return;
      showToast("info", payload.message || "对话历史已自动总结压缩");
      // 重新加载会话列表以获取最新的 summary 字段
      void loadConversations();
    });
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [activeConvId, showToast, loadConversations]);

  // 自动滚动到底部
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingContent, streamingThinking]);

  // 确保有活跃会话（首次发送时自动创建）
  async function ensureConversation(): Promise<string | null> {
    if (activeConvId) return activeConvId;
    try {
      const conv = await api.createAiConversation(paperId, "新对话");
      setConversations((c) => [conv, ...c]);
      setActiveConvId(conv.id);
      return conv.id;
    } catch (e) {
      showToast("error", `创建会话失败: ${(e as Error).message}`);
      return null;
    }
  }

  async function handleSend() {
    const text = input.trim();
    if (!text || loading) return;
    const convId = await ensureConversation();
    if (!convId) return;
    setInput("");
    setLoading(true);
    setStreamingContent("");
    setStreamingThinking("");
    try {
      await api.sendAiMessage(convId, text);
    } catch (e) {
      showToast("error", `发送失败: ${(e as Error).message}`);
    } finally {
      setLoading(false);
    }
  }

  async function handlePreset(action: PresetAction) {
    if (loading) return;
    const convId = await ensureConversation();
    if (!convId) return;
    setLoading(true);
    setStreamingContent("");
    setStreamingThinking("");
    try {
      const aiMsg = await api.runAiPresetInChat(convId, action.presetId);
      // 如果 preset 需要写入笔记 AI 区块，从返回内容中提取并写入
      if (action.writesAiBlock) {
        try {
          // 确保 note 存在：先查 paper detail 看 note_path
          let detail = await api.getPaper(paperId);
          if (!detail.note_path) {
            await api.createNote(paperId);
            // createNote 后 DB 已更新 note_path，重新获取以确保后续操作能读到
            detail = await api.getPaper(paperId);
          }
          const content = extractBlockContent(aiMsg.content);
          if (content) {
            await api.updateNoteAiBlock(paperId, action.writesAiBlock, content);
            showToast("success", `${action.label} 已写入笔记`);
          } else {
            showToast("success", `${action.label} 完成`);
          }
        } catch (e) {
          showToast("warning", `${action.label} 完成，但写入笔记失败: ${(e as Error).message}`);
        }
      } else {
        showToast("success", `${action.label} 完成`);
      }
    } catch (e) {
      showToast("error", `${action.label} 失败: ${(e as Error).message}`);
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

  async function handleNewConversation() {
    try {
      const conv = await api.createAiConversation(paperId, "新对话");
      setConversations((c) => [conv, ...c]);
      setActiveConvId(conv.id);
      setShowConvList(false);
    } catch (e) {
      showToast("error", `创建会话失败: ${(e as Error).message}`);
    }
  }

  // 手动触发对话历史总结：强制 LLM 对所有历史（除最近 N 条）做总结并缓存
  async function handleSummarize() {
    if (!activeConvId || summarizing || loading) return;
    setSummarizing(true);
    try {
      const updated = await api.summarizeAiConversation(activeConvId);
      setConversations((cs) =>
        cs.map((c) => (c.id === updated.id ? updated : c)),
      );
      if (updated.summary) {
        showToast(
          "success",
          `已生成摘要：覆盖 ${updated.summary_chars} 字历史消息`,
        );
      } else {
        showToast("info", "对话历史较短，暂无需总结");
      }
    } catch (e) {
      showToast("error", `总结失败: ${(e as Error).message}`);
    } finally {
      setSummarizing(false);
    }
  }

  async function handleDeleteConversation(id: string) {
    try {
      await api.deleteAiConversation(id);
      const next = conversations.filter((c) => c.id !== id);
      setConversations(next);
      if (activeConvId === id) {
        setActiveConvId(next.length > 0 ? next[0].id : null);
      }
    } catch (e) {
      showToast("error", `删除失败: ${(e as Error).message}`);
    }
  }

  function startRename(conv: AIConversation) {
    setRenamingId(conv.id);
    setRenameValue(conv.title);
  }

  async function commitRename() {
    if (!renamingId) return;
    const title = renameValue.trim() || "新对话";
    try {
      const updated = await api.renameAiConversation(renamingId, title);
      setConversations((cs) =>
        cs.map((c) => (c.id === updated.id ? updated : c)),
      );
    } catch (e) {
      showToast("error", `重命名失败: ${(e as Error).message}`);
    } finally {
      setRenamingId(null);
      setRenameValue("");
    }
  }

  const activeConv = conversations.find((c) => c.id === activeConvId);
  const hasStreaming = streamingContent.length > 0 || streamingThinking.length > 0;

  return (
    <div className="relative flex h-full flex-col bg-background">
      {/* 顶部：标题 + 会话切换 */}
      <div className="flex h-9 shrink-0 items-center justify-between border-b border-border px-2">
        <div className="flex items-center gap-1.5 text-xs font-medium">
          <Sparkles className="h-3.5 w-3.5 text-primary" />
          AI 对话
        </div>
        <div className="flex items-center gap-1">
          <button
            onClick={() => setShowConvList((s) => !s)}
            className="flex items-center gap-1 rounded px-1.5 py-0.5 text-xs text-muted-foreground hover:bg-muted hover:text-foreground"
            title="切换会话"
          >
            <span className="max-w-[120px] truncate">
              {activeConv?.title ?? "未选择"}
            </span>
            {activeConv?.summary && (
              <span
                className="ml-0.5 inline-block h-1.5 w-1.5 rounded-full bg-emerald-500"
                title={`已有摘要：覆盖 ${activeConv.summary_chars} 字历史`}
              />
            )}
            <ChevronDown className="h-3 w-3" />
          </button>
          <Button
            size="icon"
            variant="ghost"
            className="h-6 w-6"
            onClick={() => void handleSummarize()}
            disabled={!activeConvId || summarizing || loading}
            title="总结当前对话历史（压缩旧消息以节省上下文）"
          >
            {summarizing ? (
              <Loader2 className="h-3.5 w-3.5 animate-spin" />
            ) : (
              <ScrollText className="h-3.5 w-3.5" />
            )}
          </Button>
          <Button
            size="icon"
            variant="ghost"
            className="h-6 w-6"
            onClick={handleNewConversation}
            title="新建对话"
          >
            <Plus className="h-3.5 w-3.5" />
          </Button>
          {onClose && (
            <Button
              size="icon"
              variant="ghost"
              className="h-6 w-6"
              onClick={onClose}
              title="收起 AI 对话"
            >
              <PanelLeftClose className="h-3.5 w-3.5" />
            </Button>
          )}
        </div>
      </div>

      {/* 会话列表下拉 */}
      {showConvList && (
        <div className="absolute z-50 mt-9 w-[260px] rounded-md border border-border bg-popover p-1 shadow-md">
          <div className="max-h-[280px] overflow-y-auto">
            {conversations.length === 0 ? (
              <div className="p-2 text-center text-xs text-muted-foreground">
                暂无会话
              </div>
            ) : (
              conversations.map((conv) => (
                <div
                  key={conv.id}
                  className={cn(
                    "group flex items-center gap-1 rounded px-1.5 py-1 text-xs",
                    conv.id === activeConvId
                      ? "bg-accent text-accent-foreground"
                      : "hover:bg-muted",
                  )}
                >
                  {renamingId === conv.id ? (
                    <Input
                      value={renameValue}
                      onChange={(e) => setRenameValue(e.target.value)}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") void commitRename();
                        if (e.key === "Escape") {
                          setRenamingId(null);
                          setRenameValue("");
                        }
                      }}
                      className="h-6 flex-1 text-xs"
                      autoFocus
                    />
                  ) : (
                    <button
                      className="flex flex-1 items-center gap-1.5 truncate text-left"
                      onClick={() => {
                        setActiveConvId(conv.id);
                        setShowConvList(false);
                      }}
                    >
                      <MessageSquare className="h-3 w-3 shrink-0 opacity-60" />
                      <span className="truncate">{conv.title}</span>
                    </button>
                  )}
                  {renamingId === conv.id ? (
                    <>
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-5 w-5"
                        onClick={() => void commitRename()}
                        title="确认"
                      >
                        <Check className="h-3 w-3" />
                      </Button>
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-5 w-5"
                        onClick={() => {
                          setRenamingId(null);
                          setRenameValue("");
                        }}
                        title="取消"
                      >
                        <X className="h-3 w-3" />
                      </Button>
                    </>
                  ) : (
                    <div className="hidden gap-0.5 group-hover:flex">
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-5 w-5"
                        onClick={() => startRename(conv)}
                        title="重命名"
                      >
                        <Pencil className="h-3 w-3" />
                      </Button>
                      <Button
                        size="icon"
                        variant="ghost"
                        className="h-5 w-5"
                        onClick={() => void handleDeleteConversation(conv.id)}
                        title="删除"
                      >
                        <Trash2 className="h-3 w-3" />
                      </Button>
                    </div>
                  )}
                </div>
              ))
            )}
          </div>
          <div className="mt-1 border-t border-border pt-1">
            <Button
              size="sm"
              variant="ghost"
              className="h-7 w-full justify-start text-xs"
              onClick={handleNewConversation}
            >
              <Plus className="mr-1.5 h-3 w-3" />
              新建对话
            </Button>
          </div>
        </div>
      )}

      {/* 消息流 */}
      <div className="flex-1 overflow-y-auto px-2 py-2">
        {messages.length === 0 && !hasStreaming && (
          <div className="flex h-full flex-col items-center justify-center text-center text-xs text-muted-foreground">
            <MessageSquare className="mx-auto mb-2 h-8 w-8 opacity-30" />
            <p>向 AI 提问关于这篇论文的任何问题</p>
            <p className="mt-1 opacity-70">Enter 发送 / Shift+Enter 换行</p>
            <p className="mt-2 opacity-70">或使用下方快捷功能</p>
          </div>
        )}
        {messages.map((msg) => (
          <MessageBubble
            key={msg.id}
            msg={msg}
            presetName={msg.preset_id ? presetNameMap.get(msg.preset_id) : undefined}
          />
        ))}
        {/* 流式占位消息 */}
        {hasStreaming && (
          <StreamingBubble
            content={streamingContent}
            thinking={streamingThinking}
          />
        )}
        {loading && !hasStreaming && (
          <div className="flex justify-start py-2">
            <div className="flex items-center gap-1.5 rounded-md bg-muted px-3 py-1.5 text-xs text-muted-foreground">
              <Loader2 className="h-3 w-3 animate-spin" />
              AI 思考中…
            </div>
          </div>
        )}
        <div ref={messagesEndRef} />
      </div>

      {/* 快捷功能入口 */}
      <div className="shrink-0 border-t border-border px-2 py-1.5">
        <div className="mb-1 flex items-center gap-1 text-[10px] uppercase tracking-wide text-muted-foreground">
          <Sparkles className="h-2.5 w-2.5" />
          快捷功能
        </div>
        <div className="grid grid-cols-3 gap-1">
          {PRESET_ACTIONS.map((action) => {
            const Icon = action.icon;
            return (
              <Button
                key={action.presetId}
                size="sm"
                variant="outline"
                disabled={loading}
                onClick={() => void handlePreset(action)}
                className="h-7 flex-col gap-0 px-1 text-[10px]"
                title={action.description}
              >
                <Icon className="h-3 w-3" />
                <span className="leading-none">{action.label}</span>
              </Button>
            );
          })}
        </div>
      </div>

      {/* 输入框 */}
      <div className="shrink-0 border-t border-border p-2">
        <Textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="提问关于这篇论文的问题…"
          className="min-h-[60px] resize-none text-xs"
          disabled={loading}
        />
        <div className="mt-1 flex items-center justify-between">
          <span className="text-[10px] text-muted-foreground">
            {activeConv
              ? activeConv.summary
                ? `会话：${activeConv.title} · 已总结 ${activeConv.summary_chars} 字`
                : `会话：${activeConv.title}`
              : "将自动创建新会话"}
          </span>
          <Button
            size="sm"
            onClick={() => void handleSend()}
            disabled={loading || !input.trim()}
          >
            <Send className="mr-1 h-3 w-3" />
            发送
          </Button>
        </div>
      </div>
    </div>
  );
}

// ============================================================
// 消息气泡组件
// ============================================================

interface MessageBubbleProps {
  msg: AIMessage;
  presetName?: string;
}

function MessageBubble({ msg, presetName }: MessageBubbleProps) {
  const isUser = msg.role === "user";
  return (
    <div
      className={cn(
        "flex py-1.5",
        isUser ? "justify-end" : "justify-start",
      )}
    >
      <div
        className={cn(
          "max-w-[92%] rounded-md px-2.5 py-1.5 text-xs",
          isUser
            ? "bg-primary text-primary-foreground"
            : "bg-muted text-foreground",
        )}
      >
        {/* preset 标记 */}
        {presetName && (
          <div className="mb-1">
            <Badge variant="secondary" className="h-4 px-1 text-[10px]">
              <Sparkles className="mr-0.5 h-2 w-2" />
              {presetName}
            </Badge>
          </div>
        )}
        {/* 上下文 */}
        {msg.context && (
          <CollapsibleBlock
            title="上下文"
            icon={<FileText className="h-2.5 w-2.5" />}
            content={msg.context}
            defaultOpen={false}
          />
        )}
        {/* 思考过程 */}
        {msg.thinking && (
          <CollapsibleBlock
            title="思考过程"
            icon={<Brain className="h-2.5 w-2.5" />}
            content={msg.thinking}
            defaultOpen={false}
          />
        )}
        {/* 消息正文 */}
        <div className="whitespace-pre-wrap break-words">{msg.content}</div>
      </div>
    </div>
  );
}

interface StreamingBubbleProps {
  content: string;
  thinking: string;
}

function StreamingBubble({ content, thinking }: StreamingBubbleProps) {
  return (
    <div className="flex justify-start py-1.5">
      <div className="max-w-[92%] rounded-md bg-muted px-2.5 py-1.5 text-xs">
        {thinking && (
          <CollapsibleBlock
            title="思考过程"
            icon={<Brain className="h-2.5 w-2.5" />}
            content={thinking}
            defaultOpen={false}
          />
        )}
        {content ? (
          <div className="whitespace-pre-wrap break-words">
            {content}
            <span className="ml-0.5 inline-block h-3 w-1 animate-pulse bg-foreground/60" />
          </div>
        ) : (
          <div className="flex items-center gap-1.5 text-muted-foreground">
            <Loader2 className="h-3 w-3 animate-spin" />
            <span>AI 思考中…</span>
          </div>
        )}
      </div>
    </div>
  );
}

// ============================================================
// 可折叠块（思考过程 / 上下文）
// ============================================================

interface CollapsibleBlockProps {
  title: string;
  icon: React.ReactNode;
  content: string;
  defaultOpen?: boolean;
}

function CollapsibleBlock({
  title,
  icon,
  content,
  defaultOpen = false,
}: CollapsibleBlockProps) {
  const [open, setOpen] = useState(defaultOpen);
  const preview = content.slice(0, 80).replace(/\n/g, " ");
  return (
    <div className="mb-1 rounded border border-border/50 bg-background/30">
      <button
        onClick={() => setOpen((o) => !o)}
        className="flex w-full items-center gap-1 px-1.5 py-1 text-[10px] text-muted-foreground hover:text-foreground"
      >
        {open ? (
          <ChevronDown className="h-2.5 w-2.5" />
        ) : (
          <ChevronRight className="h-2.5 w-2.5" />
        )}
        {icon}
        <span className="font-medium">{title}</span>
        {!open && preview && (
          <span className="ml-1 truncate opacity-60">{preview}…</span>
        )}
      </button>
      {open && (
        <div className="border-t border-border/50 px-1.5 py-1 text-[10px] whitespace-pre-wrap break-words text-muted-foreground">
          {content}
        </div>
      )}
    </div>
  );
}

// ============================================================
// 工具函数：从 AI 返回内容中提取要写入笔记 AI 区块的文本
// ============================================================

function extractBlockContent(content: string): string {
  if (!content.trim()) return "";
  // 如果是纯文本/Markdown，直接返回
  return content;
}
