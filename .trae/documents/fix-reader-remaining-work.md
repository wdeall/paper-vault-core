# 计划：Reader 修复 + TODO 8/9/10 剩余工作

> 日期: 2026-06-30
> 范围: 上一轮计划（fix-reader-and-followup-todos.md）的剩余部分
> 前置状态: Part D (clippy) / A1 (DOI 归一化) / A2 (scan_duplicates 命令) / E (TODO 8) / F (TODO 9 已实现) 均已完成

---

## 总结

上一轮计划 7 个 Part 中已完成 5 个（D/A1/A2/E/F），剩余 4 项：

1. **Part A3** — 前端主动拉取重复扫描（api.ts + App.tsx）
2. **Part B** — PDF 选区改 `getClientRects()` + 多 rect 渲染（PDFViewer.tsx + types.ts + api.ts）
3. **Part C** — Reader 三栏可收起 + 比例调整（ReaderShell.tsx）
4. **Part G** — 批注 underline/strike（PDFViewer.tsx）

---

## 当前状态分析

### Part A3: 前端未主动拉取
- [src/lib/api.ts](file:///f:/learn/study/ai/trea/src/lib/api.ts) 无 `scanDuplicates` 方法
- [src/App.tsx:51-69](file:///f:/learn/study/ai/trea/src/App.tsx#L51-L69) `checkVault` 成功后只调 `loadPapers()`，未调扫描
- 后端 `scan_duplicates` 命令已在 [lib.rs:114](file:///f:/learn/study/ai/trea/src-tauri/src/lib.rs#L114) 注册
- 现有 `listen("duplicates-found")` 保留作为兜底

### Part B: PDF 选区单 rect 导致跨行高亮过大
- [PDFViewer.tsx:262](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx#L262) `range.getBoundingClientRect()` 返回**并集矩形**（跨行时含中间空白）
- [PDFViewer.tsx:424-440](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx#L424-L440) 高亮覆盖层用单个 `a.rect` 渲染单个 div
- [types/index.ts:182-187](file:///f:/learn/study/ai/trea/src/types/index.ts#L182-L187) `AnnotationRect = {x,y,w,h}`，`Annotation.rect: AnnotationRect | null`
- [api.ts:87-109](file:///f:/learn/study/ai/trea/src/lib/api.ts#L87-L109) `parseAnnotation` 直接 `JSON.parse` 为单对象
- [AnnotationSidebar.tsx:78-79](file:///f:/learn/study/ai/trea/src/components/reader/AnnotationSidebar.tsx#L78-L79) `a.rect` 传给 `onJumpToAnnotation`
- [ReaderShell.tsx:109-111](file:///f:/learn/study/ai/trea/src/components/reader/ReaderShell.tsx#L109-L111) `handleJumpToAnnotation` 实际只用 page，忽略 rect

### Part C: Reader 布局失衡 + 无收起机制
- [ReaderShell.tsx:164](file:///f:/learn/study/ai/trea/src/components/reader/ReaderShell.tsx#L164) PDF 区 `flex-1`（无 min-width，被挤压）
- [ReaderShell.tsx:176](file:///f:/learn/study/ai/trea/src/components/reader/ReaderShell.tsx#L176) 批注栏 `w-[240px] shrink-0`（固定，无收起）
- [ReaderShell.tsx:185](file:///f:/learn/study/ai/trea/src/components/reader/ReaderShell.tsx#L185) 笔记区 `flex-1 min-w-[420px]`（min-width 过大，挤压 PDF）
- 无任何 `notesCollapsed` / `annotationsCollapsed` 状态

### Part G: 批注只支持 highlight/note
- [PDFViewer.tsx:291-316](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx#L291-L316) `handleCreateHighlight(color)` 固定 `kind: "highlight"`
- [PDFViewer.tsx:318-342](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx#L318-L342) `handleCreateNote` 固定 `kind: "note"`
- [PDFViewer.tsx:456-468](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx#L456-L468) 工具条只有 5 色按钮 + 评论按钮，无 underline/strike
- [types.rs:161](file:///f:/learn/study/ai/trea/src-tauri/src/types.rs#L161) Annotation kind 文档已含 underline/strike/image（无需改）
- [types/index.ts:193](file:///f:/learn/study/ai/trea/src/types/index.ts#L193) Annotation kind 注释已含 underline/strike（无需改）

---

## 假设与决策

### 决策 1: Part B — `Annotation.rect` 改为数组类型
- `AnnotationRect` 保持 `{x,y,w,h}` 不变
- `Annotation.rect` 类型从 `AnnotationRect | null` 改为 `AnnotationRect[] | null`
- 后端 `rect` 列仍为 TEXT（JSON 字符串），内容从单对象改为数组
- `parseAnnotation` 兼容旧数据：`JSON.parse` 若为对象则包装成 `[rect]`，若为数组则直接用
- `createAnnotation` / `updateAnnotation` 序列化时写 JSON 数组字符串
- `AnnotationSidebar.onJumpToAnnotation` 传 `a.rect[0]`（第一个 rect，ReaderShell 反正只用 page）
- 高亮覆盖层 `rects.map(r => <div/>)` 渲染多个 div

### 决策 2: Part C — 收起机制用 40px 竖条
- 收起时 section 宽度变 `w-[40px] shrink-0`，内部只显示一个展开按钮（ChevronLeft/Right）
- 展开时 section 顶部 header 加收起按钮
- PDF 区不收起（始终展示）
- 比例：PDF `flex-[2_1_0%] min-w-[400px]`，笔记 `flex-[1_1_0%] min-w-[320px]`

### 决策 3: Part G — underline/strike 渲染为细线
- highlight: `backgroundColor: rgba(color, 0.35)`（现有）
- underline: 高度 2px 的实色线，定位在 rect 底部
- strike: 高度 2px 的实色线，定位在 rect 垂直中点
- 需新增 `colorSolid(name)` 返回不透明色（如 `rgb(250, 204, 21)`）
- 工具条：5 色按钮后加分隔线，再加 underline (U 图标) 和 strike (S 图标) 两个按钮
- underline/stroke 用当前选中的颜色（默认 yellow）；若未选色则用 yellow

### 决策 4: Part A3 — 前端拉取失败静默
- `api.scanDuplicates()` 调用失败时只 `console.error`，不弹 toast（避免启动时报错干扰）
- 成功且非空时弹 warning toast（与现有 listen 一致）
- 保留现有 `listen("duplicates-found")` 作为兜底

---

## 提议改动

### Part A3: 前端主动拉取

**A3.1** — [src/lib/api.ts](file:///f:/learn/study/ai/trea/src/lib/api.ts)
- 在 `deleteAnnotation` / `syncAnnotationsToNote` 附近加：
```typescript
scanDuplicates: () => call<DuplicatePair[]>("scan_duplicates"),
```
- 需在文件顶部 import 或定义 `DuplicatePair` 类型（与 App.tsx 现有定义对齐）
- 为避免重复，把 App.tsx 的 `DuplicatePair` interface 提取到 `types/index.ts`

**A3.2** — [src/types/index.ts](file:///f:/learn/study/ai/trea/src/types/index.ts)
- 文件末尾加：
```typescript
/** 后端 scan_all 返回的重复对（与 Rust duplicates::DuplicatePair 对齐）。 */
export interface DuplicatePair {
  paper_id_a: string;
  title_a: string;
  paper_id_b: string;
  title_b: string;
  reason: string;
  confidence: string;
}
```

**A3.3** — [src/App.tsx](file:///f:/learn/study/ai/trea/src/App.tsx)
- 删掉本地 `DuplicatePair` interface（改用 `@/types` 导入）
- `checkVault` 中 `setVaultReady(true)` 后加：
```typescript
// 主动拉取一次重复扫描（避免后端 setup emit 早于前端 listen 注册的时序竞态）
void api.scanDuplicates().then((pairs) => {
  if (pairs.length > 0) {
    showToast("warning", `发现 ${pairs.length} 组疑似重复论文，建议在设置中检查并合并`, { ttlSec: 10 });
  }
}).catch((e) => {
  console.error("scan duplicates", e);
});
```

### Part B: PDF 选区多 rect

**B1.** — [src/types/index.ts](file:///f:/learn/study/ai/trea/src/types/index.ts#L189-L201)
- `Annotation.rect` 类型改为 `AnnotationRect[] | null`

**B2.** — [src/lib/api.ts](file:///f:/learn/study/ai/trea/src/lib/api.ts#L87-L109)
- `parseAnnotation` 改为：
```typescript
function parseAnnotation(raw: RawAnnotation): Annotation {
  let rects: AnnotationRect[] | null = null;
  if (raw.rect) {
    try {
      const parsed = JSON.parse(raw.rect);
      if (Array.isArray(parsed)) {
        rects = parsed as AnnotationRect[];
      } else if (parsed && typeof parsed === "object") {
        // 兼容旧单 rect 数据
        rects = [parsed as AnnotationRect];
      }
    } catch {
      rects = null;
    }
  }
  return { /* ... */ rect: rects, /* ... */ };
}
```
- `createAnnotation` 入参 `rect` 类型改为 `AnnotationRect[] | null`，序列化 `JSON.stringify(params.rect)`
- `updateAnnotation` 同理

**B3.** — [src/components/reader/PDFViewer.tsx](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx#L40-L45)
- `SelectionState.rect` 类型改为 `AnnotationRect[]`
- `handleMouseUp` (行 243-279) 改用 `range.getClientRects()`：
```typescript
const rects = Array.from(range.getClientRects());
if (rects.length === 0) return;
const layerRect = textLayer.getBoundingClientRect();
const normRects: AnnotationRect[] = rects
  .filter((r) => r.width > 0 && r.height > 0)
  .map((r) => ({
    x: (r.left - layerRect.left) / layerRect.width,
    y: (r.top - layerRect.top) / layerRect.height,
    w: r.width / layerRect.width,
    h: r.height / layerRect.height,
  }));
if (normRects.length === 0) return;
// toolbar 定位用第一个 rect
const first = rects[0];
const x = first.left - layerRect.left + first.width / 2;
const y = first.top - layerRect.top;
setSelectionState({ text, rect: normRects, x, y });
```
- `handleCreateHighlight` (行 291-316) / `handleCreateNote` (行 318-342): `rect: selectionState.rect` 不变（类型已改为数组）
- 高亮覆盖层 (行 424-440) 改为 map rects：
```tsx
{annotations
  .filter((a) => a.page === currentPage && a.rect && a.rect.length > 0)
  .flatMap((a) =>
    a.rect!.map((r, i) => (
      <div
        key={`${a.id}-${i}`}
        className="absolute"
        style={renderStyle(a.kind, r, a.color)}
      />
    ))
  )}
```
- 新增 `renderStyle(kind, r, color)` 辅助函数（见 Part G）

**B4.** — [src/components/reader/AnnotationSidebar.tsx](file:///f:/learn/study/ai/trea/src/components/reader/AnnotationSidebar.tsx#L77-L81)
- `handleClickAnnotation` 改为传第一个 rect：
```typescript
function handleClickAnnotation(a: Annotation) {
  if (a.page != null && a.rect && a.rect.length > 0) {
    onJumpToAnnotation(a.page, a.rect[0]);
  }
}
```

### Part C: Reader 布局可收起

**C1.** — [src/components/reader/ReaderShell.tsx](file:///f:/learn/study/ai/trea/src/components/reader/ReaderShell.tsx)
- 顶部 import 加 `PanelRightClose, PanelLeftClose, PanelRightOpen, PanelLeftOpen` from lucide-react
- 新增 state：
```typescript
const [notesCollapsed, setNotesCollapsed] = useState(false);
const [annotationsCollapsed, setAnnotationsCollapsed] = useState(false);
```
- PDF 区 (行 164)：`className="flex-1 border-r border-border bg-muted/30"` → `className="flex-[2_1_0%] min-w-[400px] border-r border-border bg-muted/30"`
- 批注栏 (行 176-183)：
```tsx
<section className={annotationsCollapsed ? "w-[40px] shrink-0 border-l border-border" : "w-[240px] shrink-0 border-l border-border"}>
  {annotationsCollapsed ? (
    <button
      className="flex h-full w-full flex-col items-center justify-center text-muted-foreground hover:text-foreground"
      onClick={() => setAnnotationsCollapsed(false)}
      title="展开批注栏"
    >
      <PanelRightOpen className="h-4 w-4" />
    </button>
  ) : (
    <div className="flex h-full flex-col">
      <div className="flex h-8 shrink-0 items-center justify-between border-b border-border px-2">
        <span className="text-xs font-medium">批注</span>
        <Button size="icon" variant="ghost" className="h-6 w-6" onClick={() => setAnnotationsCollapsed(true)} title="收起批注栏">
          <PanelRightClose className="h-3.5 w-3.5" />
        </Button>
      </div>
      <div className="flex-1 overflow-hidden">
        <AnnotationSidebar ... />
      </div>
    </div>
  )}
</section>
```
- 笔记区 (行 185-200)：
```tsx
<section className={notesCollapsed ? "w-[40px] shrink-0 border-l border-border" : "flex-[1_1_0%] min-w-[320px] border-l border-border"}>
  {notesCollapsed ? (
    <button
      className="flex h-full w-full flex-col items-center justify-center text-muted-foreground hover:text-foreground"
      onClick={() => setNotesCollapsed(false)}
      title="展开笔记"
    >
      <PanelLeftOpen className="h-4 w-4" />
    </button>
  ) : (
    <div className="flex h-full flex-col">
      <div className="flex h-8 shrink-0 items-center justify-between border-b border-border px-2">
        <span className="text-xs font-medium">笔记</span>
        <Button size="icon" variant="ghost" className="h-6 w-6" onClick={() => setNotesCollapsed(true)} title="收起笔记">
          <PanelLeftClose className="h-3.5 w-3.5" />
        </Button>
      </div>
      <div className="flex-1 overflow-y-auto">
        {hasNote ? <NoteEditor paperId={paperId} /> : <CreateNotePlaceholder />}
      </div>
    </div>
  )}
</section>
```

### Part G: 批注 underline/strike

**G1.** — [src/components/reader/PDFViewer.tsx](file:///f:/learn/study/ai/trea/src/components/reader/PDFViewer.tsx)
- COLORS 数组旁加 SOLID_COLORS 映射（不透明色）：
```typescript
const SOLID_COLORS: Record<string, string> = {
  yellow: "rgb(250, 204, 21)",
  red: "rgb(248, 113, 113)",
  green: "rgb(74, 222, 128)",
  blue: "rgb(96, 165, 250)",
  purple: "rgb(192, 132, 252)",
};
function colorSolid(name: string | null): string {
  return SOLID_COLORS[name ?? "yellow"] ?? SOLID_COLORS.yellow;
}
```
- 新增 `renderStyle(kind: string, r: AnnotationRect, color: string | null): React.CSSProperties`：
```typescript
function renderStyle(kind: string, r: AnnotationRect, color: string | null): React.CSSProperties {
  const base: React.CSSProperties = {
    left: `${r.x * 100}%`,
    top: `${r.y * 100}%`,
    width: `${r.w * 100}%`,
  };
  if (kind === "underline") {
    return { ...base, height: "2px", top: `${(r.y + r.h) * 100}%`, backgroundColor: colorSolid(color) };
  }
  if (kind === "strike") {
    return { ...base, height: "2px", top: `${(r.y + r.h / 2) * 100}%`, backgroundColor: colorSolid(color) };
  }
  // highlight (默认，含 note)
  return { ...base, height: `${r.h * 100}%`, backgroundColor: colorRgba(color) };
}
```
- 工具条 (行 456-480) 5 色按钮后加分隔线 + underline/strike 按钮：
```tsx
<div className="mx-1 h-4 w-px bg-border" />
<Button
  size="icon"
  variant="ghost"
  className="h-6 w-6"
  disabled={creating}
  onClick={() => handleCreateAnnotation("underline")}
  title="下划线"
>
  <Underline className="h-3.5 w-3.5" />
</Button>
<Button
  size="icon"
  variant="ghost"
  className="h-6 w-6"
  disabled={creating}
  onClick={() => handleCreateAnnotation("strike")}
  title="删除线"
>
  <Strikethrough className="h-3.5 w-3.5" />
</Button>
```
- import 加 `Underline, Strikethrough` from lucide-react
- 新增 `handleCreateAnnotation(kind: "underline" | "strike")`：
```typescript
const handleCreateAnnotation = useCallback(
  async (kind: "underline" | "strike") => {
    if (!selectionState) return;
    setCreating(true);
    try {
      await api.createAnnotation({
        paperId,
        kind,
        page: currentPage,
        rect: selectionState.rect,
        color: "yellow",  // underline/strike 默认黄色
        text: selectionState.text,
        comment: null,
      });
      showToast("success", kind === "underline" ? "已添加下划线" : "已添加删除线");
      window.getSelection()?.removeAllRanges();
      setSelectionState(null);
      onAnnotationChangeRef.current();
    } catch (e) {
      showToast("error", `添加失败: ${(e as Error).message}`);
    } finally {
      setCreating(false);
    }
  },
  [selectionState, paperId, currentPage, showToast],
);
```

**G2.** — [src/components/reader/AnnotationSidebar.tsx](file:///f:/learn/study/ai/trea/src/components/reader/AnnotationSidebar.tsx#L194-L201)
- 批注列表头部颜色点旁加 kind 图标（可选，低优先）：
```tsx
{a.kind === "underline" && <Underline className="h-3 w-3" />}
{a.kind === "strike" && <Strikethrough className="h-3 w-3" />}
{a.kind === "highlight" && <span className={cn("h-2.5 w-2.5 rounded-full", a.color ? COLOR_DOT[a.color] : "bg-muted")} />}
```
- import 加 `Underline, Strikethrough` from lucide-react

---

## 验证步骤

### 自动验证
```powershell
cd f:\learn\study\ai\trea\src-tauri
& "C:\Users\业天\.cargo\bin\cargo.exe" clippy --lib --all-features   # 应 0 warning
& "C:\Users\业天\.cargo\bin\cargo.exe" test --lib                    # 所有测试通过
cd f:\learn\study\ai\trea
npx tsc --noEmit                    # 0 error
```

### 手动验证（pnpm tauri dev）

**Part A3 验证**:
- 导入两篇 DOI 相同（不同大小写/前缀）的论文 → 重启应用 → 应弹 toast「发现 N 组疑似重复」
- 或: 导入一篇后，再导入同一 PDF → 应弹 toast

**Part B 验证**:
- 导入 PDF → 进入 Reader → 选中跨 3 行的文字 → 创建高亮
- 高亮应只在选中的文字行上显示，不再是巨大矩形
- 缩放 75%/150% 后高亮位置仍对齐

**Part C 验证**:
- 进入 Reader → PDF 区明显比笔记区宽（约 2:1）
- 点批注栏收起按钮 → 批注栏缩为 40px 竖条 → PDF 区变大
- 点笔记区收起按钮 → 笔记区缩为 40px 竖条 → PDF 区占满
- 点竖条展开按钮 → 恢复

**Part G 验证**:
- 选中文字 → 点 underline 按钮 → 下划线高亮（2px 黄线在文字底部）
- 选中文字 → 点 strike 按钮 → 删除线高亮（2px 黄线在文字中间）
- 侧边栏显示对应 kind 图标

---

## 执行顺序

1. **Part A3** — api.ts + types/index.ts + App.tsx（前端拉取，独立）
2. **Part B** — types/index.ts + api.ts + PDFViewer.tsx + AnnotationSidebar.tsx（多 rect，涉及面广）
3. **Part G** — PDFViewer.tsx + AnnotationSidebar.tsx（underline/strike，依赖 B 的 renderStyle）
4. **Part C** — ReaderShell.tsx（布局，独立）
5. **验证** — cargo clippy + cargo test + npx tsc + commit

Part B 和 G 都改 PDFViewer.tsx，紧接做避免冲突。

---

## 不在本期范围

- TODO 10 的 image 类型批注（需截图+存储，单独排期）
- 拖拽调整三栏宽度（本期只用固定比例 + 收起）
