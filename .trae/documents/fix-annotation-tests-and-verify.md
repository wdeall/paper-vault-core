# 修复 annotation.rs 测试 + 最终验证 + 提交

## 背景

上一轮已完成 4 项验收整改:
- **A3 重复论文提示** — `App.tsx` 启动时主动调用 `api.scanDuplicates()`,绕开后端 setup emit 早于前端 listen 的时序竞态
- **B 选区范围过大** — `PDFViewer.tsx` 改用 `range.getClientRects()` 取逐行 rect,`Annotation.rect` 类型改为 `AnnotationRect[]`,`parseAnnotation` 兼容旧单 rect 数据,高亮覆盖层 `flatMap` 多 rect 渲染
- **C 阅读界面布局** — `ReaderShell.tsx` 三栏可收起(PDF `flex-[2_1_0%]`、批注 `w-[240px]`、笔记 `flex-[1_1_0%]`),收起时变 40px 竖条
- **G underline/strike 批注** — `renderStyle` 按 kind 分支渲染,工具条加 underline/strike 按钮,`AnnotationSidebar` kind 图标分支

后端 clippy 15 warnings 已修复 14 项:
- `needless_lifetimes` (7 处) → 提取 `commands/common.rs::require_vault` 共享
- `too_many_arguments` → `AnnotationInput` 结构体封装
- `field_reassign_with_default` → `index.rs` 改结构体字面量初始化
- `derivable_impls` → `PaperStatus` 改 `#[derive(Default)]` + `#[default]`
- `single_char_add_str` → `bibtex.rs` 改 `out.push(' ')`
- `manual_pattern_char_comparison` → `identifier.rs:133` 改 `['.', ',', ';', ')']`
- `collapsible_str_replace` → `identifier.rs:248` 改 `replace(['-', ' '], "")`
- `doc_overindented_list_items` / `doc_lazy_continuation` → `identifier.rs:6` 缩进 6 空格

TODO 8/9/10 也已就位:
- **TODO 8** DOI 归一化 — `duplicates::normalize_doi` 处理 `https://doi.org/` / `doi:` 前缀 + 小写
- **TODO 9** FTS 自动同步 — `paper.rs` `insert`(line 340) / `update`(line 438) 均调用 `rebuild_fts_for_paper`
- **TODO 10** 批注 kind 扩展 — `Annotation.kind` 支持 `highlight | note | underline | strike`,`PDFViewer` 渲染分支 + 工具条按钮

## 当前阻塞

`cargo test --lib` 失败,**8 个编译错误全部来自 `src-tauri/src/services/annotation.rs` 测试模块**:

`create()` 签名已从旧的 8 参数 `(vault, paper_id, kind, page, rect, color, text, comment)` 改为 `(vault, paper_id, &AnnotationInput)`,但 8 处测试调用未同步更新。

## 实施步骤

### 步骤 1:修复 annotation.rs 测试模块 8 处旧签名调用

文件: `f:\learn\study\ai\trea\src-tauri\src\services\annotation.rs`

按行号逐个修改(参照已正确的 `test_create_annotation` line 387 / `test_delete_annotation` line 515 模式):

**1.1 `test_list_by_paper_order` (line 429, 432, 434)**

```rust
// line 429
let _a1 = create(dir.path(), "p1", "highlight", Some(2), None, None, None, None).unwrap();
// 改为
let _a1 = create(
    dir.path(),
    "p1",
    &AnnotationInput {
        kind: "highlight".into(),
        page: Some(2),
        rect: None,
        color: None,
        text: None,
        comment: None,
    },
)
.unwrap();

// line 432 同上,page: Some(1)
// line 434 同上,page: Some(1)
```

**1.2 `test_update_annotation` (line 452-461)**

```rust
let ann = create(
    dir.path(),
    "p1",
    &AnnotationInput {
        kind: "highlight".into(),
        page: Some(1),
        rect: None,
        color: Some("#fff".into()),
        text: Some("old text".into()),
        comment: Some("old comment".into()),
    },
)
.unwrap();
```

**1.3 `test_export_to_markdown_with_data` (line 563-572, 574-583)**

```rust
// 第一处 (highlight)
create(
    dir.path(),
    "p1",
    &AnnotationInput {
        kind: "highlight".into(),
        page: Some(5),
        rect: None,
        color: Some("#ffeb3b".into()),
        text: Some("important text".into()),
        comment: Some("a comment".into()),
    },
)
.unwrap();

// 第二处 (note)
create(
    dir.path(),
    "p1",
    &AnnotationInput {
        kind: "note".into(),
        page: Some(5),
        rect: None,
        color: None,
        text: None,
        comment: Some("standalone note".into()),
    },
)
.unwrap();
```

**1.4 `test_sync_to_note_no_note` (line 605)**

```rust
create(
    dir.path(),
    "p1",
    &AnnotationInput {
        kind: "highlight".into(),
        page: Some(1),
        rect: None,
        color: None,
        text: None,
        comment: None,
    },
)
.unwrap();
```

**1.5 `test_sync_to_note_insert` (line 657-666)**

```rust
create(
    dir.path(),
    "p1",
    &AnnotationInput {
        kind: "highlight".into(),
        page: Some(2),
        rect: None,
        color: Some("#ff0".into()),
        text: Some("highlighted".into()),
        comment: Some("note text".into()),
    },
)
.unwrap();
```

### 步骤 2:运行验证(三条命令必须全部通过)

```powershell
# 2.1 Rust 测试
cd f:\learn\study\ai\trea\src-tauri
C:\Users\业天\.cargo\bin\cargo.exe test --lib

# 2.2 Rust clippy (0 warning)
C:\Users\业天\.cargo\bin\cargo.exe clippy --lib --all-features

# 2.3 TypeScript 类型检查 (0 error)
cd f:\learn\study\ai\trea
npx tsc --noEmit
```

**通过标准**:
- `cargo test --lib`: 所有测试 pass(含 annotation.rs 9 个测试 + duplicates.rs 4 个测试 + identifier.rs 测试等)
- `cargo clippy --lib --all-features`: 0 warning
- `npx tsc --noEmit`: 0 error

### 步骤 3:Git commit

```powershell
cd f:\learn\study\ai\trea
git status
git diff --stat
git log --oneline -5
# 暂存本次改动的文件(精确指定,不用 git add -A)
git add src-tauri/src/services/annotation.rs `
        src-tauri/src/services/identifier.rs `
        src-tauri/src/services/index.rs `
        src-tauri/src/export/bibtex.rs `
        src-tauri/src/types.rs `
        src-tauri/src/commands/common.rs `
        src-tauri/src/commands/annotation.rs `
        src-tauri/src/commands/papers.rs `
        src-tauri/src/duplicates.rs `
        src-tauri/src/lib.rs `
        src/types/index.ts `
        src/lib/api.ts `
        src/App.tsx `
        src/components/reader/PDFViewer.tsx `
        src/components/reader/ReaderShell.tsx `
        src/components/reader/AnnotationSidebar.tsx
git commit -m "fix: 修复阅读界面选区/布局/批注类型并清理 clippy warnings

- PDFViewer: 选区改用 getClientRects 取逐行 rect,避免跨行选区高亮矩形过大
- Annotation.rect 类型改为 AnnotationRect[],parseAnnotation 兼容旧单 rect 数据
- 新增 underline/strike 批注 kind,renderStyle 按 kind 分支渲染
- ReaderShell: 三栏(PDF/批注/笔记)可收起,PDF 占比增大,笔记区缩小
- App.tsx: 启动时主动调用 scanDuplicates,绕开 setup emit 早于 listen 的时序竞态
- annotation: create 签名改用 AnnotationInput 结构体(避免 too_many_arguments)
- types: PaperStatus 改用 #[derive(Default)] + #[default]
- 修复 clippy 15 warnings: needless_lifetimes / too_many_arguments / field_reassign_with_default / derivable_impls / single_char_add_str / manual_pattern_char_comparison / collapsible_str_replace / doc_overindented_list_items
- 同步更新 annotation.rs 测试模块 8 处 create() 调用为新签名"
git status
```

## 假设与决策

1. **不动 `commands/notes.rs` / `commands/search.rs` / `commands/ai.rs` / `commands/export.rs` / `commands/settings.rs` 的 `require_vault`** — 这些文件已改为 `use crate::commands::common::require_vault;`(共享 common.rs 的版本),无需重复修改。验证阶段若 clippy 仍报 `needless_lifetimes` 再处理。
2. **不修改测试逻辑** — 仅改 `create()` 调用签名,断言内容保持不变
3. **不追加新功能** — 严格限定在本计划范围内
4. **PowerShell heredoc 限制** — commit message 用多个 `-m` 参数或反引号续行,不用 `<<'EOF'`

## 验证清单

- [ ] `cargo test --lib` 全绿
- [ ] `cargo clippy --lib --all-features` 0 warning
- [ ] `npx tsc --noEmit` 0 error
- [ ] git commit 成功,working tree clean
