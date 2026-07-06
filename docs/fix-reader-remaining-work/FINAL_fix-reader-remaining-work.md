# FINAL — 修复阅读界面选区/布局/批注类型并清理 clippy warnings

## 1. 概述

本次任务是 PaperVault v1 阅读工作台的验收整改,针对用户实际使用反馈的 4 个问题进行处理,并顺带完成 TODO 5 / 8 / 9 / 10 的剩余工作。

**完成度**: 4 项验收问题全部闭环 + 4 项 TODO 完成 + clippy 0 warning

## 2. 验收问题处理

### 2.1 重复论文无提示(验收问题 1)

**现象**: TODO 1 启动时重复扫描通知未出现。

**根因**: Tauri event 时序竞态 — 后端 `setup()` 中 `emit("duplicates-found", ...)` 早于前端 `listen("duplicates-found", ...)` 注册完成,事件丢失。

**修复**: 前端 `App.tsx::checkVault()` 在 `setVaultReady(true)` 后主动调用 `api.scanDuplicates()` 拉取结果,绕开 event 时序依赖。后端事件通道保留作为兜底。

**改动**:
- `src/lib/api.ts` — 新增 `scanDuplicates: () => call<DuplicatePair[]>("scan_duplicates")`
- `src/types/index.ts` — 新增 `DuplicatePair` interface(与 Rust `duplicates::DuplicatePair` 对齐)
- `src/App.tsx` — `checkVault()` 中加主动拉取逻辑
- `src-tauri/src/commands/papers.rs` — 新增 `scan_duplicates` 命令(`lib.rs` 已注册但原缺实现)
- `src-tauri/src/duplicates.rs` — `DuplicatePair` 加 `Default` derive

### 2.2 选区范围过大 + 阅读界面布局(验收问题 2)

**现象**: PDF 跨行选区时高亮矩形远大于原文;阅读界面笔记区过大,论文区过小,批注/笔记栏无法收起。

**根因(选区)**: `range.getBoundingClientRect()` 对跨行选区返回并集矩形(整行宽度 × 多行高度),导致高亮覆盖整段文字。

**修复(选区)**: 改用 `range.getClientRects()` 取逐行 rect 数组,`Annotation.rect` 类型从 `AnnotationRect | null` 改为 `AnnotationRect[] | null`,高亮覆盖层用 `flatMap` 渲染多个 div。`parseAnnotation` 兼容旧单 rect 数据(自动包成数组)。

**修复(布局)**: `ReaderShell.tsx` 三栏可收起:
- PDF 区: `flex-[2_1_0%] min-w-[400px]`(占比增大,不收起)
- 批注栏: 展开时 `w-[240px]`,收起时 `w-[40px]` 竖条
- 笔记区: 展开时 `flex-[1_1_0%] min-w-[320px]`,收起时 `w-[40px]` 竖条
- 新增 `notesCollapsed` / `annotationsCollapsed` state + 收起/展开按钮(`PanelLeftClose/Open` / `PanelRightClose/Open` 图标)

**改动**:
- `src/components/reader/PDFViewer.tsx` — `SelectionState.rect` 改数组类型,`handleMouseUp` 改用 `getClientRects()`,高亮覆盖层 `flatMap` 多 rect,新增 `renderStyle()` 按 kind 分支
- `src/components/reader/ReaderShell.tsx` — 三栏 flex 比例调整 + 可收起
- `src/types/index.ts` — `Annotation.rect` 改 `AnnotationRect[] | null`
- `src/lib/api.ts` — `parseAnnotation` 兼容数组/旧单 rect,`createAnnotation`/`updateAnnotation` rect 类型改数组

### 2.3 clippy 15 warnings(验收问题 3)

**现象**: `cargo clippy --lib --all-features` 报 15 个 warning。

**修复**(全部清零):

| lint | 数量 | 修复方式 |
|------|------|----------|
| `needless_lifetimes` | 7 | 提取 `commands/common.rs::require_vault(state: &State<'_, AppState>)` 共享,7 个 command 模块改 `use crate::commands::common::require_vault;` |
| `too_many_arguments` | 2 | `annotation::create` 签名从 8 参数改为 `(vault, paper_id, &AnnotationInput)`,`commands::create_annotation` 同步;新增 `AnnotationInput` 结构体 |
| `field_reassign_with_default` | 1 | `index.rs::status_summary` 改用结构体字面量初始化 `IndexStatusSummary { total, indexed, ..Default::default() }` |
| `derivable_impls` | 1 | `PaperStatus` 改用 `#[derive(Default)]` + `#[default]` 属性(移除手动 `impl Default`) |
| `single_char_add_str` | 1 | `bibtex.rs` `out.push_str(" ")` → `out.push(' ')` |
| `manual_pattern_char_comparison` | 1 | `identifier.rs:133` `trim_end_matches(\|c: char\| c == '.' \|\| ...)` → `trim_end_matches(['.', ',', ';', ')'])` |
| `collapsible_str_replace` | 1 | `identifier.rs:248` `.replace('-', "").replace(' ', "")` → `.replace(['-', ' '], "")` |
| `doc_overindented_list_items` / `doc_lazy_continuation` | 1 | `identifier.rs:6` 文档注释 URL 缩进从 9 空格改为 6 空格 |

**改动文件**:
- `src-tauri/src/commands/common.rs`(新增)
- `src-tauri/src/commands/{ai,annotation,export,notes,papers,search,settings,mod}.rs`(改用共享 require_vault)
- `src-tauri/src/services/{annotation,index,identifier}.rs`
- `src-tauri/src/export/bibtex.rs`
- `src-tauri/src/types.rs`

### 2.4 TODO 8/9/10 执行(验收问题 4)

#### TODO 8 — DOI 搜索归一化

`src-tauri/src/duplicates.rs::normalize_doi()` 处理:
- `https://doi.org/` 前缀剥离
- `http://doi.org/` 前缀剥离
- `doi:` 前缀剥离
- 全小写 + trim

`import_pdf` / `import_by_identifier` 均在入库前调用 `normalize_doi`,确保重复检测命中率。

#### TODO 9 — FTS 自动同步

`src-tauri/src/services/paper.rs`:
- `insert()`(line 340)调用 `rebuild_fts_for_paper(&tx, &paper.id)?`
- `update()`(line 438)调用 `rebuild_fts_for_paper(&tx, id)?`

论文入库/更新即自动同步 `papers_fts` 虚拟表,无需手动 `reindex_all`。

#### TODO 10 — 批注 kind 扩展

`Annotation.kind` 支持 `highlight | note | underline | strike`(原本仅 `highlight | note`)。

前端渲染:
- `PDFViewer.tsx::renderStyle(kind, r, color)` 按 kind 分支:
  - `highlight` / `note`: 半透明背景色矩形(`colorRgba`)
  - `underline`: 2px 实色线在矩形底部(`colorSolid`)
  - `strike`: 2px 实色线在矩形中部(`colorSolid`)
- 工具条 5 色按钮后加 underline / strike 按钮 + 分隔线
- `handleCreateAnnotation(kind: "underline" | "strike")` 回调(默认黄色实色)
- `AnnotationSidebar.tsx` 列表头部 kind 图标分支(用 `<span title>` 包裹避免 lucide title prop 报错)

## 3. 改动清单

### 3.1 后端(Rust)

| 文件 | 改动 |
|------|------|
| `src-tauri/src/commands/common.rs` | **新增** — 共享 `require_vault` |
| `src-tauri/src/commands/ai.rs` | 改用共享 require_vault |
| `src-tauri/src/commands/annotation.rs` | `create_annotation` 入参改 `AnnotationInput` |
| `src-tauri/src/commands/export.rs` | 改用共享 require_vault |
| `src-tauri/src/commands/mod.rs` | 注册 common 模块 |
| `src-tauri/src/commands/notes.rs` | 改用共享 require_vault |
| `src-tauri/src/commands/papers.rs` | 改用共享 require_vault + 新增 `scan_duplicates` 命令 |
| `src-tauri/src/commands/search.rs` | 改用共享 require_vault |
| `src-tauri/src/commands/settings.rs` | 改用共享 require_vault |
| `src-tauri/src/duplicates.rs` | `DuplicatePair` 加 Default derive |
| `src-tauri/src/export/bibtex.rs` | `push_str(" ")` → `push(' ')` |
| `src-tauri/src/services/annotation.rs` | `create` 签名改 `&AnnotationInput` + 8 处测试同步更新 |
| `src-tauri/src/services/identifier.rs` | `trim_end_matches` 数组语法 + `replace` 数组语法 + 文档缩进 |
| `src-tauri/src/services/index.rs` | `status_summary` 结构体字面量初始化 |
| `src-tauri/src/services/paper.rs` | (无本轮改动,确认 TODO 9 已就位) |
| `src-tauri/src/services/search.rs` | (无本轮改动) |
| `src-tauri/src/types.rs` | `AnnotationInput` 新增 + `PaperStatus` 改 `#[derive(Default)]` |
| `src-tauri/src/lib.rs` | `scan_duplicates` 命令注册 |

### 3.2 前端(TypeScript/React)

| 文件 | 改动 |
|------|------|
| `src/App.tsx` | `checkVault` 中主动调用 `scanDuplicates` |
| `src/components/reader/AnnotationSidebar.tsx` | kind 图标分支 + `handleClickAnnotation` 改用 `a.rect[0]` |
| `src/components/reader/PDFViewer.tsx` | `getClientRects` 多 rect + `renderStyle` kind 分支 + underline/strike 工具条按钮 |
| `src/components/reader/ReaderShell.tsx` | 三栏 flex 比例调整 + 可收起 |
| `src/lib/api.ts` | `parseAnnotation` 兼容数组 + `scanDuplicates` 新增 |
| `src/types/index.ts` | `Annotation.rect` 改数组 + `DuplicatePair` 新增 |

## 4. 验证结果

| 验证项 | 命令 | 结果 |
|--------|------|------|
| Rust 单元测试 | `cargo test --lib` | **110 passed, 0 failed** |
| Rust lint | `cargo clippy --lib --all-features` | **0 warning** |
| TypeScript 类型检查 | `npx tsc --noEmit` | **0 error** |
| Git commit | `git commit d9af548` | 23 文件,+504/-243 |

### 测试覆盖

- `services::annotation::tests`: 9 个测试全部通过(含 `test_create_annotation` / `test_list_by_paper` / `test_update_annotation` / `test_delete_annotation` / `test_export_to_markdown_*` / `test_sync_to_note_*`)
- `duplicates::tests`: 4 个测试全部通过(`doi_normalize` / `title_normalize` / `detect_by_doi` / `scan_all_*`)
- `services::identifier::tests`: 20+ 个测试全部通过(DOI/arXiv/PMID/ISBN 解析 + ISBN 校验)
- 其他模块测试: 全部通过

## 5. 质量评估

### 5.1 代码质量

- **规范一致性**: 严格遵循现有项目代码风格(中文注释 + 英文变量名 + 4 空格缩进 Rust / 2 空格 TS)
- **可读性**: `renderStyle` 函数按 kind 分支清晰;`AnnotationInput` 结构体封装避免长参数列表
- **复杂度**: 无过度设计,改动范围严格限定在 4 项验收问题
- **复用性**: `common::require_vault` 消除 7 处重复,后续新 command 直接复用

### 5.2 测试质量

- 8 处 `create()` 测试调用同步更新为新签名,断言内容保持不变
- 无新增测试(本次为修复 + 重构,无新功能需要新测试)
- 现有 110 个测试全部通过,无回归

### 5.3 文档质量

- 代码注释同步更新(`AnnotationInput` 文档 / `renderStyle` 注释 / 三栏收起说明)
- 本 FINAL 报告完整记录改动清单和验证结果

### 5.4 系统集成

- `scan_duplicates` 命令在 `lib.rs` 已注册,前端 `api.scanDuplicates` 可正常调用
- `AnnotationInput` 结构体 serde 对齐前端 `input` 参数
- `Annotation.rect` 数组类型向后兼容旧单 rect 数据(`parseAnnotation` 自动包装)
- FTS 自动同步在 `paper::insert` / `update` 中调用,无需前端手动触发

### 5.5 技术债务

- **无新增技术债务**
- **消除既有债务**: clippy 15 warnings 全部清零;7 处 `require_vault` 重复提取为共享函数

## 6. 端到端验证

启动 `npm run tauri:dev`,应用编译启动成功。

待用户手工验证项:
- [ ] 启动时若库内有重复论文,应出现 warning toast(10 秒)
- [ ] PDF 跨行选区时,高亮矩形应逐行覆盖(非整段并集)
- [ ] 阅读界面 PDF 区占比增大,批注栏 240px,笔记区可收起
- [ ] 批注栏 / 笔记栏收起后变 40px 竖条,点击展开
- [ ] 选区工具条 5 色按钮后有 underline / strike 按钮,点击生成对应批注
- [ ] 批注侧栏列表中 underline / strike 批注显示对应图标

## 7. 假设与决策记录

1. **scan_duplicates 用主动拉取而非事件** — 时序竞态下事件不可靠,主动拉取保证可见性。后端事件通道保留作为兜底(若未来加增量扫描可复用)。
2. **Annotation.rect 改数组但兼容旧数据** — `parseAnnotation` 检测 `Array.isArray`,非数组自动包成 `[parsed]`,旧批注无需迁移。
3. **underline/strike 默认黄色实色** — 与 highlight 黄色半透明区分;5 色按钮仅作用于 highlight,underline/strike 固定黄色。
4. **三栏比例 `flex-[2_1_0%]` / `w-[240px]` / `flex-[1_1_0%]`** — PDF 区权重 2,笔记区权重 1,批注栏固定宽度。收起时变 40px 竖条。
5. **git author identity 本仓库局部配置** — `user.name=trea / user.email=trea@local`(与历史 commit 一致,未动全局配置)。

## 8. 相关文档

- 计划文件: `.trae/documents/fix-reader-remaining-work.md`(已批准)
- 计划文件: `.trae/documents/fix-annotation-tests-and-verify.md`(已批准)
- 本轮 commit: `d9af548 fix: 修复阅读界面选区/布局/批注类型并清理 clippy warnings`
