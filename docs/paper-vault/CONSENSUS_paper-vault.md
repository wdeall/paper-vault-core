# CONSENSUS — PaperVault（本地论文阅读与 Markdown 笔记应用）

> 任务代号：`paper-vault`
> 工作目录：`f:/learn/study/ai/trea`
> 版本：v1.0 — 2026-06-14
> 上游文档：[ALIGNMENT_paper-vault.md](./ALIGNMENT_paper-vault.md)

## 一、明确的需求描述

构建一个**本地桌面端**论文阅读与 Markdown 笔记应用，参考 Zotero 的文献管理 + Obsidian 的本地 Markdown 笔记方式。

### v1 必须实现的能力
1. 集中库目录 `PaperVault/` 管理所有资产
2. PDF 单/批导入、自动归档、未识别兜底
3. SQLite + FTS5 全文索引与搜索
4. Zotero 式三栏首页（目录 / 列表 / 详情）
5. PDF + Markdown 并排阅读工作台
6. 阅读进度保存与恢复
7. 集合 / 标签 / 关键词 / 多视图
8. DOI 优先 + 标题归一化的重复检测
9. BibTeX / Markdown 引用导出
10. 7 个内置 AI skill 预设 + 用户可改提示词
11. 元数据提取、摘要翻译、论文总结、自动建笔记
12. OpenAI 兼容 API 配置（base_url / api_key / model）

### v1 明确不做
- 智能集合 → v1.5
- 多论文综述自动生成 → v1.5
- PDF 高亮批注 → v1.5
- Related Papers → v1.5
- RIS 导出 → v1.5
- AI 运行历史 → v1.5
- 双向链接 / 图谱 / Canvas → v2
- 自动云同步 / 增量备份 → 不做

## 二、技术实现方案

### 技术栈（已锁定）
| 层 | 选型 | 备注 |
|---|---|---|
| 桌面壳 | Tauri 2.x | Rust + 系统 Webview |
| 前端 | React 18 + TypeScript | Vite 5 |
| UI 库 | shadcn/ui + Tailwind CSS 3 + Radix | 组件源码在本地 |
| 状态管理 | Zustand | 轻量、TS 友好 |
| 路由 | React Router 6 | |
| Markdown 编辑器 | CodeMirror 6 | @codemirror/lang-markdown + 主题 |
| PDF 渲染 | pdfjs-dist | 纯 JS，文本层可搜索 |
| 本地数据库 | SQLite + FTS5 | 通过 tauri-plugin-sql 访问 |
| 后端逻辑 | 自写 Tauri Rust 命令 | 文件、AI、网络请求 |
| AI 客户端 | reqwest + serde_json | OpenAI 兼容协议 |
| 测试 | Vitest（前端）+ cargo test（Rust） | |
| 包管理 | pnpm（前端）+ cargo（Rust） | |
| Lint/Format | ESLint + Prettier（前端）+ clippy + rustfmt（Rust） | |

### 目录结构（脚手架）
```
f:/learn/study/ai/trea/
├── docs/paper-vault/            # 本次工作文档
│   ├── ALIGNMENT_paper-vault.md
│   ├── CONSENSUS_paper-vault.md
│   ├── DESIGN_paper-vault.md
│   ├── TASK_paper-vault.md
│   ├── ACCEPTANCE_paper-vault.md
│   ├── FINAL_paper-vault.md
│   └── TODO_paper-vault.md
├── src/                          # React 前端
│   ├── main.tsx
│   ├── App.tsx
│   ├── routes/
│   ├── components/
│   │   ├── ui/                   # shadcn 组件
│   │   ├── library/              # 库视图组件
│   │   ├── reader/               # 阅读工作台
│   │   └── notes/                # Markdown 编辑器包装
│   ├── stores/                   # Zustand stores
│   ├── hooks/
│   ├── lib/
│   │   ├── api.ts                # Tauri invoke 封装
│   │   ├── format.ts
│   │   └── bibtex.ts
│   ├── types/                    # 与 Rust 共享的类型
│   └── styles/
├── src-tauri/                    # Tauri 后端
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── src/
│   │   ├── main.rs
│   │   ├── lib.rs
│   │   ├── commands/             # IPC 命令
│   │   │   ├── papers.rs
│   │   │   ├── notes.rs
│   │   │   ├── search.rs
│   │   │   ├── ai.rs
│   │   │   ├── export.rs
│   │   │   ├── settings.rs
│   │   │   └── indexer.rs
│   │   ├── db/                   # SQLite + 迁移
│   │   │   ├── mod.rs
│   │   │   ├── migrations/
│   │   │   └── fts.rs
│   │   ├── pdf/                  # 元数据提取、文本提取
│   │   ├── markdown/             # 笔记读写、frontmatter 解析
│   │   ├── ai/                   # OpenAI 客户端 + preset 引擎
│   │   ├── vault/                # PaperVault 目录管理
│   │   ├── duplicates/           # 重复检测
│   │   ├── export/               # BibTeX / 引用
│   │   └── seed/                 # 示例数据
│   └── icons/
├── tests/                        # 前端测试
├── package.json
├── pnpm-lock.yaml
├── tsconfig.json
├── tailwind.config.js
├── vite.config.ts
├── .gitignore
├── .env.example
└── README.md
```

### PaperVault 库目录结构（运行时创建）
```
PaperVault/                       # 用户在设置页选择或首次启动时创建
├── papers.db                     # SQLite
├── pdfs/
│   └── YYYY/paper-id-title.pdf
├── notes/
│   ├── papers/paper-id-title.md
│   └── topics/topic-note.md
├── attachments/
├── exports/
└── backups/papers-YYYYMMDD-HHMMSS.db
```

### 数据模型（v1 最小可用）
所有表通过 tauri-plugin-sql 管理，建表 SQL 在 `src-tauri/src/db/migrations/0001_init.sql`。

```sql
-- 论文主表
CREATE TABLE papers (
  id            TEXT PRIMARY KEY,    -- uuid
  title         TEXT NOT NULL DEFAULT '',
  authors       TEXT NOT NULL DEFAULT '[]',  -- JSON 数组
  year          INTEGER,
  venue         TEXT,
  doi           TEXT,
  abstract      TEXT,
  keywords      TEXT NOT NULL DEFAULT '[]',  -- JSON 数组
  tags          TEXT NOT NULL DEFAULT '[]',  -- JSON 数组
  status        TEXT NOT NULL DEFAULT '未读', -- 未读/阅读中/已读/重点重读
  rating        INTEGER,
  pdf_path      TEXT,
  note_path     TEXT,
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);

-- 阅读进度
CREATE TABLE reading_progress (
  paper_id         TEXT PRIMARY KEY REFERENCES papers(id) ON DELETE CASCADE,
  current_page     INTEGER NOT NULL DEFAULT 0,
  total_pages      INTEGER NOT NULL DEFAULT 0,
  progress_percent REAL NOT NULL DEFAULT 0,
  last_read_at     INTEGER
);

-- 集合
CREATE TABLE collections (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  parent_id   TEXT,
  created_at  INTEGER NOT NULL
);
CREATE TABLE paper_collections (
  paper_id      TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
  collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  PRIMARY KEY (paper_id, collection_id)
);

-- 全文索引（FTS5 虚拟表）
CREATE VIRTUAL TABLE fulltext_index USING fts5(
  paper_id UNINDEXED,
  source_type UNINDEXED,   -- title/authors/abstract/keywords/notes/pdf
  content,
  page UNINDEXED,
  tokenize = 'unicode61 remove_diacritics 2'
);

-- 索引状态
CREATE TABLE index_status (
  paper_id    TEXT PRIMARY KEY REFERENCES papers(id) ON DELETE CASCADE,
  status      TEXT NOT NULL DEFAULT '未索引',  -- 未索引/索引中/已索引/索引失败
  error       TEXT,
  indexed_at  INTEGER
);

-- AI skill 预设
CREATE TABLE ai_skill_presets (
  id            TEXT PRIMARY KEY,        -- 固定为内置 id 或 uuid（用户自定义）
  name          TEXT NOT NULL,
  bound_action  TEXT NOT NULL,           -- 绑定功能
  skill         TEXT NOT NULL,           -- pdf / research-lookup / literature-review
  system_prompt TEXT NOT NULL,
  user_template TEXT NOT NULL,
  output_format TEXT NOT NULL DEFAULT 'json',
  auto_write    INTEGER NOT NULL DEFAULT 0,
  is_builtin    INTEGER NOT NULL DEFAULT 0,
  updated_at    INTEGER NOT NULL
);

-- AI 提供方配置
CREATE TABLE ai_provider_config (
  id          TEXT PRIMARY KEY,    -- 固定 'default'
  base_url    TEXT NOT NULL DEFAULT '',
  api_key     TEXT NOT NULL DEFAULT '',
  model       TEXT NOT NULL DEFAULT '',
  updated_at  INTEGER NOT NULL
);
```

## 三、关键约束与约定

### 1. 数据来源原则
- **PDF 文件**：原始证据，不修改，归档到 `pdfs/YYYY/`
- **元数据**：存 SQLite，重建索引成本低
- **Markdown**：事实源，AI 内容写固定区块，用户手写不覆盖
- **阅读进度 / 全文索引 / AI preset / 重复检测结果**：存 SQLite
- **API key**：存 `ai_provider_config`（不写入 .env 文件、不入 git）

### 2. AI 写入策略
- 元数据提取 → 生成候选 → 用户确认 → 写库
- 总结 / 翻译 / 笔记 → 写入 Markdown AI 区块，不覆盖手写区
- 用户修改提示词 → 存为 `ai_skill_presets` 新 id（`is_builtin=0`）
- 恢复默认 → 删除该 `bound_action` 的自定义版本

### 3. 文件命名规则
- 论文 id：uuid v4（小写，无连字符）
- PDF 命名：`{paper_id}-{title-slug}.pdf`，slug 由标题转 ascii-lowercase-短横线，去停用词
- Markdown 命名：与 PDF 同前缀
- 备份命名：`papers-YYYYMMDD-HHMMSS.db`

### 4. Markdown 模板（固定）
```markdown
---
id: <paper_id>
title: ""
authors: []
year: ""
venue: ""
doi: ""
keywords: []
tags: []
status: 未读
rating:
pdf_path: ""
---

# <title>

## 基本信息

## AI 摘要
<!-- AI_GENERATED_START:summary -->
<!-- AI_GENERATED_END:summary -->

## 论文要点
<!-- AI_GENERATED_START:key_points -->
<!-- AI_GENERATED_END:key_points -->

## 方法理解

## 实验与结果

## 局限与问题

## 我的笔记

## 相关论文
```

### 5. 安全
- API key 仅存在本地数据库，不上传、不入 git
- 导入 PDF 时做大小限制（默认 200MB）和扩展名白名单
- 删除论文时默认不删文件，强确认后才删

## 四、任务边界（v1 严格范围）

### 范围内
✅ 上述 v1 必须实现的能力全部 12 项
✅ 7 个内置 AI skill 预设
✅ 示例数据（3-5 篇占位论文，无真实 PDF）
✅ Vitest + cargo test 基础测试
✅ README + .env.example

### 范围外（v1 不做）
❌ 自动云同步 / 多端
❌ Obsidian vault 同步
❌ 增量备份 / 定时备份
❌ 任何 web 远程服务
❌ 账号系统

## 五、验收标准

### 功能验收
1. ✅ 启动应用，能选择或新建 `PaperVault/` 库目录
2. ✅ 单 PDF 和批量 PDF 导入成功，文件归档到 `pdfs/YYYY/`
3. ✅ 导入未识别论文显示"待补全"状态，文件名作为默认标题
4. ✅ 元数据自动提取，AI 候选显示在前端供用户确认
5. ✅ 论文列表显示阅读状态 + 进度百分比
6. ✅ 点击论文打开阅读工作台，PDF + Markdown 并排
7. ✅ 切换论文 / 关闭应用后，下次打开恢复页码
8. ✅ SQLite FTS5 全文搜索命中标题/作者/关键词/摘要/笔记/PDF
9. ✅ 重复导入触发疑似重复提示（DOI/标题归一化）
10. ✅ BibTeX / Markdown 引用导出正确
11. ✅ 设置页配置 OpenAI 兼容 API（base_url / api_key / model）
12. ✅ 7 个内置 AI skill 预设能运行（需用户已配 API key）
13. ✅ 用户修改提示词后 AI 使用修改版本
14. ✅ 恢复默认后提示词回到内置版本
15. ✅ AI 总结只更新 Markdown AI 区块，不覆盖手写区

### 质量验收
- 前端 ESLint / Prettier 无错
- Rust clippy / rustfmt 无错
- `cargo test` 全部通过
- `pnpm test` 全部通过
- `pnpm tauri build` 产出可运行安装包（dev 模式至少能启动）

### 文档验收
- README 含：项目介绍、依赖安装、运行命令、库目录说明
- .env.example 含所有可配置项
- 关键设计决策有注释

## 六、决策记录

| # | 决策项 | 选择 | 理由 |
|---|---|---|---|
| A | 实施节奏 | A1 先做完整脚手架 | 早期看整体结构 |
| B | UI 库 | B1 shadcn/ui + Tailwind | 现代轻量、可改源码 |
| C | Markdown 编辑器 | C1 CodeMirror 6 | 轻量高性能、TS 原生 |
| D | PDF 渲染 | D1 pdfjs-dist | 跨平台、文本层可搜索 |
| E | Tauri 集成 | E1 plugin-sql + 自写命令 | 数据库走插件、复杂逻辑走命令 |
| F | 测试 | F1 Vitest + cargo test | 前后端各自覆盖 |
| G | 示例数据 | G1 提供示例 | 便于演示 UI |
| H | AI 默认配置 | H1 留空 | 用户隐私优先 |

## 七、风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| Tauri 2.x 与 React 18 集成坑 | 中 | 脚手架阶段跑通 hello world 后再扩展 |
| pdfjs-dist 在 Tauri 跨域问题 | 中 | 使用 asset protocol / 本地文件协议 |
| FTS5 中文分词 | 中 | 默认 unicode61 + 大小写不敏感；中文按字符匹配 |
| AI 响应格式不稳定 | 中 | 强 prompt 约束 + 后端 JSON 解析兜底 + 重试一次 |
| Rust 编译速度 | 低 | 增量编译 + 仅在需要时改 Rust |
| 大量 PDF 索引内存压力 | 低 | 分页批量提交 + 后台任务 |

## 八、进入下一阶段

共识已达成，进入 **Architect 阶段**：
- 后续将生成 `DESIGN_paper-vault.md` 系统架构设计
- 然后 `TASK_paper-vault.md` 拆分原子任务
- 进入 Automate 阶段前需要您最终审批
