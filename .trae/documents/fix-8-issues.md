# 修复 8 个验收问题(库分类/元数据保存/DOI/AI 总结/系统提示词/AI 对话/复现计划)

## 背景

用户反馈 8 个问题,经代码探索确认根因后制定本计划。

## 问题与根因汇总

| # | 问题 | 根因 | 修复位置 |
|---|------|------|----------|
| 1 | 左侧阅读状态分类无法点击 | `paper.ts:83-84` 两个 setter 互相覆盖对方字段 | `src/stores/paper.ts` |
| 2 | 手动修改元数据切换后重置 | 前端 `detail` 是本地 useState,切换 paperId 时 useEffect 覆盖;无未保存提示 | `PaperDetailPane.tsx` |
| 3 | 自动读取元数据没有 DOI | `pdf.rs:30` DOI 正则只允许大写 A-Z,小写 DOI 被截断 | `src-tauri/src/pdf.rs` |
| 4 | 总结报错 os error 3 | `paper.pdf_path` 存绝对路径,vault 移动后路径找不到 | `ai_svc.rs` + `vault.rs` |
| 5 | 添加系统提示词强调论文助手身份 | `ai_svc.rs:94-103` system prompt 直接用 preset 的,无统一前缀 | `ai_svc.rs` |
| 6 | 总结只输入前 6000 字 | `ai_svc.rs:84` 硬编码 `.chars().take(6000)` | `ai_svc.rs` + `presets.rs` |
| 7 | 添加 AI 对话功能 | 当前只有 preset 单次运行,无对话 | 新增 Chat 组件 + 后端命令 |
| 8 | AI 快捷功能添加复现实验计划 | 无此 preset | `presets.rs` + `AIPanel.tsx` |

## 实施步骤

### 步骤 1:修复状态分类点击(问题 1)

**文件**: `src/stores/paper.ts:83-84`

**改动**: setter 不再互相清空对方字段,改为只管理自己的字段。CollectionsPane 调用方已同时调用两个 setter,所以清空逻辑由调用方负责。

```typescript
// 改前
setStatusFilter: (s) => set({ statusFilter: s, activeCollectionId: null }),
setActiveCollection: (id) => set({ activeCollectionId: id, statusFilter: null }),

// 改后
setStatusFilter: (s) => set({ statusFilter: s }),
setActiveCollection: (id) => set({ activeCollectionId: id }),
```

同时修改 `setSmartView` 保持一致(它已经清空另外两个字段,保持不变)。

**CollectionsPane.tsx 不需要改** — 调用方已同时调用 `setStatusFilter(opt.value)` + `setActiveCollection(null)`,改后正好实现"设置 status + 清空 collection"。

### 步骤 2:修复元数据保存(问题 2)

**文件**: `src/components/library/PaperDetailPane.tsx`

**改动 A**: 加 dirty 标记 + 切换前确认

```typescript
const [detail, setDetail] = useState<PaperDetail | null>(null);
const [savedDetail, setSavedDetail] = useState<PaperDetail | null>(null); // 最近一次保存的快照
const isDirty = detail && savedDetail ? JSON.stringify(detail) !== JSON.stringify(savedDetail) : false;

useEffect(() => {
  setLoading(true);
  setCandidate(null);
  getPaper(paperId)
    .then((d) => {
      setDetail(d);
      setSavedDetail(d); // 同步快照
    })
    .catch((e) => showToast("error", `加载失败: ${(e as Error).message}`))
    .finally(() => setLoading(false));
}, [paperId, getPaper, showToast]);

async function handleSave() {
  if (!detail) return;
  setSaving(true);
  try {
    const { reading_progress: _rp, index_status: _is, collections: _cs, ...paper } = detail;
    void _rp; void _is; void _cs;
    const updated = await updatePaper(paperId, paper);
    setDetail({ ...detail, ...updated });
    setSavedDetail({ ...detail, ...updated }); // 更新快照
    showToast("success", "已保存");
  } catch (e) {
    showToast("error", `保存失败: ${(e as Error).message}`);
  } finally {
    setSaving(false);
  }
}
```

**改动 B**: 保存按钮加 dirty 高亮(有未保存修改时变红色)

```tsx
<Button
  size="sm"
  variant={isDirty ? "default" : "ghost"}
  onClick={handleSave}
  disabled={saving}
  className={isDirty ? "bg-primary text-primary-foreground" : ""}
>
```

**改动 C**: 切换论文前由父组件处理(父组件 PaperListPane 切换 selectedPaperId 时,此处 useEffect 自然触发;真正的"切换"由列表点击控制,详情面板无法拦截)。采用简化方案:用 beforeunload 提示 + dirty 时保存按钮高亮,不做切换拦截(切换拦截需要改父组件,复杂度高)。

### 步骤 3:修复 DOI 提取(问题 3)

**文件**: `src-tauri/src/pdf.rs:30`

**改动**: DOI 正则改为允许大小写字母(与 `identifier.rs:62` 一致)

```rust
// 改前
let doi_re = Regex::new(r"10\.\d{4,9}/[-._;()/:A-Z0-9]+").unwrap();

// 改后
let doi_re = Regex::new(r"10\.\d{4,9}/[-._;()/:A-Za-z0-9]+").unwrap();
```

**测试**: `pdf.rs:74` 的测试正则也要同步改。

### 步骤 4:修复 AI 总结 os error 3(问题 4)

**文件**: `src-tauri/src/services/ai_svc.rs:64-86`

**改动 A**: 路径解析改为相对 vault 兼容绝对路径

```rust
if !paper.pdf_path.is_empty() {
    let pp = if std::path::Path::new(&paper.pdf_path).is_absolute() {
        std::path::PathBuf::from(&paper.pdf_path)
    } else {
        vault.join(&paper.pdf_path)
    };
    // ... 后续用 pp
}
```

**改动 B**: extract_basic/extract_pages 失败时 log 并继续(不吞错)

```rust
if pp.exists() {
    if let Ok(basic) = crate::pdf::extract_basic(&pp) {
        vars.insert("first_page_text".into(), basic.first_page_text);
        vars.insert("page_count".into(), basic.page_count.to_string());
    } else {
        log::warn!("PDF 首页提取失败: {}", pp.display());
    }
    let pages = crate::pdf::extract_pages(&pp);
    if pages.is_empty() {
        log::warn!("PDF 分页提取为空: {}", pp.display());
        vars.insert("pdf_text".into(), String::new());
    } else {
        let text: String = pages
            .iter()
            .take(40)
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>()
            .join("\n\n");
        vars.insert("pdf_text".into(), text.chars().take(20000).collect());
    }
} else {
    log::warn!("PDF 文件不存在: {}", pp.display());
    vars.insert("pdf_text".into(), String::new());
}
```

**改动 C**: `vault::copy_pdf` 仍然存绝对路径(不改 DB schema,避免迁移),但 `ai_svc.rs` 用上面的兼容逻辑处理。`commands/papers.rs:154` 的 `pdf_path: pdf_path.to_string_lossy().to_string()` 保持不变。

### 步骤 5:添加系统提示词(问题 5)

**文件**: `src-tauri/src/services/ai_svc.rs:94-103`

**改动**: 在 preset 的 system_prompt 前拼接统一身份前缀

```rust
const BASE_IDENTITY: &str = "你是 PaperVault 的论文助手,专注于学术论文的阅读、理解、总结、元数据管理与复现规划。回答需准确、简洁、结构化,使用 Markdown 格式。";
let system_content = format!("{BASE_IDENTITY}\n\n{}", p.system_prompt);
let messages = vec![
    client::ChatMessage {
        role: "system".into(),
        content: system_content,
    },
    client::ChatMessage {
        role: "user".into(),
        content: user_msg,
    },
];
```

### 步骤 6:修复 6000 字限制(问题 6)

**文件**: `src-tauri/src/services/ai_svc.rs:84` + `src-tauri/src/ai/presets.rs:43`

**改动 A**: `ai_svc.rs` 硬编码改为 20000 字 + take(40) 页

```rust
// 改前
let text: String = pages.iter().take(20)...
vars.insert("pdf_text".into(), text.chars().take(6000).collect());

// 改后
let text: String = pages.iter().take(40)...
vars.insert("pdf_text".into(), text.chars().take(20000).collect());
```

**改动 B**: `presets.rs:43` 模板描述同步更新

```rust
// 改前
user_template: "标题：{{title}}\n作者：{{authors}}\n\nPDF 文本（前 6000 字）：\n{{pdf_text}}\n\n请总结：".into(),

// 改后
user_template: "标题：{{title}}\n作者：{{authors}}\n\nPDF 文本（前 20000 字）：\n{{pdf_text}}\n\n请总结：".into(),
```

### 步骤 7:添加 AI 对话功能(问题 7)

**新增文件**: `src/components/ai/ChatPanel.tsx`

**功能**: 独立 Chat 面板,多轮对话 + 历史记录(会话内)+ 流式响应(简化为非流式,一次返回)

**Props**:
```typescript
interface Props {
  paperId: string;
}
```

**State**:
```typescript
interface ChatMessage {
  role: "user" | "assistant";
  content: string;
  timestamp: number;
}
const [messages, setMessages] = useState<ChatMessage[]>([]);
const [input, setInput] = useState("");
const [loading, setLoading] = useState(false);
```

**UI**:
- 顶部: 标题"AI 对话" + 清空按钮
- 中间: 消息列表(用户右对齐 / AI 左对齐,Markdown 渲染)
- 底部: 输入框 + 发送按钮(Enter 发送 / Shift+Enter 换行)

**发送逻辑**:
```typescript
async function handleSend() {
  if (!input.trim() || loading) return;
  const userMsg: ChatMessage = { role: "user", content: input, timestamp: Date.now() };
  setMessages((m) => [...m, userMsg]);
  setInput("");
  setLoading(true);
  try {
    const result = await api.chatWithPaper(paperId, input, messages.map(m => ({ role: m.role, content: m.content })));
    const aiMsg: ChatMessage = { role: "assistant", content: result, timestamp: Date.now() };
    setMessages((m) => [...m, aiMsg]);
  } catch (e) {
    showToast("error", `对话失败: ${(e as Error).message}`);
  } finally {
    setLoading(false);
  }
}
```

**新增后端命令**: `src-tauri/src/commands/ai.rs`

```rust
#[tauri::command]
pub async fn chat_with_paper(
    state: State<'_, AppState>,
    paper_id: String,
    input: String,
    history: Vec<ChatMessageInput>,
) -> AppResult<String> {
    let vault = require_vault(&state)?;
    crate::services::ai_svc::chat(&vault, &paper_id, &input, &history).await
}
```

**新增后端服务**: `src-tauri/src/services/ai_svc.rs::chat`

```rust
pub async fn chat(
    vault: &Path,
    paper_id: &str,
    input: &str,
    history: &[ChatMessageInput],
) -> AppResult<String> {
    let cfg = get_provider(vault)?;
    const BASE_IDENTITY: &str = "你是 PaperVault 的论文助手..."; // 同步骤 5
    let mut messages = vec![client::ChatMessage {
        role: "system".into(),
        content: format!("{BASE_IDENTITY}\n\n当前正在讨论论文 ID: {paper_id}。用户可能基于该论文提问。").into(),
    }];
    // 加载论文元数据作为上下文
    if let Some(paper) = crate::services::paper::load_paper(vault, paper_id)? {
        messages.push(client::ChatMessage {
            role: "system".into(),
            content: format!("论文上下文：\n标题：{}\n作者：{}\n年份：{}\nDOI：{}\n摘要：{}",
                paper.title, paper.authors.join(", "),
                paper.year.map(|y| y.to_string()).unwrap_or_default(),
                paper.doi, paper.abstract_text),
        });
    }
    // 加历史消息
    for m in history {
        messages.push(client::ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        });
    }
    // 加当前输入
    messages.push(client::ChatMessage {
        role: "user".into(),
        content: input.to_string(),
    });
    let raw = client::chat(&cfg, messages, false).await?;
    Ok(raw)
}
```

**新增类型**: `src-tauri/src/types.rs`

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChatMessageInput {
    pub role: String,
    pub content: String,
}
```

**前端 API**: `src/lib/api.ts`

```typescript
chatWithPaper: (paperId: string, input: string, history: { role: string; content: string }[]) =>
  call<string>("chat_with_paper", { paperId, input, history }),
```

**路由集成**: 在 `ReaderShell.tsx` 笔记区上方加 Tab 切换(笔记 / AI 对话),或在 AIPanel 下方加 ChatPanel。**简化方案**: 在 AIPanel.tsx 底部加一个"AI 对话"折叠区,点击展开 ChatPanel。

### 步骤 8:添加复现实验计划 preset(问题 8)

**文件**: `src-tauri/src/ai/presets.rs`

**新增 preset**:

```rust
crate::types::AISkillPreset {
    id: "builtin:reproduction_plan".into(),
    name: "制定复现实验计划".into(),
    bound_action: "reproduction_plan".into(),
    skill: "pdf".into(),
    system_prompt: "你负责基于论文方法部分制定代码复现计划。输出结构化 Markdown：1) 复现目标 2) 环境依赖 3) 核心算法步骤 4) 关键超参数 5) 数据准备 6) 评估指标 7) 可能的坑。给出可直接落地的伪代码或 Python 代码片段。".into(),
    user_template: "标题：{{title}}\n作者：{{authors}}\n\nPDF 文本（前 20000 字）：\n{{pdf_text}}\n\n请制定复现实验计划：".into(),
    output_format: "markdown".into(),
    auto_write: false,
    is_builtin: true,
    updated_at: now,
},
```

**文件**: `src/components/ai/AIPanel.tsx`

**新增 action**:

```typescript
{
  presetId: "reproduction_plan",
  label: "复现实验计划",
  icon: FlaskConical, // 从 lucide-react import
  description: "基于论文方法部分制定代码复现计划与步骤。",
  writesAiBlock: "summary", // 写入笔记 summary 区块
},
```

**import 加**: `FlaskConical` from lucide-react

## 验证步骤

```powershell
cd f:\learn\study\ai\trea\src-tauri
$env:Path = "C:\Users\业天\.cargo\bin;$env:Path"
cargo clippy --lib --all-features  # 0 warning
cargo test --lib                    # 全部通过

cd ..
npx tsc --noEmit                    # 0 error
```

## 假设与决策

1. **问题 1 setter 不互相清空** — 调用方(CollectionsPane)已同时调用两个 setter,清空逻辑由调用方负责,store setter 只管自己的字段
2. **问题 2 不做切换拦截** — 简化方案:dirty 时保存按钮高亮 + beforeunload 提示,不做复杂的切换确认对话框(需要改父组件)
3. **问题 3 只改正则** — `pdf_extract` crate 能力有限不改,仅修正正则大小写 bug
4. **问题 4 不改 DB schema** — `vault::copy_pdf` 仍存绝对路径,`ai_svc.rs` 用兼容逻辑(绝对路径直接用,相对路径拼 vault)。未来可考虑迁移为相对路径
5. **问题 5 统一前缀** — 所有 preset(含用户自定义)自动带上"PaperVault 论文助手"身份
6. **问题 6 硬编码改 20000** — 不做 preset 可配置(避免 DB 迁移),硬编码改大。20 页→40 页,6000 字→20000 字
7. **问题 7 非流式对话** — 简化实现,一次返回完整响应。历史记录仅会话内,不持久化
8. **问题 8 复现计划写入笔记 summary 区块** — 用户可手动编辑覆盖

## 验证清单

- [ ] 左侧"未读/在读/已读"状态分类可点击,列表正确筛选
- [ ] 左侧集合分类可点击,列表正确筛选
- [ ] 修改元数据后保存按钮变红(高亮 dirty)
- [ ] 点击保存后按钮恢复,toast"已保存"
- [ ] 导入含小写 DOI 的 PDF(如 10.1109/cvpr.2020.01234),DOI 字段正确提取
- [ ] AI 总结不再报 os error 3
- [ ] AI 总结结果明显更长(20000 字输入)
- [ ] AI 输出带"PaperVault 论文助手"身份语气
- [ ] AIPanel 下方有"AI 对话"入口,可多轮对话
- [ ] AIPanel 有"复现实验计划"按钮,点击生成结构化复现方案
