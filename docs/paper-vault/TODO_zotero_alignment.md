# TODO — Zotero 对齐项目待办事宜

> 日期: 2026-06-22
> 优先级: 高 > 中 > 低

---

## 高优先级 (需尽快处理)

### 1. ✅ 启动后台重复扫描 + 通知 (SPEC §7.3 缺口) [已完成 commit c1034c0]

**已实现**:
- `duplicates.rs`: 新增 `scan_all()` 全库扫描 (DOI + 标题归一化) + `DuplicatePair` 结构 + 3 测试
- `lib.rs`: setup() 中 `tauri::async_runtime::spawn` 异步扫描 + `app.emit("duplicates-found", &pairs)`
- `App.tsx`: `listen<DuplicatePair[]>("duplicates-found")` + toast 通知 (ttlSec 10)
- `stores/ui.ts`: `showToast` options `label`/`onClick` 改可选 (支持纯提示场景)
- 测试: cargo test 110 passed

### 2. ✅ SearchPanel 接入路由 (M-C 遗留) [已完成 commit c1034c0]

**已实现**:
- `stores/ui.ts`: 新增 `searchPanelOpen` / `toggleSearchPanel` / `setSearchPanelOpen`
- `TopBar.tsx`: 加 `SlidersHorizontal` 切换按钮 (variant 随状态切换) + `handleSearch` 改为打开面板
- `LibraryShell.tsx`: `{searchPanelOpen && <SearchPanel />}` 条件渲染
- 测试: tsc pass

### 3. 手动验收 P4 PDF 批注

**需要验证**:
- pdf.js 选区 → 5 色批注创建流程
- bbox 跨缩放稳定性 (不同 zoom 下位置一致)
- 批注高亮覆盖层正确渲染
- 侧边栏编辑/删除/同步到笔记

**操作指引**: `pnpm tauri dev` 启动应用,导入一篇 PDF 论文,进入 Reader 选中文本创建批注。

### 4. 手动验收 P5 导出格式

**需要验证**:
- BibTeX 导入 Zotero 显示正确
- RIS 导入 EndNote/Mendeley 显示正确
- CSL-JSON 通过 Pandoc 引用正确

**操作指引**:
```bash
# 导出 BibTeX
# 在应用里选中论文 → 导出 BibTeX → 保存 .bib → Zotero 导入

# 导出 RIS
# 同上 → 保存 .ris → EndNote/Mendeley 导入

# CSL-JSON + Pandoc
# 导出 CSL-JSON → 保存 .json
# pandoc --csl=ieee.csl --bibliography=papers.json input.md -o output.pdf
```

---

## 中优先级

### 5. cargo clippy 完整验证

**现状**: sandbox 环境下 tauri-build build script panic (Os code 0),无法运行 clippy。
**需要做**: 在非沙盒环境 (本机 PowerShell) 跑 `cargo clippy --lib` 确认无 lint 警告。

**操作指引**:
```powershell
cd f:\learn\study\ai\trea\src-tauri
cargo clippy --lib
```

### 6. TopBar 搜索行为验证

**现状**: TopBar 搜索改为跳转 `/library` + 固定 fulltext 模式,但 PaperListPane 只显示命中条数提示。
**需要做**: 验证搜索结果是否正确展示,或调整为弹出 SearchResults 弹窗。

### 7. package.json / pnpm-lock.yaml 确认

**现状**: pdfjs-dist 已在 package.json (M-D commit),但需确认 pnpm-lock.yaml 同步。
**操作指引**:
```powershell
cd f:\learn\study\ai\trea
git diff HEAD -- pnpm-lock.yaml
# 如果有差异,pnpm install 重新生成
```

---

## 低优先级 (后续迭代)

### 8. DOI 规范化精确匹配 (SPEC §3.3.4)

**现状**: search_structured 的 doi 字段直接比较,未做规范化 (大小写/前缀)。
**需要做**: 搜索时先调 `identifier::normalize_doi()` 再比较。

### 9. FTS 索引 trigger (SPEC §8 风险缓解)

**现状**: paper 创建/更新后需手动调 `reindex_paper`。
**需要做**: 可选加 SQLite trigger 自动同步 papers_fts,或 paper::create/update 里自动调 sync_papers_fts。

### 10. 批注 kind 扩展

**现状**: 支持 highlight / note 两种 kind。
**需要做**: SPEC 提到 underline / strike / image,可后续扩展。
