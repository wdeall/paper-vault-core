# PaperVault

> 本地论文阅读与 Markdown 笔记应用
> 组合 **Zotero 式文献管理** + **Obsidian 式 Markdown 笔记**，AI 辅助整理。

## 项目理念

- **PDF 是原始证据**
- **元数据是检索索引**
- **Markdown 是长期笔记资产**
- **AI 是辅助整理工具**
- **Skill 是 AI 功能的预设工作流**，用户可以修改提示词

## 技术栈

- **桌面壳**：Tauri 2.x (Rust)
- **前端**：React 18 + TypeScript 5 + Vite 5
- **UI**：shadcn/ui + Tailwind CSS 3 + Radix
- **状态管理**：Zustand
- **路由**：React Router 6
- **Markdown 编辑器**：CodeMirror 6
- **PDF 渲染**：pdfjs-dist
- **本地数据库**：SQLite + FTS5
- **AI 客户端**：OpenAI 兼容 HTTP API
- **测试**：Vitest（前端） + cargo test（Rust）

## 库目录结构

```text
PaperVault/
  papers.db            # SQLite 数据库（索引 / 元数据 / 进度 / AI 预设）
  pdfs/                # 导入后的 PDF 文件（按年份分目录）
  notes/
    papers/            # 单篇论文 Markdown 笔记
    topics/            # 整体 / 主题笔记
  attachments/         # 额外附件
  exports/             # 导出结果
  backups/             # 数据库备份
```

## 开发

```bash
# 安装前端依赖
pnpm install

# 启动开发模式（Vite + Tauri）
pnpm tauri:dev

# 类型检查
pnpm typecheck

# 单元测试
pnpm test

# 打包发布
pnpm tauri:build
```

## 功能特性（v1）

### 文献库

- Zotero 式三栏：集合 / 论文列表 / 详情
- PDF 单个或批量导入
- 重复检测（DOI → 标题归一化 → 作者+年份）
- 关键词、标签、集合、组织视图
- 示例数据加载（5 篇占位论文）

### 阅读工作台

- 内置 PDF 阅读器（pdfjs-dist）
- 阅读进度自动保存
- Markdown 笔记并排编辑（CodeMirror 6）
- AI 工具栏：元数据提取 / 翻译 / 总结 / 自动建笔记
- AI 写入固定区块，不覆盖用户手写内容

### 搜索

- SQLite FTS5 全文索引
- 标题 / 作者 / DOI / 关键词 / 摘要 / 笔记 / PDF 全文
- 命中片段高亮

### 导出

- BibTeX
- Markdown 引用列表

### 备份与迁移

- 一键打开库目录
- 数据库备份（`backups/papers-YYYYMMDD-HHMMSS.db`）
- 整库可拷贝 / 同步到云盘 / Git / Obsidian

### AI Skill 预设

- 7 个内置预设
- 用户可编辑、复制、恢复默认
- 修改保存在独立字段

## 路线图

- **v1**（当前）：单论文闭环 + 基础文献库
- **v1.5**：智能集合 / RIS 导出 / PDF 高亮 / Related Papers / AI 运行历史
- **v2**：双向链接 / 论文知识图谱 / Canvas 研究地图

## 文档

- [docs/paper-vault/ALIGNMENT_paper-vault.md](docs/paper-vault/ALIGNMENT_paper-vault.md) — 阶段 1 对齐
- [docs/paper-vault/CONSENSUS_paper-vault.md](docs/paper-vault/CONSENSUS_paper-vault.md) — 阶段 2 共识
- [docs/paper-vault/DESIGN_paper-vault.md](docs/paper-vault/DESIGN_paper-vault.md) — 阶段 3 架构
- [docs/paper-vault/TASK_paper-vault.md](docs/paper-vault/TASK_paper-vault.md) — 阶段 4 任务
- [docs/paper-vault/ACCEPTANCE_paper-vault.md](docs/paper-vault/ACCEPTANCE_paper-vault.md) — 阶段 5 验收
- [docs/paper-vault/FINAL_paper-vault.md](docs/paper-vault/FINAL_paper-vault.md) — 阶段 6 总结
- [docs/paper-vault/TODO_paper-vault.md](docs/paper-vault/TODO_paper-vault.md) — 待办与缺失配置

## 许可

仅供学习与个人使用。
