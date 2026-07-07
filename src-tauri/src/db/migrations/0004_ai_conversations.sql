-- 0004_ai_conversations.sql
--
-- AI 对话历史持久化：会话 + 消息。
-- 支持按论文分组的多个独立会话；消息含 thinking/context/tool_calls 字段
-- 用于 agent 风格展示（参考 opencode 等 agent 工具）。

CREATE TABLE IF NOT EXISTS ai_conversations (
    id TEXT PRIMARY KEY,
    paper_id TEXT,
    title TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS ai_messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL,
    role TEXT NOT NULL,
    content TEXT NOT NULL,
    thinking TEXT,
    context TEXT,
    tool_calls TEXT,
    preset_id TEXT,
    created_at INTEGER NOT NULL,
    FOREIGN KEY (conversation_id) REFERENCES ai_conversations(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_ai_messages_conversation
    ON ai_messages(conversation_id, created_at);
CREATE INDEX IF NOT EXISTS idx_ai_conversations_paper
    ON ai_conversations(paper_id, updated_at);
