# 删除批注功能 + PDF 自适应容器宽度

## 背景

用户反馈批注功能问题过多,决定直接删除。同时 PDF 阅读区扩大后页面无法占满容器,需要去掉 padding 并让 PDF 自适应容器宽度。

## 决策

1. **删除范围**: 只删批注(highlight/underline/strike/note 批注 + AnnotationSidebar),**保留笔记**(NoteEditor)
2. **PDF 占满**: 去掉 `p-4` padding + 加自适应容器宽度(初始加载按容器宽度算 scale,resize 时重算)
3. **后端不动**: 保留 `annotations` 表 + 批注命令(数据库已有数据不丢,前端不再调用)
4. **前端 API/类型保留**: `api.ts` 的批注方法和 `types/index.ts` 的 Annotation 类型不删(避免破坏其他潜在引用,虽然实际已无调用方)

## 实施步骤

### 步骤 1:删除 AnnotationSidebar.tsx

```
删除文件: f:\learn\study\ai\trea\src\components\reader\AnnotationSidebar.tsx
```

### 步骤 2:重构 PDFViewer.tsx

文件: `f:\learn\study\ai\trea\src\components\reader\PDFViewer.tsx`

**2.1 删除批注相关代码**
- 删除 import: `MessageSquare`, `Underline`, `Strikethrough`
- 删除常量: `COLORS`, `SOLID_COLORS`, `colorRgba()`, `colorSolid()`, `renderStyle()`
- 删除 interface `SelectionState`
- 删除 Props 中的 `annotations` 和 `onAnnotationChange`
- 删除 state: `selectionState`, `commentMode`, `commentText`, `creating`
- 删除 ref: `textLayerRef`, `textLayerInstanceRef`
- 删除 effect: 选区监听 `handleMouseUp`
- 删除 effect: 翻页/缩放时清除选区
- 删除回调: `handleCreateHighlight`, `handleCreateNote`, `handleCreateAnnotation`
- 删除 JSX: 批注高亮覆盖层、文本层 div、选区 toolbar、评论输入框
- 删除渲染 effect 中的文本层渲染逻辑

**2.2 去掉 padding**
- `<div className="flex min-h-full items-start justify-center p-4">` → 去掉 `p-4`

**2.3 加自适应容器宽度**
- 新增 ref: `containerRef`(指向 PDF 渲染区 `<div className="relative flex-1 overflow-auto">`)
- 新增 state: `containerWidth`(初始 0)
- 新增 effect: `ResizeObserver` 监听 containerRef 宽度变化,更新 `containerWidth`
- 修改渲染逻辑: 当 `containerWidth > 0` 且未手动 zoom 时,scale = `containerWidth / viewport.width`(留 0px margin,完全占满)
- 保留手动 zoom 按钮:用户点击 zoom 后切换为手动模式(`userZoom` state),不再自适应;双击 zoom 按钮或加"适配"按钮恢复自适应
- 简化方案: 加一个"适配宽度"按钮(`Maximize` 图标),点击后 `setFitWidth(true)`;`fitWidth` 为 true 时 scale 跟随 containerWidth;手动 zoom 后 `setFitWidth(false)`

**Props 最终签名**:
```typescript
interface Props {
  paperId: string;
  src: string;
  initialPage: number;
  onPageChange: (page: number) => void;
  onTotalPages: (n: number) => void;
}
```

**自适应核心逻辑**:
```typescript
const containerRef = useRef<HTMLDivElement | null>(null);
const [containerWidth, setContainerWidth] = useState(0);
const [fitWidth, setFitWidth] = useState(true); // 默认自适应
const pageRef = useRef<{ width: number; height: number } | null>(null);

// ResizeObserver
useEffect(() => {
  const el = containerRef.current;
  if (!el) return;
  const ro = new ResizeObserver((entries) => {
    for (const e of entries) {
      setContainerWidth(e.contentRect.width);
    }
  });
  ro.observe(el);
  return () => ro.disconnect();
}, []);

// 渲染时计算 scale
const effectiveScale = (() => {
  if (fitWidth && containerWidth > 0 && pageRef.current) {
    return containerWidth / pageRef.current.width;
  }
  return zoom / 100;
})();

// 渲染 effect 依赖改为 [docReady, currentPage, effectiveScale]
// 获取 page 后记录原始尺寸:
// const baseViewport = page.getViewport({ scale: 1 });
// pageRef.current = { width: baseViewport.width, height: baseViewport.height };
// const viewport = page.getViewport({ scale: effectiveScale });
```

**工具栏新增"适配宽度"按钮**:
- 在 zoom 按钮后加 `<Button onClick={() => setFitWidth(true)}>` 用 `Maximize` 图标
- fitWidth 为 true 时该按钮高亮(variant="default"),false 时 ghost

### 步骤 3:重构 ReaderShell.tsx

文件: `f:\learn\study\ai\trea\src\components\reader\ReaderShell.tsx`

**3.1 删除批注相关代码**
- 删除 import: `AnnotationSidebar`, `PanelRightClose`, `PanelRightOpen`(批注栏收起按钮)
- 删除 state: `annotations`, `annotationVersion`, `annotationsCollapsed`
- 删除 effect: `listAnnotations` 调用
- 删除回调: `handleAnnotationChange`, `handleJumpToAnnotation`
- 删除 JSX: 整个批注栏 `<section>`(annotationsCollapsed 三元 + AnnotationSidebar)
- PDFViewer 的 props 去掉 `annotations` 和 `onAnnotationChange`

**3.2 调整布局为两栏**
```tsx
<div className="flex flex-1 overflow-hidden">
  {/* PDF 阅读区 */}
  <section className="flex-[3_1_0%] min-w-[400px] border-r border-border bg-muted/30">
    <PDFViewer
      paperId={paperId}
      src={detail.pdf_path}
      initialPage={currentPage}
      onPageChange={handlePageChange}
      onTotalPages={handleTotalPages}
    />
  </section>
  {/* 笔记编辑区（可收起） */}
  <section className={notesCollapsed ? "w-[40px]..." : "flex-[1_1_0%] min-w-[320px]..."}>
    ...
  </section>
</div>
```

PDF 区权重从 `flex-[2_1_0%]` 改为 `flex-[3_1_0%]`(批注栏删除后 PDF 可以更宽)。

### 步骤 4:验证

```powershell
cd f:\learn\study\ai\trea
npx tsc --noEmit  # 0 error
npm run lint       # 0 warning
```

### 步骤 5:Git commit

```powershell
git add src/components/reader/PDFViewer.tsx src/components/reader/ReaderShell.tsx
git rm src/components/reader/AnnotationSidebar.tsx
git commit -m "refactor: 删除 PDF 批注功能并让 PDF 自适应容器宽度

- 删除 AnnotationSidebar 组件(批注侧栏)
- PDFViewer: 删除选区/高亮/underline/strike/评论工具条 + 文本层
- PDFViewer: 去掉 p-4 padding,加 ResizeObserver 自适应容器宽度
- PDFViewer: 新增'适配宽度'按钮,手动 zoom 后切换为固定 scale
- ReaderShell: 三栏改两栏(PDF flex-[3_1_0%] + 笔记),删除批注栏
- 保留笔记功能(NoteEditor)和后端批注 API/表"
```

## 假设与决策

1. **不删后端批注代码** — `annotations` 表 + `commands/annotation.rs` + `services/annotation.rs` 保留,数据库已有批注数据不丢失,未来若恢复前端批注可直接复用
2. **不删 api.ts 批注方法** — 避免破坏潜在引用,虽然实际已无调用方(可用 `npm run lint` 验证)
3. **不删 types/index.ts Annotation 类型** — 同上
4. **PDF 自适应默认开启** — `fitWidth` 初始 true,首次加载即占满容器;手动 zoom 后关闭自适应
5. **保留文本层** — 虽然不再做选区批注,但文本层让用户可以复制文字(实际上删除文本层会更简单,但保留可复制能力更有用)→ **决策:删除文本层**,因为用户说"问题过多直接删除",且文本层是批注选区的基础,删掉更干净。用户若需复制可用系统 PDF 阅读器(ExternalLink 按钮)

## 验证清单

- [ ] `npx tsc --noEmit` 0 error
- [ ] `npm run lint` 0 warning
- [ ] 启动应用,PDF 占满阅读区宽度(无左右灰色空白)
- [ ] 窗口缩放时 PDF 自动重新适配宽度
- [ ] 点击"适配宽度"按钮恢复自适应
- [ ] 手动 zoom +/- 后 PDF 不再自适应(固定 scale)
- [ ] 阅读界面只有两栏(PDF + 笔记),无批注栏
- [ ] 笔记栏可收起/展开
- [ ] PDF 上无任何高亮/工具条/选区交互
