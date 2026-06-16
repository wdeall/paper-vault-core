> ⚠️ **OBSOLETE — 此文档已被新的对齐 Zotero 方案取代，不再维护**
>
> 本文档描述的 v1.5 "L1 主题综述线" 已废弃，相关代码改动未实施。
> 新的方向：见 [`SPEC_zotero_alignment.md`](./SPEC_zotero_alignment.md)（数据模型重构、标识符优先导入、去重合并、双通道搜索、PDF 批注、标准化导出）。
> 保留此文档仅为历史参考。
>
> ---

# PaperVault v1.5 — 主题综述线（L1）设计规范

> 阶段：Brainstorming → Design
> 上一版：v1（已交付）
> 主线：**L1 主题综述线** —— 跨论文的批量管理 + 主题综述生成
> 设计日期：2026-06-14

---

## 1. 主线目标

让用户能从"逐篇阅读 + 单篇笔记"扩展到"跨论文研究"：

1. 用**智能集合**快速圈出一组论文（按状态、年份、关键词、标签等）
2. 在论文列表里**批量勾选**这组论文
3. 用 **AI 生成主题综述**写入用户指定的主题笔记（覆盖更新固定 AI 区块，不动用户手写区）
4. 主题笔记承载跨论文研究的长期资产，为 v2 双向链接打基础

---

## 2. 范围

### 2.1 v1.5 必交付

| 编号 | 功能 | 备注 |
|---|---|---|
| F1 | 智能集合（一键预设 + 用户自定义） | 7 个内置预设；UI 三层渐进 |
| F2 | 关键词聚合视图（左栏自动） | 前 15 + 显示更多；可"固定"为智能集合 |
| F3 | 标签聚合视图（左栏自动） | 用户实际打了 tag 才显示 |
| F4 | 论文列表批量勾选 + 批量工具条 | 工具条承载综述、导出、加集合三个动作 |
| F5 | 多论文综述（topic review） | 主题一文 + 覆盖更新 AI 区块 |
| F6 | 主题笔记模板与 frontmatter（type:topic + source_papers） | 与单篇笔记互不干扰 |
| F7 | 多值条件 OR/AND 显式选择（任一/全部） | 仅 keywords / tags 两个字段 |

### 2.2 不在 v1.5 范围（保持 v1.5/v2 边界清晰）

- ❌ PDF 高亮 / 批注（→ v1.6 或 L2 主线）
- ❌ AI 流式输出（→ L3）
- ❌ AI 运行历史（→ L3）
- ❌ 双向链接 / 反向链接（→ v2）
- ❌ 知识图谱 / Canvas（→ v2）
- ❌ Related Papers 主动 UI（保留接口，UI 留 v2）
- ❌ RIS 导出（v1 已有 BibTeX + Markdown，RIS 留 v1.6）

### 2.3 v1.5 顺手收口的轻量工程债

仅在与本主线"碰到"时一并处理，不展开：

- HTTP 客户端加 60s timeout（综述请求耗时长，必须做）
- "命中数实时预览" 用 SQL `COUNT(*) FROM papers WHERE …` 实现，需要可复用的 rule → SQL 函数
- 主题笔记选择对话框需要列出 `notes/topics/` 下的现有笔记 → 后端补 `list_topic_notes` IPC

---

## 3. 决策总结（Brainstorming 六题回顾）

| # | 决策 | 选择 |
|---|---|---|
| Q1 | v1.5 主线 | **L1 主题综述线** |
| Q2 | 综述笔记的"一篇一文"还是"主题一文" | **B：主题一文，覆盖更新 AI 区块** |
| Q3 | 多篇论文如何组合上下文 | **B：元数据 + 用户笔记正文（手写区）** |
| Q4 | 智能集合规则表达 | **A：字段下拉 + 简单 AND**（多值字段内部支持 OR/AND） |
| Q5 | 关键词集合 | **A 增强**：左栏自动聚合 + 可固定为智能集合 |
| Q6 | 多值 OR/AND | **用户显式选择 任一(any) / 全部(all)**，不默认猜测 |
| Q7 | 综述生成触发流程 | **A：列表批量勾选 → 工具条 → 对话框 → 写入主题笔记** |

---

## 4. 信息架构

### 4.1 左栏 CollectionsPane（v1.5 最终）

```
┌────────────────────────────┐
│ 智能视图                    │  ← v1
│   📁 全部论文                │
│   📁 最近阅读                │
│   📁 最近修改                │
├────────────────────────────┤
│ 阅读状态                    │  ← v1
│   📥 未读                   │
│   📖 阅读中                 │
│   ✅ 已读                   │
│   ⭐ 重点重读                │
├────────────────────────────┤
│ ⭐ 智能集合                  │  ← v1.5 新增
│   📥 未读论文（内置）        │
│   🔥 重点重读（内置）        │
│   📅 2025 年论文（内置）     │
│   🆕 最近添加（内置）        │
│   📖 最近阅读（内置）        │
│   ✏️ 已读但无笔记（内置）    │
│   ⏳ 待补全（内置）          │
│   ─────────────             │
│   🔬 我的：backdoor 复现中  │
│   ➕ 新建智能集合            │
├────────────────────────────┤
│ 🏷️ 关键词（v1.5 新增，自动）│
│   backdoor (8) 📌            │
│   federated learning (12) 📌 │
│   diffusion model (5) 📌     │
│   ...（前 15 个，按命中降序）│
│   [显示更多]                 │
├────────────────────────────┤
│ 🔖 标签（v1.5 新增，自动；   │
│      仅当存在用户标签时显示） │
│   待补全 (3)                 │
│   重点 (2)                   │
├────────────────────────────┤
│ 集合                        │  ← v1
│   📁 我的项目 / 综述 ...     │
│   ➕ 新建集合                │
└────────────────────────────┘
```

### 4.2 PaperListPane 工具条（v1.5 新增）

无勾选时隐藏；勾选 ≥ 1 时显示固定工具条：

```
┌─────────────────────────────────────────────────────────────┐
│ ☑ 已选 8 篇  [生成综述]  [导出 BibTeX]  [加入集合 ▾]  ✕    │
└─────────────────────────────────────────────────────────────┘
```

### 4.3 SmartCollectionEditor 对话框

见上一段（第六题给出的 ASCII mock），不重复。关键：**字段名中文化、操作符默认值锁定、值类型自适应、命中数实时预览**。

### 4.4 TopicReviewDialog 对话框

```
┌─────────────────────────────────────────┐
│ 生成主题综述                              │
│                                         │
│ 主题笔记：                                │
│  ◯ 选择已有：[federated learning ▾]      │  ← list_topic_notes()
│  ◉ 新建：    [输入主题名               ] │
│                                         │
│ AI 预设：                                 │
│  [topic_literature_review (内置) ▾]     │
│  [编辑提示词…]   ← 单次临时修改           │
│                                         │
│ 输入概览：                                │
│  • 8 篇论文 · 元数据 + 用户笔记          │
│  • 预计 ≈ 6,400 token                    │
│  ⚠ 超过 15 篇会触发警告                   │
│                                         │
│ ☐ 完成后跳转到主题笔记                    │
│                                         │
│            [ 取消 ]   [ 开始生成 ]      │
└─────────────────────────────────────────┘
```

---

## 5. 数据模型（增量）

### 5.1 新表 `smart_collections`

```sql
CREATE TABLE smart_collections (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  rules_json TEXT NOT NULL,
  sort_by TEXT NOT NULL DEFAULT 'updated_at',
  sort_dir TEXT NOT NULL DEFAULT 'desc',
  is_builtin INTEGER NOT NULL DEFAULT 0,
  icon TEXT,
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);

CREATE INDEX idx_smart_collections_builtin ON smart_collections(is_builtin);
```

`rules_json` 反序列化为 `Vec<Rule>`：

```rust
#[derive(Serialize, Deserialize)]
pub struct Rule {
    pub field: String,        // "status" | "year" | "keywords" | ...
    pub op: String,           // "=" | ">=" | "contains" | "between" | "last_n_days" | ...
    pub value: serde_json::Value,  // 单值或数组
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#match: Option<String>,   // "any" | "all"，仅 multi-value
}
```

### 5.2 新表 `topic_notes`

记录主题笔记的元数据（不存正文，正文仍在 `notes/topics/*.md`）。

```sql
CREATE TABLE topic_notes (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  note_path TEXT NOT NULL UNIQUE,
  source_papers_json TEXT NOT NULL,   -- JSON array of paper_id
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL
);
```

> 之所以加表而不是仅靠扫描 `notes/topics/`：① 列出现有笔记很快、② `source_papers` 不必每次都解析 frontmatter、③ 与 v1 单篇 `papers` 表对称。

### 5.3 现有表无 schema 变更

`papers` / `paper_collections` / `index_status` / `fulltext_index` / `ai_skill_presets` 不动。

`ai_provider_config` **新增字段** `timeout_secs INTEGER NOT NULL DEFAULT 60`（详见 12 节风险表与 6 节 P6 阶段）。

### 5.4 字段白名单（智能集合规则）

| field（中文显示） | 操作符 | 值类型 | 多值支持 OR/AND |
|---|---|---|---|
| `status`（状态） | `=`, `!=` | 枚举 | — |
| `year`（年份） | `=`, `>=`, `<=`, `between` | 数字 | — |
| `rating`（评分） | `>=`, `<=` | 0–5 | — |
| `tags`（标签） | `contains`, `not_contains` | string[] | ✅ any/all |
| `keywords`（关键词） | `contains`, `not_contains` | string[] | ✅ any/all |
| `authors`（作者） | `contains` | string | — |
| `title`（标题） | `contains` | string | — |
| `venue`（期刊/会议） | `contains`, `=` | string | — |
| `created_at`（添加时间） | `>=`, `<=`, `last_n_days` | timestamp / int | — |
| `updated_at`（修改时间） | `>=`, `<=`, `last_n_days` | timestamp / int | — |
| `index_status`（索引状态） | `=` | 枚举 | — |
| `has_note`（是否有笔记） | `=` | bool | — |

### 5.5 内置智能集合（首次启动 seed）

```rust
fn builtin_smart_collections(now: i64) -> Vec<SmartCollection> {
    vec![
        sc("builtin:unread", "未读论文", "📥", vec![rule("status", "=", "未读")], now),
        sc("builtin:starred", "重点重读", "🔥", vec![rule("status", "=", "重点重读")], now),
        sc("builtin:y_current","本年论文", "📅",
           vec![rule("year", "=", "{{current_year}}")], now),  // 渲染时替换为系统当前年
        sc("builtin:recent7", "最近添加", "🆕", vec![rule("created_at", "last_n_days", 7)], now),
        sc("builtin:read7",   "最近阅读", "📖", vec![rule("updated_at", "last_n_days", 7)], now),
        sc("builtin:noote",   "已读但无笔记", "✏️",
           vec![rule("status","=","已读"), rule("has_note","=", false)], now),
        sc("builtin:todo",    "待补全", "⏳", vec![rule_arr("tags","contains",vec!["待补全"], "any")], now),
    ]
}
```

---

## 6. 规则 → SQL 翻译（核心算法）

### 6.1 设计原则

- **白名单**：只允许 5.4 表里的 (field, op) 组合，未知组合返回 `BadRequest`
- **参数绑定**：所有 value 走 `?` 参数，不拼字符串
- **零 OR/AND 树**：第 5 题已确定顶层规则纯 AND；多值 OR 仅在单个 rule 内部
- **生成的 SQL 形如**：

```sql
SELECT p.* FROM papers p
LEFT JOIN reading_progress rp ON rp.paper_id = p.id
WHERE 1=1
  AND p.status = ?
  AND p.year >= ?
  AND EXISTS (SELECT 1 FROM json_each(p.keywords) WHERE value IN (?, ?))
  AND (p.note_path != '')
  ...
ORDER BY p.<sort_by> <sort_dir>
```

### 6.2 多值 OR/AND 翻译

`field` 来自 5.4 节白名单，**不直接来自用户输入**——前端只能选下拉里出现的字段名，后端再次校验白名单后才进 SQL，避免注入。

```rust
match (op, match_mode) {
    ("contains", Some("any")) => format!("EXISTS (SELECT 1 FROM json_each(p.{field}) WHERE value IN ({}))", placeholders),
    ("contains", Some("all")) => format!("(SELECT COUNT(DISTINCT value) FROM json_each(p.{field}) WHERE value IN ({})) = {}", placeholders, n),
    ("contains", None)         => format!("EXISTS (SELECT 1 FROM json_each(p.{field}) WHERE value = ?)"),
    ("not_contains", _)        => format!("NOT EXISTS (...)"),
    _ => return Err(BadRequest(format!("不支持的组合: {field} {op}"))),
}
```

### 6.3 命中数预览

UI 每次条件变化（防抖 200ms）调 `count_smart_collection(rules)` IPC，返回单个数字，足够快。

---

## 7. IPC 命令（增量）

```rust
// === 智能集合 ===
list_smart_collections() -> Vec<SmartCollection>
get_smart_collection(id: String) -> SmartCollection
create_smart_collection(name, rules, sort_by, sort_dir, icon) -> SmartCollection
update_smart_collection(id, patch: SmartCollection) -> SmartCollection
delete_smart_collection(id) -> ()  // is_builtin = 1 拒绝
list_papers_by_smart(id: String) -> Vec<Paper>
preview_smart_collection(rules: Vec<Rule>, sort_by, sort_dir) -> Vec<Paper>  // 不持久化
count_smart_collection(rules: Vec<Rule>) -> u64

// === 关键词 / 标签聚合 ===
list_keywords_with_count(limit: Option<u32>) -> Vec<{keyword, count}>
list_tags_with_count(limit: Option<u32>) -> Vec<{tag, count}>
pin_keyword_as_collection(keyword: String, name: Option<String>) -> SmartCollection
// 同一 keyword 已固定 → 返回已有项，不重复创建
// 默认 name = keyword 本身

// === 主题笔记 ===
list_topic_notes() -> Vec<TopicNote>
create_topic_note(title: String, source_papers: Vec<String>) -> TopicNote
delete_topic_note(id: String, also_remove_file: bool) -> ()

// === 综述生成 ===
generate_topic_review(
    paper_ids: Vec<String>,
    note_id: Option<String>,           // None = new
    new_title: Option<String>,
    preset_id: String,
    prompt_overrides: Option<{ system?: String, user?: String }>
) -> TopicReviewResult

// === 批量动作（v1.5 顺手） ===
batch_add_to_collection(paper_ids: Vec<String>, collection_id: String) -> ()
```

返回结构：

```rust
pub struct TopicReviewResult {
    pub note: TopicNote,
    pub raw_markdown: String,
    pub token_estimate: u32,
}

pub struct TopicNote {
    pub id: String,
    pub title: String,
    pub note_path: String,
    pub source_papers: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}
```

---

## 8. 主题笔记模板

### 8.1 文件位置

`PaperVault/notes/topics/<slug>.md`，slug 取自标题（与 v1 单篇笔记同算法）。

### 8.2 模板

```markdown
---
id: topic-<slug>
type: topic
title: <用户输入的主题名>
source_papers: [<paper_id_1>, <paper_id_2>, ...]
created_at: <ts>
updated_at: <ts>
---

# <主题名>

## 我的总结


## AI 综述
<!-- AI_GENERATED_START:review -->
<!-- AI_GENERATED_END:review -->

## 涉及论文
<!-- AI_GENERATED_START:paper_list -->
<!-- AI_GENERATED_END:paper_list -->

## 笔记

```

### 8.3 写入规则

- 首次生成：按模板写入，AI 区块填内容
- 二次生成：
  - 替换 `AI_GENERATED:review` 与 `AI_GENERATED:paper_list` 两个区块
  - frontmatter 更新 `source_papers`、`updated_at`
  - 不动 `id` / `created_at` / `title`
  - 不动用户手写区（"我的总结"、"笔记"等任意 H2）
- **降级策略**：
  - 用户删除了某一个 AI marker（仅 `START` 或仅 `END`）→ 当作"该区块缺失"处理：在文件**末尾**追加完整新区块（`## AI 综述\n<!-- ... -->`），不破坏既有正文
  - 用户两个 AI marker 都删除了 → 同上，末尾追加
  - 用户改了 AI 区块内容但 marker 完整 → 仍按 marker 范围替换，原内容丢失（这是设计意图：AI 区块由 AI 拥有）

---

## 9. AI Preset：`topic_literature_review`（内置）

### 9.1 输入变量

```
{{topic_title}}            主题名
{{paper_count}}            论文数
{{papers_with_notes}}      多篇拼装后的上下文（见下）
```

### 9.2 `papers_with_notes` 拼装

后端 `ai_svc::run_topic_review` 在调用 LLM 前生成：

```text
## [{year}] {title}
Authors: {authors_joined}
Venue: {venue}
DOI: {doi}
Keywords: {keywords_joined}
Abstract: {abstract_text[..800]}
Notes (user-written):
{user_handwritten_section[..2000]}

---
```

- "用户手写区" = Markdown 笔记中**剔除全部 `<!-- AI_GENERATED_START:* -->...END:* -->` 区块**后剩余的正文
- 每篇截断后用 `---` 分割
- 全文超 24000 字符再做整体截断（保留前 N 篇完整，末尾标注"已截断"）

### 9.3 默认 system prompt

```
你是一名严谨的学术综述助手。基于用户提供的 N 篇论文（含元数据与用户笔记），
生成一份结构化的主题综述初稿，覆盖：
1. 研究问题与动机
2. 主要方法路线（按类别归并，注明来源论文）
3. 实验与对比（指出共识与分歧）
4. 局限与开放问题
5. 进一步阅读建议

要求：
- 输出 Markdown，不带 code fence
- 引用论文时使用 [{title}] 简写
- 禁止编造未在输入中出现的论文 / 数据
- 用户笔记中的"我"代表论文阅读者，可以引用其观察
- 控制在 800–1500 字
```

### 9.4 默认 user template

```
主题：{{topic_title}}
论文数：{{paper_count}}

输入：
{{papers_with_notes}}
```

### 9.5 输出处理

- `output_format = "markdown"`
- 后端把整段 markdown 写入 `AI_GENERATED:review` 区块
- 同时由后端生成 `paper_list` 块（不需要 LLM，直接用 `papers` 表数据组成）：

```markdown
- [{year}] {title} — {authors_joined} ({doi})
- ...
```

---

## 10. 前端组件清单（增量）

```
src/components/library/
  ├── PaperListPane.tsx              ← 改：加复选框、批量工具条
  ├── BatchToolbar.tsx               ← 新增
  ├── CollectionsPane.tsx            ← 改：加智能集合 / 关键词 / 标签三段
  ├── SmartCollectionsSection.tsx    ← 新增（集合列表 + 新建按钮）
  ├── KeywordsSection.tsx            ← 新增
  ├── TagsSection.tsx                ← 新增
  ├── PaperDetailPane.tsx            ← 不动

src/components/smart/
  ├── SmartCollectionEditor.tsx      ← 新增（对话框）
  ├── RuleRow.tsx                    ← 新增（一行 field+op+value+match）
  ├── ValueInput.tsx                 ← 新增（按 field 类型自适应）
  └── HitPreview.tsx                 ← 新增（命中数 + 列表）

src/components/topic/
  ├── TopicReviewDialog.tsx          ← 新增
  └── TopicNotePicker.tsx            ← 新增（list_topic_notes + 新建）

src/stores/
  ├── smart.ts                       ← 新增（智能集合 store + activeRules）
  ├── batch.ts                       ← 新增（已选 paper id 集合）
  └── paper.ts                       ← 改：listPapers 支持 activeRules 路径
```

---

## 11. 与 v1 的兼容性

- 既有 `papers` / `notes/papers/` / `paper_collections` 不动
- v1 单篇 AI 工具栏（详情面板的 AIPanel）保留，与综述生成互不影响
- v1 用户的笔记中如已存在 `AI_GENERATED:summary` / `AI_GENERATED:key_points` 区块 → 综述生成在拼装上下文时会**剔除全部 AI 区块**（包括 `summary` / `key_points` / 任何其他 `AI_GENERATED:*`），避免 "AI 总结的总结" 现象
- 现有"按状态筛选" / "按集合筛选" 与新的"智能集合 activeRules" 三选一，互斥（同时只有一个生效，UI 切换时清掉另两个）

---

## 12. 风险与缓解

| 风险 | 缓解 |
|---|---|
| token 超限（用户选 30 篇 + 长笔记） | 前端 ≥ 15 篇警告；后端 24000 字符硬截断；Dialog 显示估算 |
| LLM 编造论文 | system prompt 明确禁止；输出后端不做事实校验（v2 用 research-lookup 校验） |
| 二次生成误覆盖用户内容 | AI 区块严格匹配 START/END marker；找不到 marker 时**追加**而不是覆盖 |
| 智能集合规则注入 | 字段白名单 + 参数绑定 |
| 命中预览 SQL 慢 | 防抖 200ms；无索引时仅做最简实现，v1.6 加复合索引 |
| HTTP 60s timeout 不够 | 默认 60s，UI 提供"60s/120s/180s" 选择，写入 `ai_provider_config.timeout_secs` |
| pdfjs worker / Tauri capabilities 与 v1.5 新 IPC 冲突 | 复用 v1 通道；新增 IPC 走相同 capability：`fs:default` 不动 |

---

## 13. 测试场景（验收清单）

| 场景 | 预期 |
|---|---|
| 新建智能集合 `keywords contains backdoor (any)` | 命中预览即时；保存后左栏出现 |
| 内置 "已读但无笔记" 集合 | 列表只剩状态=已读 且 note_path 为空 的论文 |
| 关键词聚合自动出现并排序 | 前 15 个按命中降序；点击切换列表；📌 后变成持久化智能集合 |
| 列表勾选 8 篇 → "生成综述" → 新建主题笔记 "FL-Backdoor" | 生成 markdown 写入 notes/topics/fl-backdoor.md，AI 区块非空 |
| 同一主题二次生成 | review 区块被替换；用户在"我的总结"写的内容保留 |
| 选 16 篇时显示警告 | Dialog 顶部黄色提示，不阻塞 |
| 单次临时改 system prompt | 本次请求使用新 prompt；preset 表数据不变 |
| 综述请求超时 | 60s 后报错，不卡 UI；能重试 |
| 多值 keywords 选 [a, b] + match=all | SQL 用 COUNT(DISTINCT value) = 2 |
| 同字段单选 | 模式开关隐藏 |

---

## 14. 实施阶段拆分（供 writing-plans 使用）

```
P0  数据模型迁移（migrations/0002_v1_5.sql）
P1  Rust 规则引擎（rule.rs：解析 + 转 SQL + 执行）
P2  smart_collections CRUD + IPC + seed 内置
P3  关键词 / 标签聚合 IPC + 固定为集合
P4  topic_notes 表 + IPC + 模板写入
P5  topic_literature_review preset + ai_svc 扩展（拼装上下文）
P6  HTTP 客户端 60s timeout
P7  前端 batch store + PaperListPane 复选框 + BatchToolbar
P8  前端 smart store + SmartCollectionEditor + RuleRow + ValueInput + HitPreview
P9  前端 CollectionsPane 三段（智能集合 / 关键词 / 标签）
P10 前端 TopicReviewDialog + TopicNotePicker
P11 与 v1 兼容性回归（detail pane / search / 单篇 AI）
P12 ACCEPTANCE_v1.5 + 文档同步
```

依赖：P0 → P1 → P2/P3/P4/P5 并行 → P6（独立）→ P7-P10 并行 → P11 → P12

---

## 15. 退出与升级到 v2 的接口

为后续 v2 做的预留（不实现，仅"不堵"）：

- `topic_notes.source_papers` 已经是 paper_id 数组 → v2 反向链接索引可直接消费
- `smart_collections.rules_json` 用 JSON Value 存 → v2 升级到 AND/OR 树时可加 `"kind":"group"` 节点保持向后兼容
- preset 的 `topic_literature_review` 设计成"单 preset 单功能"，v2 Skill marketplace 落地时直接复用

---

## 文档边界

- 本文档是 v1.5 的**设计**，不是实现计划
- 实现计划由 brainstorming → writing-plans 阶段产出，文件名暂定 `PLAN_v1.5.md`
- 实现验收记录到 `ACCEPTANCE_v1.5.md`（与 v1 ACCEPTANCE 同结构）
