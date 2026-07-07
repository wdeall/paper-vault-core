# PaperVault

> 本地优先的论文阅读与笔记工作台
> 组合 **Zotero 式文献管理** + **Obsidian 式 Markdown 笔记** + **AI 辅助整理**，全程离线运行，数据完全自有。

## 这是什么

PaperVault 是一款桌面应用，把"读论文 → 整理元数据 → 写笔记 → 检索复用"的全流程整合到一个窗口里。

- PDF 当作**原始证据**保留
- 元数据作为**检索索引**
- Markdown 作为**长期知识资产**
- AI 作为**辅助整理工具**（可改提示词、可关闭、可换模型）

所有数据存在本地一个文件夹里，可以放云盘、可以 Git 同步、可以丢进 Obsidian 一起用。AI 调用走 OpenAI 兼容 HTTP 接口，密钥本地保存，不上传任何论文内容到第三方。

## 核心功能

### 文献库（Zotero 式三栏）

- 集合 / 论文列表 / 详情三栏布局
- PDF 单个或批量导入，按年份自动归档
- 重复检测（DOI → 标题归一化 → 作者+年份三级匹配）
- 关键词、标签、集合、组织视图四种维度分类
- 元数据手动编辑 + AI 一键提取 + Crossref 联网补全

### 阅读工作台

- 内置 PDF 阅读器（基于 pdfjs-dist）
- 阅读进度自动保存，重开继续
- Markdown 笔记并排编辑（CodeMirror 6，支持语法高亮）
- 选中文本右键即可发送给 AI
- AI 写入固定区块（`<!-- AI_GENERATED_START -->` 标记），不覆盖用户手写内容

### AI 辅助（7 个内置 Skill 预设）

| 预设 | 用途 |
|------|------|
| 元数据提取 | 从 PDF 文本识别标题、作者、年份、期刊、DOI、摘要、关键词 |
| 论文总结 | 生成结构化摘要写入笔记 |
| 关键要点 | 提取核心论点 |
| 翻译 | 中英互译 |
| 复现计划 | 列出代码与数据集复现步骤 |
| 自动建笔记 | 一键生成完整笔记骨架 |
| 自由对话 | 带论文上下文的多轮对话 |

- 所有预设的提示词都可以在「设置 → AI」里编辑、复制、恢复默认
- 修改保存在独立字段，升级不会覆盖用户自定义
- AI 对话支持流式输出，历史记录持久化到数据库

### 搜索（SQLite FTS5 全文索引）

- 一次搜：标题 / 作者 / DOI / 关键词 / 摘要 / 笔记 / PDF 全文
- 命中片段高亮，点击直达原文位置
- 索引在后台自动维护，导入即可搜

### 导出

- BibTeX（适配 LaTeX/BibDesk/Zotero）
- Markdown 引用列表
- RIS（适配 EndNote/Mendeley）

### 备份与迁移

- 一键打开库目录
- 数据库快照备份（`backups/papers-YYYYMMDD-HHMMSS.db`）
- 整库即一个文件夹，可拷贝 / 云盘 / Git / Obsidian

## 技术栈

| 层 | 选型 |
|----|------|
| 桌面壳 | Tauri 2.x (Rust) |
| 前端 | React 18 + TypeScript 5 + Vite 5 |
| UI | shadcn/ui + Tailwind CSS 3 + Radix |
| 状态 | Zustand |
| 路由 | React Router 6 |
| Markdown 编辑器 | CodeMirror 6 |
| PDF 渲染 | pdfjs-dist |
| 本地数据库 | SQLite + FTS5（rusqlite bundled） |
| AI 客户端 | OpenAI 兼容 HTTP API（reqwest + rustls） |
| 元数据联网 | Crossref REST API |
| 测试 | Vitest（前端） + cargo test（Rust） |

## 库目录结构

```text
PaperVault/
  papers.db            # SQLite 数据库（索引 / 元数据 / 进度 / AI 预设 / 对话历史）
  pdfs/                # 导入后的 PDF 文件（按年份分目录）
  notes/
    papers/            # 单篇论文 Markdown 笔记
    topics/            # 主题笔记
  attachments/         # 额外附件
  exports/             # 导出结果
  backups/             # 数据库备份
```

## 快速开始

### 直接使用（推荐）

到 [Releases](https://github.com/wdeall/paper-vault-core/releases) 下载 `paper-vault.exe`，双击运行即可。

- **Windows 10/11**：系统自带 WebView2 Runtime，开箱即用
- 首次启动会让你选择一个空文件夹作为「库目录」，所有数据都存这里
- 在「设置 → AI」填入 OpenAI 兼容 API（base URL + key），即可使用 AI 功能

### 从源码构建

```bash
# 前置：Node 18+ / pnpm 9+ / Rust 1.74+ / WebView2 Runtime

# 安装前端依赖
pnpm install

# 开发模式（Vite + Tauri 热重载）
pnpm tauri:dev

# 类型检查 / 单元测试
pnpm typecheck
pnpm test

# 打包发布（生成 exe / msi / nsis 安装包）
pnpm tauri:build
```

## AI 配置

应用支持任何 OpenAI 兼容的 API 端点：

| 配置项 | 示例 |
|--------|------|
| Base URL | `https://api.openai.com/v1` / `https://api.deepseek.com/v1` / 自建网关 |
| API Key | `sk-...` |
| 模型名 | `gpt-4o-mini` / `deepseek-chat` / 任何 chat completions 模型 |

密钥保存在本地 SQLite 数据库，不进 Git，不上传任何地方。AI 功能是可选的，不开通也能正常用作 PDF 阅读器 + 笔记本。

## 项目结构

```text
paper-vault-core/
├── src/                    # 前端 React 应用
│   ├── components/         # UI 组件（库 / 阅读 / 搜索 / 设置 / AI）
│   ├── stores/             # Zustand 状态
│   ├── routes/             # 路由页面
│   └── lib/                # API 封装与工具
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── ai/             # AI 客户端与预设模板
│   │   ├── commands/       # Tauri 命令（前端可调用）
│   │   ├── db/             # SQLite 迁移与连接
│   │   ├── export/         # 多格式导出
│   │   ├── services/       # 业务服务（论文 / 笔记 / 搜索 / AI 对话）
│   │   ├── pdf.rs          # PDF 文本提取启发式
│   │   ├── markdown.rs     # Markdown AI 区块管理
│   │   └── lib.rs          # 应用入口与 setup
│   ├── capabilities/       # Tauri 权限配置
│   └── tauri.conf.json     # Tauri 构建配置
├── index.html
├── package.json
└── vite.config.ts
```

## 设计取舍

- **本地优先**：所有数据都在用户硬盘上，不依赖任何云服务。AI 调用是可选的增强，不是必需依赖。
- **Markdown 是一等公民**：笔记不是数据库里的富文本，而是磁盘上的 `.md` 文件，可以脱离应用直接用任何编辑器打开。
- **AI 写入有边界**：AI 只能写入笔记里的固定区块，用户手写的内容永远不会被覆盖。
- **PDF 是证据**：导入的 PDF 原样保留在 `pdfs/` 下，不做转换、不抽取片段单独存储，所有引用都回指原文。
- **可扩展的 Skill**：AI 提示词不写死在代码里，而是作为数据库里的预设，用户可以自由修改、复制、恢复默认。

## 许可

仅供学习与个人使用。
