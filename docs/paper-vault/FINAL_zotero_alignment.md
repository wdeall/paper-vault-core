# FINAL — Zotero 对齐项目总结报告

> 日期: 2026-06-22
> 项目: trea (paper-vault)
> 范围: SPEC_zotero_alignment.md v1.0 / PLAN_zotero_alignment.md v2.0

---

## 1. 执行概览

5 个 Milestone 全部实施完成,采用 subagent-driven-development 流程 (Implementer → 验证 → commit)。

| Milestone | Commit | 说明 | 后端测试 |
|---|---|---|---|
| M-A (P0 schema) | `47e7c26` | Zotero 对齐 schema + identifier 表 + creators/keywords 拆表 | — |
| M-B (P1+P2 导入+去重) | `22897cb` + `5c6ab43` | 4 resolver + 3 层去重 + 字段级合并 + 5min 撤销 | 13 (resolver) + 9 (merge) |
| M-C (P3 搜索) | `b0d5b5b` | structured/fulltext/both 三模式 + papers_fts + 移除 fulltext_index | 10 (search) |
| M-D (P4 批注) | `1d13a22` | annotation CRUD + pdf.js 选区 + 侧边栏 + Markdown 同步 | 12 (annotation) |
| M-E (P5 导出) | `3f46757` | CSL-JSON 中间层 + RIS + BibTeX 重构 + 文件保存 | 9 (export) |

**总测试**: 107 passed, 0 failed (`cargo test --lib`)
**前端**: `npx tsc --noEmit` 通过,无类型错误

---

## 2. SPEC 验收标准对齐

### 7.1 P0 数据模型 — 5/5 通过

- [x] 新 schema 全部表创建成功 (0001 + 0002 + 0003 migration)
- [x] 旧 JSON 字段全部 drop (migrate_v2 迁移逻辑)
- [x] `papers.status` 枚举 CHECK 约束生效
- [x] 旧 vault.db 备份存在 (migrate_v2 备份)
- [x] 启动后 vault 可正常工作

### 7.2 P1 标识符优先导入 — 4/4 通过

- [x] 4 种 identifier 均可 resolve (Crossref / arXiv / PubMed / OpenLibrary)
- [x] 导入对话框支持手输 / 粘贴 / PDF (ImportByIdDialog.tsx)
- [x] 失败时显示具体错误 (friendlyMessage 剥离 thiserror 前缀)
- [x] Crossref / arXiv / NCBI / OpenLibrary 网络错误重试一次 (send_with_retry)

### 7.3 P2 去重与合并 — 3/4 通过

- [x] 同一 DOI 二次导入触发冲突提示 (DOI UNIQUE + duplicate.rs)
- [x] 合并对话框列出所有字段差异 (MergeDialog pickDiff 7 字段)
- [x] 5min 内可撤销合并 (merge_log JSON snapshot + undo_merge)
- [ ] **启动后台扫发现重复时通知区提示** — 未实现自动扫描+通知 (手动 check_duplicates 可用)

### 7.4 P3 双通道搜索 — 4/4 通过

- [x] StructuredQuery 7 字段独立可用 (title/author/year/venue/doi/status/keyword)
- [x] FTS 搜索返回带权重排序 (bm25(papers_fts) + score)
- [x] 双通道结果可正确合并 (search_both: FTS ∩ Structured)
- [x] 关键词搜索跨 paper 聚合可用 (list_keywords)

### 7.5 P4 PDF 批注 — 3/4 通过 (1 项需手动验证)

- [x] 5 色批注保存正确 (yellow/red/green/blue/purple)
- [?] **bbox 跨缩放稳定** — 归一化坐标实现正确,需手动验证不同缩放下位置一致
- [x] 侧边栏按颜色/页过滤 (AnnotationSidebar.tsx)
- [x] 批注导出为 `.md` 笔记正确 (sync_to_note + ANNOTATIONS_START/END 标记)

### 7.6 P5 标准化导出 — 1/4 通过 (3 项需手动验证)

- [?] **BibTeX 导入 Zotero 正确显示** — 格式符合规范,需手动导入 Zotero 验证
- [?] **RIS 导入 EndNote/Mendeley 正确显示** — 格式符合规范,需手动验证
- [?] **CSL-JSON 导入 Pandoc 正确** — JSON 结构符合 CSL schema,需手动验证
- [x] 特殊字符 (中文、LaTeX 符号) 转义正确 (escape_bibtex + 测试)

---

## 3. 代码质量评估

### 后端 (Rust)

- **编译**: `cargo check --tests` 无警告通过
- **测试**: 107 passed, 0 failed, 0 ignored
- **代码规范**: 中文注释, AppError/AppResult 错误处理, crate::db::open(vault) DB 访问模式
- **无新依赖**: 全部使用 Cargo.toml 已有依赖 (reqwest / serde_json / uuid / rusqlite)
- **clippy**: 因 tauri-build build script 在 sandbox 环境下 panic (Os code 0) 无法运行,非代码问题

### 前端 (React + TypeScript)

- **类型检查**: `npx tsc --noEmit` 通过,无类型错误
- **代码规范**: 中文注释和 UI 文案, 现有 UI 组件复用 (Button/Card/Textarea/Badge)
- **新依赖**: pdfjs-dist@^4.7.76 (M-D PDF 选区必需)
- **Tauri 插件**: @tauri-apps/plugin-dialog + @tauri-apps/plugin-fs (已存在于 package.json)

### 文档

- SPEC_zotero_alignment.md v1.0 — 需求规格 (未修改)
- PLAN_zotero_alignment.md v2.0 — 实施计划 (未修改)
- RECOVERY_2026-06-16_git_loss.md — 两次 .git 丢失事故记录
- 0003_drop_fulltext_index.sql — M-C 新增 migration

---

## 4. 技术决策记录

| 决策 | 选择 | 理由 |
|---|---|---|
| FTS 策略 | 彻底移除 fulltext_index,只保留 papers_fts | SPEC 要求 FTS 只索引 metadata;简化维护 |
| PDF 选区 | 引入 pdf.js (pdfjs-dist) | iframe 无法获取选区;pdf.js 提供文本层 |
| 批注导出 | 固定 HTML 注释区块 (ANNOTATIONS_START/END) | 可重复同步不覆盖用户手写内容 |
| 导出架构 | Paper → CSL-JSON → BibTeX/RIS | SPEC §3.6.2 统一中间表示 |
| 前端搜索 UI | 顶部模式切换 + 条件展开 | 符合 SPEC §3.4.3 三模式要求 |

---

## 5. Git 历史

```
3f46757 M-E (P5): 标准化导出 — CSL-JSON 中间层 + RIS + BibTeX 重构 + 文件保存
1d13a22 M-D (P4): PDF 批注 — annotation CRUD + pdf.js 选区 + 侧边栏 + Markdown 同步
b0d5b5b M-C (P3): 双通道搜索 — structured/fulltext/both + papers_fts + 移除 fulltext_index
5c6ab43 M-B cleanup: ImportByIdDialog/MergeDialog UX + AppError 去重 + SPEC 字段策略对齐
22897cb M-B cleanup: resolver transient retry + PubMed/OpenLibrary 404 tests
902ecad M-B cleanup: 10 dead code warnings 清理 + cleanup_old_merge_log startup hook
645885a docs: RECOVERY 记录事故 #2 + 防御措施
47e7c26 Reconstructed: M-A + M-B + polish + RECOVERY + cargo fix
```

Bundle 备份: `C:\Users\业天\AppData\Local\Temp\paper-vault-backup-2026-06-22-me.bundle`

---

## 6. 已知限制

1. **SearchPanel 孤儿组件**: 已实现但未接入路由,需要后续在 LibraryShell 或路由中挂载
2. **TopBar 搜索行为**: 搜索跳转 `/library` 后结果展示需手动验证
3. **cargo clippy**: tauri-build build script 在 sandbox 环境下 panic,需在非沙盒环境验证
4. **手动验收项**: P4 bbox 跨缩放 / P5 三格式导入 Zotero/EndNote/Pandoc 需手动验证
