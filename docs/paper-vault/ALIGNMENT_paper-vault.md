# ALIGNMENT — PaperVault（本地论文阅读与 Markdown 笔记应用）

> 任务代号：`paper-vault`
> 工作目录：`f:/learn/study/ai/trea`
> 当前状态：空目录（绿地项目）
> 文档版本：v1.0 — 2026-06-14

## 一、原始需求摘要

构建一个**本地桌面端论文阅读工具**，参考 Zotero 的文献管理 + Obsidian 的本地 Markdown 笔记。

### 五条核心原则
1. **PDF 是原始证据** — 不修改原 PDF，路径 + 副本归档。
2. **元数据是检索索引** — 存 SQLite，可重建。
3. **Markdown 是长期笔记资产** — 事实源，可独立打开。
4. **AI 是辅助整理工具** — 用户确认后写入。
5. **Skill 是 AI 功能的预设工作流** — 用户可改提示词。

### 推荐技术栈（来自用户）
- 桌面端：Tauri + React + TypeScript
- 本地数据库：SQLite（建议 FTS5）
- 文件存储：本地 `PaperVault/` 目录
- AI 接口：OpenAI 兼容 API（可配 base_url、api_key、model）

### 信息架构（v1）
- 首页：Zotero 式三栏（目录 / 论文列表 / 详情）
- 阅读工作台：PDF + Markdown 并排
- v2 再加双向链接 / 图谱 / Canvas

### 核心功能模块
1. 文献导入（PDF 单/批 + 未识别兜底）
2. PDF 阅读器（页码、缩放、搜索、阅读进度）
3. Markdown 笔记（用户主笔记 + AI 区块分隔 + 模板）
4. 文献组织（集合 / 标签 / 关键词 / 多视图）
5. 阅读进度（页码 + 状态 + 百分比）
6. 全文搜索（SQLite FTS5 + 权重排序）
7. 重复检测（DOI > 标题归一化 > 作者+年份）
8. 引用导出（BibTeX / Markdown citation / RIS v1.5）
9. 备份迁移（打开库目录 + 导出 db 备份）
10. AI 与 Skill 预设（内置 7 个预设 + 用户可改）

### 分阶段交付
- **v1 最小可用版**（本次目标）
- v1.5（智能集合 / 整体笔记 / 高亮 / RIS / 运行历史）
- v2（双向链接 / 图谱 / Canvas / Skill marketplace）

## 二、任务范围边界确认

### ✅ 包含在 v1
- Tauri + React + TS 项目脚手架
- `PaperVault/` 库目录结构与文件管理
- SQLite + FTS5 全文搜索
- PDF 导入、识别、归档、去重
- PDF 阅读器与阅读进度
- Markdown 笔记 CRUD、模板、AI 区块
- 集合、标签、关键词、目录视图
- BibTeX 导出
- OpenAI 兼容 API 配置
- 7 个内置 AI skill 预设
- 用户可编辑提示词入口
- 元数据提取、摘要翻译、论文总结、自动建笔记

### ❌ 不在 v1（明确排除）
- 智能集合 → v1.5
- 多论文综述自动生成 → v1.5
- PDF 高亮批注 → v1.5
- Related Papers → v1.5
- RIS 导出 → v1.5
- AI 运行历史 → v1.5
- 双向链接 / 反向链接 → v2
- 知识图谱 / Canvas → v2
- Obsidian vault 兼容 → v2
- 自动云同步 / 增量备份 → 不做

## 三、对现有项目的理解

工作目录 `f:/learn/study/ai/trea` 为**全新空目录**，无任何现有代码、文档、配置。因此：

- 不用顾虑既有代码风格或架构冲突
- 项目名称、目录结构、依赖选型完全从规范出发
- `docs/` 目录由本次任务创建并使用

## 四、识别的歧义点与需要澄清的决策

### 决策 A：实施起点与节奏 ⭐ 高优先级
v1 范围仍较大（10+ 子模块）。可选执行节奏：
- **A1**：先做完整项目脚手架（目录、依赖、基础 IPC、占位 UI）再逐模块填充 — 便于您早期查看整体结构
- **A2**：从核心闭环开始（PDF 导入 + 列表 + 详情）跑通最小可演示版本，再扩展笔记、搜索、AI — 便于您更早看到能用的功能
- **A3**：一次性实现 v1 全量再交付 — 周期长、中间无反馈

### 决策 B：前端 UI / 样式库 ⭐ 高优先级
需要在以下方案中确认：
- **B1（推荐）**：shadcn/ui + Tailwind CSS + Radix — 现代、轻量、组件可复制易改
- **B2**：Mantine — 组件齐全、TS 友好、风格统一
- **B3**：Ant Design — 表格表单强、风格偏传统
- **B4**：纯 Tailwind + 自写组件 — 最灵活但工作量大

### 决策 C：Markdown 编辑器 ⭐ 中优先级
- **C1（推荐）**：CodeMirror 6 — 轻量、TS 友好、扩展性强
- **C2**：Monaco Editor — VS Code 同款，重
- **C3**：TipTap — 富文本编辑体验

### 决策 D：PDF 渲染方案 ⭐ 中优先级
- **D1（推荐）**：pdfjs-dist（纯 JS，跨平台，Tauri 友好）
- **D2**：react-pdf（基于 pdfjs 的 React 封装）
- **D3**：Tauri 原生 webview 直接渲染 PDF

### 决策 E：Tauri 集成层 ⭐ 中优先级
- **E1（推荐）**：tauri-plugin-sql（rusqlite 内置，支持 SQLite + 迁移）
- **E2**：自写 Rust 后端命令 + rusqlite
- **E3**：tauri-plugin-sql + 自写命令混合

### 决策 F：测试策略 ⭐ 低优先级
- **F1（推荐）**：Vitest（前端）+ Rust 内置 cargo test（后端）
- **F2**：Vitest + Playwright（端到端）
- **F3**：仅 Rust 测试，前端不写单测

### 决策 G：是否需要演示模式 ⭐ 低优先级
- **G1**：提供一组示例 PDF / 示例数据，启动后可演示
- **G2**：纯净启动，用户自行导入

### 决策 H：AI 默认配置 ⭐ 低优先级
- 默认 base_url、api_key 是否留空由用户填？
- 是否提供 mock LLM（返回固定 JSON）用于本地无 key 测试？

---

## 五、待用户澄清的问题清单（优先级降序）

1. **实施节奏**：A1 / A2 / A3 选哪个？
2. **前端 UI 库**：B1 / B2 / B3 / B4 选哪个？
3. **Markdown 编辑器**：C1 / C2 / C3 选哪个？
4. **PDF 渲染方案**：D1 / D2 / D3 选哪个？
5. **Tauri 集成层**：E1 / E2 / E3 选哪个？
6. **测试策略**：F1 / F2 / F3 选哪个？
7. **演示数据**：G1 / G2 选哪个？
8. **AI 默认配置**：留空用户填 / 提供 mock / 其他？

> 建议先回答前 5 个关键决策，后 3 个可给合理默认。
