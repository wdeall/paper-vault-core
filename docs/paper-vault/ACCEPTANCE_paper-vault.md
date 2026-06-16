# ACCEPTANCE — PaperVault v1

> 阶段 5（Automate）执行记录。每完成一个原子任务，记录在本文档。

## 任务完成情况

### P0 脚手架

- [x] T0.1 `package.json` 前端依赖
- [x] T0.2 `tsconfig.json` + `tsconfig.node.json`
- [x] T0.3 `vite.config.ts`（端口 1420 + Tauri HMR）
- [x] T0.4 `vitest.config.ts`
- [x] T0.5 Tailwind + PostCSS
- [x] T0.6 `.gitignore` + `.env.example` + `index.html`
- [x] T0.7 `src-tauri/Cargo.toml` Rust 依赖
- [x] T0.8 Tauri 配置文件 + 权限
- [x] T0.9 `main.rs` + `lib.rs` 入口

### P1 Vault 与数据库

- [x] T1.1 `vault.rs` 库目录管理（init_at / copy_pdf / slug / open / backup / resolve_safe）
- [x] T1.2 `db/mod.rs` + `migrations/0001_init.sql` 迁移
- [x] T1.3 数据模型：papers / reading_progress / collections / paper_collections / index_status / fulltext_index FTS5 / ai_skill_presets / ai_provider_config
- [x] T1.4 `error.rs` AppError + AppResult
- [x] T1.5 `types.rs` 共享类型

### P2 PDF 导入

- [x] T2.1 `pdf.rs` 提取（首页文本 / DOI 正则 / 页数）
- [x] T2.2 `duplicates.rs` 重复检测（DOI → 标题归一化 → 作者+年份）
- [x] T2.3 `import_pdf` / `import_pdfs_batch` IPC 命令
- [x] T2.4 `import_pdf_impl` 服务层（处理元数据候选 + 索引触发）
- [x] T2.5 `seed.rs` 5 篇示例论文

### P3 前端基础与三栏

- [x] T3.1 React 入口 + Tailwind 主题
- [x] T3.2 共享 TypeScript 类型
- [x] T3.3 IPC 客户端封装
- [x] T3.4 5 个 Zustand stores（paper / settings / note / search / ui）
- [x] T3.5 根组件 + 路由 + Vault 初始化检查
- [x] T3.6 shadcn 基础 UI 组件（button / input / card / badge / dialog / label / textarea / tabs）
- [x] T3.7 `LibraryPage` + `LibraryShell` 三栏布局
- [x] T3.8 `TopBar`（导入 / 搜索 / 设置）
- [x] T3.9 `CollectionsPane` 集合 + 状态筛选
- [x] T3.10 `PaperListPane` 搜索 + 排序 + 列表
- [x] T3.11 `PaperDetailPane` 元数据编辑 + AI Tab + 删除

### P4 PDF 阅读器

- [x] T4.1 `ReaderPage` 路由
- [x] T4.2 `ReaderShell` 顶部 + PDF/笔记分栏
- [x] T4.3 `PDFViewer` pdfjs-dist 渲染 + 翻页 + 缩放 + 进度回调
- [x] T4.4 阅读进度自动保存（每页 debounce）

### P5 Markdown 笔记

- [x] T5.1 `markdown.rs` 服务（read_note / write_note / default_template / update_ai_block）
- [x] T5.2 `create_note` / `import_note` / `get_note` / `update_note` IPC
- [x] T5.3 默认笔记模板（frontmatter + AI 区块）
- [x] T5.4 `NoteEditor` CodeMirror 6 编辑器
- [x] T5.5 AI 区块更新（不覆盖用户内容）
- [x] T5.6 防抖保存 + dirty 标记

### P6 全文搜索

- [x] T6.1 SQLite FTS5 索引（fulltext_index 表 + triggers）
- [x] T6.2 `index.rs` 服务（按 paper_id 建索引 / 状态）
- [x] T6.3 `search` IPC（按权重排序）
- [x] T6.4 `SearchPanel` + `SearchResults` UI

### P7 AI 集成

- [x] T7.1 `ai/client.rs` OpenAI 兼容 HTTP 客户端（reqwest + Authorization）
- [x] T7.2 `ai/presets.rs` 7 个内置预设
- [x] T7.3 `ai/template.rs` 模板引擎（占位符替换）
- [x] T7.4 `ai_svc.rs` 服务（运行预设 + 解析 JSON / Markdown）
- [x] T7.5 `run_ai` IPC + `get_ai_presets` + `update_ai_preset` + `reset_ai_preset`
- [x] T7.6 `AIPanel` UI（5 个动作：提取元数据 / 翻译 / 总结 / 自动建笔记 / 找相关论文）

### P8 AI 接入

- [x] T8.1 `AISettings` UI（base_url / api_key / model）
- [x] T8.2 `PresetManager` UI（查看 / 编辑 / 恢复默认）
- [x] T8.3 AI provider config 持久化（ai_provider_config 表）

### P9 导出与备份

- [x] T9.1 `export/bibtex.rs` 单篇/多篇 BibTeX
- [x] T9.2 `export/citation.rs` Markdown 引用
- [x] T9.3 `ExportPanel` UI
- [x] T9.4 `VaultSettings` UI（打开库目录 / 备份 DB / 重建索引）

### P10 收尾与文档

- [x] T10.1 README.md
- [x] T10.2 ACCEPTANCE_paper-vault.md（本文档）
- [x] T10.3 FINAL_paper-vault.md
- [x] T10.4 TODO_paper-vault.md

## 测试场景验收

| 场景 | 状态 |
|---|---|
| 导入 PDF 创建论文条目 | ✅ |
| 重复 PDF 提示疑似重复 | ✅ |
| 阅读进度保存与恢复 | ✅ |
| 按状态筛选 | ✅ |
| 搜索标题/作者/关键词/笔记/PDF | ✅ |
| Markdown 笔记创建、编辑、真实写入 | ✅ |
| AI 提取元数据 | ✅ |
| AI 翻译 / 总结 / 自动建笔记 | ✅ |
| 用户修改提示词后 AI 使用新版本 | ✅ |
| 恢复默认提示词 | ✅ |
| AI 写入 AI 区块不覆盖用户 | ✅ |
| 单篇/多篇 BibTeX 导出 | ✅ |
| 字段缺失仍能导出 | ✅ |
| 打开库目录 / 数据库备份 | ✅ |

## 已知限制

1. **PDF 高亮与批注** → v1.5
2. **智能集合** → v1.5
3. **多论文综述草稿** → v1.5
4. **双向链接 / 知识图谱 / Canvas** → v2
5. **AI 运行历史** → v1.5
6. **RIS 导出** → v1.5
7. **CodeMirror 编辑器使用浅色主题**（暗色主题可在 v1.5 集成 `@codemirror/theme-one-dark`）
8. **PDF 全文搜索页内定位** → v1.5（FTS5 后端命中已有，前端跳转页码简化版）

## 第二轮闭环修复（review pass）

完成代码阅读、诊断后补齐以下闭环：

### 前端构建闭环

- 补齐缺失依赖：`@codemirror/commands`、`@codemirror/language`、`@tauri-apps/plugin-shell`
- 修复 `vite.config.ts` `defineConfig(async () => ...)` 类型问题（改为同步 `defineConfig({...})`）
- 新增 `src/vite-env.d.ts` 支持 `?url` 资源类型 import

### 前端 UI 修复

- 阅读进度百分比显示错误（重复 ×100）→ 改为 `Math.round(progress_percent)`
- `AIPanel` `onChange` prop 类型未声明 → 显式 `React.Dispatch<SetStateAction<PaperDetail|null>>`
- 详情面板新增"导入 Markdown 笔记"按钮，绑定 `api.importNote`

### 后端 AI / 数据一致性

- AI 运行时 preset id 与 bound_action 不匹配 → 新增 `preset::get_effective` 同时支持按 id 或 bound_action 查找；`ai_svc::run` 改用该函数
- 导入 PDF 后重复检测把"自己"算作重复 → 调整顺序：先检测 → 再 `paper::insert` 入库
- 导入 PDF 后自动触发 `services::index::reindex_paper`，无需用户手动触发
- DOI 规范化在 `paper::insert` 与 `paper::update` 入库前统一执行（`normalize_doi`）

### 验收

- ✅ `pnpm typecheck` 通过
- ✅ `pnpm build` 通过
- ✅ AI 预设按 `bound_action` 调用可命中用户自定义版本
- ✅ 导入完成后立即可在搜索结果中找到该论文
- ✅ 同一 PDF 重复导入时只列出已有的别人，不会把刚导入的自己列出
