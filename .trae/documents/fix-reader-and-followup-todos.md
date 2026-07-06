# 计划：修复 Reader 体验 + 重复通知 + clippy + TODO 8/9/10

> 日期: 2026-06-23
> 范围: 用户反馈 4 项问题 + 后续 TODO 8/9/10 执行

---

## 总结

用户验收后发现 4 个问题，并要求执行 TODO 8/9/10：

1. **重复论文无提示** — 启动时序竞态导致事件丢失 + DOI 未归一化导致 scan_all 找不到
2. **PDF 选区远大于原文** — `getBoundingClientRect()` 对跨行选区返回并集矩形
3. **Reader 布局失衡** — 笔记区 `min-w-[420px]` 挤压 PDF 区，无收起机制
4. **clippy 15 个 warning** — 7 个 require_vault 重复 + 2 个 too_many_arguments + 6 个杂项
5. **TODO 8** — DOI 搜索未归一化
6. **TODO 9** — FTS 索引需手动 reindex
7. **TODO 10** — 批注 kind 仅支持 highlight/note

---

## 当前状态分析

### 问题 1: 重复通知无提示（根因双重的）

**根因 A — 时序竞态**（[lib.rs:60-73](file:///f:/learn/study/ai/trea/src-tauri/src/lib.rs#L60-L73) vs [App.tsx:33-49](file:///f:/learn/study/ai/trea/src/App.tsx#L33-L49)）：
- 后端 setup() 立即 spawn → scan_all（只查 SQLite，毫秒级）→ emit
- 前端 mount → `await api.getVaultInfo()`（IPC 往返）→ `setVaultReady(true)` → 才注册 listen
- emit 发生在 listen 注册之前 → 事件丢失

**根因 B — DOI 未归一化**（[paper.rs:464-485 insert_from_metadata](file:///f:/learn/study/ai/trea/src-tauri/src/services/paper.rs#L464-L485)）：
- `insert_from_metadata` 直接 `doi: meta.doi.clone()`，未调 `normalize_doi`
- `scan_all` SQL 用 `a.doi = b.doi` 严格字符串比较
- 同一 DOI 不同来源（大小写/前缀差异）→ DB 里不同字符串 → 找不到重复

### 问题 2: PDF 选区远大于原文

**根因**（[PDFViewer.tsx:262](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx#L262)）：
- `range.getBoundingClientRect()` 对跨行选区返回**并集矩形**（包含中间空白）
- 高亮覆盖层（行 428-438）用单个 rect 渲染 → 跨行选区显示为巨大矩形
- 正确做法：用 `range.getClientRects()` 逐行取 rect，渲染多个高亮块

### 问题 3: Reader 布局失衡

**根因**（[ReaderShell.tsx:162-201](file:///f:/learn/study/ai/trea/src/components/reader/ReaderShell.tsx#L162-L201)）：
- PDF 区 `flex-1` 无 min-width（行 164）
- 批注栏 `w-[240px] shrink-0` 固定（行 176）
- 笔记区 `flex-1 min-w-[420px]`（行 185）— 窗口缩小时挤压 PDF
- **无任何收起机制** — AnnotationSidebar/NoteEditor 都无法折叠

### 问题 4: clippy 15 warnings（用户已贴完整输出）

| 类别 | 数量 | 位置 |
|------|------|------|
| `needless_lifetimes` (require_vault) | 7 | commands/{papers,notes,search,ai,export,settings,annotation}.rs |
| `too_many_arguments` (create_annotation) | 1 | commands/annotation.rs:19 |
| `field_reassign_with_default` | 1 | services/index.rs:86 |
| `doc_overindented_list_items` | 1 | services/identifier.rs:6 |
| `manual_pattern_char_comparison` | 1 | services/identifier.rs:133 |
| `collapsible_str_replace` | 1 | services/identifier.rs:248 |
| `too_many_arguments` (annotation::create) | 1 | services/annotation.rs:41 |
| `single_char_add_str` | 1 | export/bibtex.rs:64 |
| `derivable_impls` | 1 | types.rs:51 |

### TODO 8: DOI 搜索未归一化

[search.rs:114-119](file:///f:/learn/study/ai/trea/src-tauri/src/services/search.rs#L114-L119) 和 search_both 两处 `p.doi = ?` 直接比较，未调 `normalize_doi`。

### TODO 9: FTS 索引需手动 reindex

- [paper.rs insert:249 / update:346 / delete:447](file:///f:/learn/study/ai/trea/src-tauri/src/services/paper.rs#L249) 未自动调 `sync_papers_fts`
- [commands/papers.rs:184, 484](file:///f:/learn/study/ai/trea/src-tauri/src/commands/papers.rs#L184) 手动调 `reindex_paper`
- [sync_papers_fts](file:///f:/learn/study/ai/trea/src-tauri/src/services/search.rs#L24) 已存在，只需在事务内调用

### TODO 10: 批注 kind 仅 highlight/note

[PDFViewer.tsx:298,324](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx#L298) 只创建 highlight/note。SPEC 提到 underline/strike/image。

---

## 假设与决策

### 决策 1: 重复通知修复策略 — 双管齐下
- **修时序竞态**: 改用 Tauri 事件**缓存模式** — 后端先把扫描结果存入 AppState，前端 listen 注册后主动调一个 `get_pending_duplicates` 命令拉取；或更简单：前端 listen 注册成功后，前端主动调 `check_duplicates_all` 命令触发一次扫描。
- **选简化方案**: 前端 `vaultReady=true` 后主动调 `scan_all` 命令（新建），前端自己弹 toast。后端 setup() 的 spawn 保留作为兜底（万一前端没启动）。
- **修 DOI 归一化**: `insert_from_metadata` 写 DB 前调 `normalize_doi`。

### 决策 2: PDF 选区 — 改用 getClientRects()
- 选区改用 `range.getClientRects()` 取多行 rect 数组
- `AnnotationRect` 改为支持多 rect（`rects: Array<{x,y,w,h}>`）
- 后端 `annotation.rect` 字段存 JSON 数组（兼容旧单 rect：解析时若为对象则包装成 `[rect]`）
- 高亮覆盖层 map rects 渲染多个 div

### 决策 3: Reader 布局 — 三栏可收起 + 比例调整
- PDF 区 `flex-[2]` （占比更大）
- 笔记区 `flex-[1] min-w-[320px]`（降低 min-width）
- 批注栏 `w-[240px] shrink-0` 保持，但加收起按钮
- 加 `notesCollapsed` / `annotationsCollapsed` 状态到 ReaderShell，收起时宽度变 0（或留一个 40px 折叠条）
- 收起按钮放在各 section header 或边界拖拽条上

### 决策 4: clippy 修复策略
- 7 个 `require_vault` 提取到 `commands/mod.rs` 或新建 `commands/common.rs` 作为 pub fn，各模块 `use super::common::require_vault` 或 `use crate::commands::common::require_vault`
- 2 个 `too_many_arguments`: annotation create/create_annotation 引入 `AnnotationInput` 结构体封装 6 个字段（kind/page/rect/color/text/comment）
- 其余 6 个杂项按 clippy 提示直接修

### 决策 5: TODO 8 — DOI 搜索归一化
- search.rs 两处 `query.doi` 先调 `normalize_doi` 再比较

### 决策 6: TODO 9 — FTS 自动同步（应用层方案 A）
- paper.rs `insert` / `update` / `delete` 事务内调 `search::sync_papers_fts` / `DELETE FROM papers_fts`
- 删掉 commands/papers.rs:184, 484 手动 reindex 调用（保留 reindex_paper 命令供 reindex_all 用）

### 决策 7: TODO 10 — 批注 kind 扩展（本期只做 underline/strike，image 延后）
- 后端 annotation 表无 CHECK 约束，无需迁移
- 前端 PDFViewer 选区工具条加 underline/strike 按钮
- 高亮覆盖层按 kind 分支渲染: highlight/underline/strike 三种样式
- image 类型复杂（需截图+存储），本期不做，留 TODO

---

## 提议改动

### Part A: 修复重复通知（问题 1）

**A1. 修 DOI 归一化** — [src-tauri/src/services/paper.rs](file:///f:/learn/study/ai/trea/src-tauri/src/services/paper.rs)
- `insert_from_metadata` (行 464-485): `doi` 字段改为 `crate::duplicates::normalize_doi(&meta.doi)`
- `insert` (行 249): 入参 `paper.doi` 已归一化（调用方负责），但加防御性 `let doi = normalize_doi(&paper.doi);` 写入 DB
- `update` (行 346): 同上

**A2. 新增 scan_all 命令** — [src-tauri/src/commands/papers.rs](file:///f:/learn/study/ai/trea/src-tauri/src/commands/papers.rs) + [lib.rs](file:///f:/learn/study/ai/trea/src-tauri/src/lib.rs)
- 新增 `#[tauri::command] pub async fn scan_duplicates(state) -> AppResult<Vec<DuplicatePair>>`
- 调 `duplicates::scan_all(&vault)`
- lib.rs invoke_handler 注册

**A3. 前端主动拉取** — [src/App.tsx](file:///f:/learn/study/ai/trea/src/App.tsx)
- `checkVault` 成功后（vaultReady=true 之后）主动调 `api.scanDuplicates()`，若返回非空则 showToast
- 保留现有 listen 作为兜底（后端 setup spawn 仍保留）
- [src/lib/api.ts](file:///f:/learn/study/ai/trea/src/lib/api.ts) 加 `scanDuplicates`

### Part B: 修复 PDF 选区（问题 2）

**B1. AnnotationRect 支持多 rect** — [src/components/reader/PDFViewer.tsx](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx)
- `AnnotationRect` 改为 `rects: Array<{x,y,w,h}>`（或新增 `rects` 字段，保留 `rect` 兼容旧数据）
- `handleMouseUp` (行 243-279): 用 `range.getClientRects()` 取 DOMRectList，逐个归一化
- 高亮渲染 (行 424-440): `rects.map(r => <div style={...}/>)`

**B2. 后端 rect 字段兼容** — 无需改后端（rect 是 TEXT 存 JSON），前端解析时兼容两种格式：
- 若 `JSON.parse(rect)` 是数组 → 直接用
- 若是对象 `{x,y,w,h}` → 包装成 `[rect]`
- 写入时统一写数组

### Part C: Reader 布局调整（问题 3）

**C1. 比例调整** — [src/components/reader/ReaderShell.tsx](file:///f:/learn/study/ai/trea/src/components/reader/ReaderShell.tsx) 行 164-185
- PDF 区: `flex-1` → `flex-[2_1_0%] min-w-[400px]`
- 笔记区: `flex-1 min-w-[420px]` → `flex-[1_1_0%] min-w-[320px]`
- 批注栏: 保持 `w-[240px] shrink-0`

**C2. 可收起机制** — ReaderShell.tsx
- 新增 state: `const [notesCollapsed, setNotesCollapsed] = useState(false)`
- 新增 state: `const [annotationsCollapsed, setAnnotationsCollapsed] = useState(false)`
- 批注栏 section: `className={annotationsCollapsed ? "w-[40px] shrink-0" : "w-[240px] shrink-0"}` + 内部条件渲染
- 笔记区 section: `className={notesCollapsed ? "w-[40px] shrink-0" : "flex-[1_1_0%] min-w-[320px]"}` + 内部条件渲染
- 收起时显示一个竖条带展开按钮（ChevronLeft/Right 图标）
- 展开时在 section 顶部加收起按钮

### Part D: clippy 修复（问题 4）

**D1. 提取 require_vault** — 新建 [src-tauri/src/commands/common.rs](file:///f:/learn/study/ai/trea/src-tauri/src/commands/common.rs)
```rust
pub fn require_vault(state: &State<'_, AppState>) -> AppResult<std::path::PathBuf> {
    let guard = state.vault_path.read();
    guard.as_ref().cloned()
        .ok_or_else(|| AppError::Config("vault 未初始化".into()))
}
```
- commands/mod.rs 加 `pub mod common;`
- 7 个文件删掉本地 `require_vault`，改 `use crate::commands::common::require_vault;`

**D2. AnnotationInput 结构体** — [src-tauri/src/types.rs](file:///f:/learn/study/ai/trea/src-tauri/src/types.rs)
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationInput {
    pub kind: String,
    pub page: Option<i32>,
    pub rect: Option<String>,
    pub color: Option<String>,
    pub text: Option<String>,
    pub comment: Option<String>,
}
```
- [commands/annotation.rs:19](file:///f:/learn/study/ai/trea/src-tauri/src/commands/annotation.rs#L19): `create_annotation(state, paper_id, input: AnnotationInput)`
- [services/annotation.rs:41](file:///f:/learn/study/ai/trea/src-tauri/src/services/annotation.rs#L41): `create(vault, paper_id, input: &AnnotationInput)`
- [src/lib/api.ts](file:///f:/learn/study/ai/trea/src/lib/api.ts) + [PDFViewer.tsx](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx): 调用方改传对象

**D3. 其余 6 个杂项**
- [services/index.rs:85-87](file:///f:/learn/study/ai/trea/src-tauri/src/services/index.rs#L85): 改用结构体初始化语法
- [services/identifier.rs:6](file:///f:/learn/study/ai/trea/src-tauri/src/services/identifier.rs#L6): 文档注释 URL 缩进改 5 空格
- [services/identifier.rs:133](file:///f:/learn/study/ai/trea/src-tauri/src/services/identifier.rs#L133): `trim_end_matches(['.', ',', ';', ')'])`
- [services/identifier.rs:248](file:///f:/learn/study/ai/trea/src-tauri/src/services/identifier.rs#L248): `replace(['-', ' '], "")`
- [export/bibtex.rs:64](file:///f:/learn/study/ai/trea/src-tauri/src/export/bibtex.rs#L64): `push_str(" ")` → `push(' ')`
- [types.rs:51-55](file:///f:/learn/study/ai/trea/src-tauri/src/types.rs#L51): 删手动 impl，改 `#[derive(Default)]` + `#[default]` 标注 Unread

### Part E: TODO 8 — DOI 搜索归一化

**E1.** [src-tauri/src/services/search.rs](file:///f:/learn/study/ai/trea/src-tauri/src/services/search.rs)
- 顶部加 `use crate::duplicates::normalize_doi;`
- `search_structured` (行 114-119): `let nd = normalize_doi(d); if !nd.is_empty() { sql.push_str(" AND p.doi = ?"); args.push(Box::new(nd)); }`
- `search_both` (行 353): 同样处理
- 加测试: 搜索 `https://doi.org/10.1109/FOO` 能命中 `10.1109/foo`

### Part F: TODO 9 — FTS 自动同步

**F1.** [src-tauri/src/services/paper.rs](file:///f:/learn/study/ai/trea/src-tauri/src/services/paper.rs)
- `insert` (行 249): 事务内 INSERT papers 后调 `search::sync_papers_fts(&conn, &paper.id)?`
- `update` (行 346): 事务内 UPDATE 后调 `search::sync_papers_fts(&conn, id)?`
- `delete` (行 447): 事务内 `DELETE FROM papers_fts WHERE paper_id = ?1`
- 加 `use crate::services::search;`

**F2.** [src-tauri/src/commands/papers.rs](file:///f:/learn/study/ai/trea/src-tauri/src/commands/papers.rs)
- 删掉行 184、484 的手动 `reindex_paper` 调用（保留 reindex_paper 命令本身供 reindex_all）
- import_pdf / import_by_identifier 不再手动 reindex

**F3. 测试** — paper.rs 测试模块加测试: `insert` 后 `search_fulltext` 能命中

### Part G: TODO 10 — 批注 underline/strike（image 延后）

**G1.** [src/components/reader/PDFViewer.tsx](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx)
- 选区工具条 (行 446-460): 5 色按钮后加 underline/strike 两个按钮
- `handleCreateHighlight` 重构为 `handleCreateAnnotation(kind: "highlight"|"underline"|"strike", color?: string)`
- underline: `kind: "underline"`, color 固定或可选
- strike: `kind: "strike"`, color 固定红色或可选

**G2. 高亮覆盖层渲染** — PDFViewer.tsx 行 424-440
```tsx
{annotations.filter(...).map((a) => {
  const rects = parseRects(a.rect);  // 兼容旧单 rect
  return rects.map((r, i) => {
    const style = baseStyle(r);
    if (a.kind === "underline") style.borderBottom = `2px solid ${color}`;
    else if (a.kind === "strike") style.textDecoration = "line-through";
    else style.backgroundColor = colorRgba(a.color);  // highlight
    return <div key={`${a.id}-${i}`} style={style} />;
  });
})}
```

**G3.** [src-tauri/src/types.rs](file:///f:/learn/study/ai/trea/src-tauri/src/types.rs) Annotation 文档注释更新 kind 枚举说明

---

## 验证步骤

### 自动验证
```powershell
cd f:\learn\study\ai\trea\src-tauri
cargo clippy --lib --all-features   # 应 0 warning
cargo test --lib                    # 所有测试通过，含新测试
cd f:\learn\study\ai\trea
npx tsc --noEmit                    # 0 error
```

### 手动验证（pnpm tauri dev）

**问题 1 验证**:
- 导入两篇 DOI 相同（不同大小写/前缀）的论文 → 重启应用 → 应弹 toast「发现 1 组疑似重复」
- 或: 导入一篇后，再导入同一 PDF → 应弹 toast

**问题 2 验证**:
- 导入 PDF → 进入 Reader → 选中跨 3 行的文字 → 创建高亮
- 高亮应只在选中的文字行上显示，不再是巨大矩形
- 缩放 75%/150% 后高亮位置仍对齐

**问题 3 验证**:
- 进入 Reader → PDF 区明显比笔记区宽（约 2:1）
- 点批注栏收起按钮 → 批注栏缩为 40px 竖条 → PDF 区变大
- 点笔记区收起按钮 → 笔记区缩为 40px 竖条 → PDF 区占满
- 点竖条展开按钮 → 恢复

**TODO 8 验证**:
- 搜索框输入 `https://doi.org/10.xxx/FOO` → 应命中 DOI 为 `10.xxx/foo` 的论文

**TODO 9 验证**:
- 导入新论文 → 立即用全文搜索能搜到（无需手动 reindex）
- 更新论文标题 → 搜新标题能搜到
- 删除论文 → 搜不到

**TODO 10 验证**:
- 选中文字 → 点 underline 按钮 → 下划线高亮
- 选中文字 → 点 strike 按钮 → 删除线高亮
- 侧边栏显示对应 kind 图标

---

## 执行顺序

1. **Part D (clippy)** — 独立、低风险，先做
2. **Part A (重复通知)** — DOI 归一化 + 新命令 + 前端拉取
3. **Part E (TODO 8)** — DOI 搜索归一化（与 A1 同源，紧接做）
4. **Part F (TODO 9)** — FTS 自动同步
5. **Part B (选区)** — 前端改 getClientRects
6. **Part C (Reader 布局)** — 前端三栏可收起
7. **Part G (TODO 10)** — 批注 underline/strike

每完成一个 Part 跑 `cargo test --lib` + `npx tsc --noEmit`，最后整体 clippy 验证 + commit。

---

## 不在本期范围

- TODO 10 的 image 类型批注（需截图+存储，单独排期）
- TopBar 搜索结果展示（TODO 6，另议）
- pnpm-lock.yaml 确认（TODO 7，用户已自行验证或后续）
