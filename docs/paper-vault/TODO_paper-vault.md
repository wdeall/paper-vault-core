# TODO — PaperVault v1

> 提交时已识别的待办与缺失配置项，方便你直接定位支持。

## 用户需要做的配置

### 1. 安装前端依赖

```bash
cd f:\learn\study\ai\trea
pnpm install
```

如果 `pnpm` 不可用，可改用 `npm install` 或 `yarn install`。

### 2. 启动开发

```bash
pnpm tauri:dev
```

### 3. 首次启动时

- 在弹出的 VaultInitDialog 中选择 PaperVault 库目录（建议 `D:\Documents\PaperVault` 等固定位置）
- 可选：点击"加载示例数据"自动创建 5 篇占位论文
- 在 设置 → AI 配置 中填入 OpenAI 兼容 API 的 base_url / api_key / model
  - 若无 API：元数据提取 / 翻译 / 总结 / 自动建笔记将无法运行
  - 应用其余功能（PDF 阅读 / Markdown 笔记 / 搜索 / 导出）完全可用

### 4. 环境变量

`.env.example` 已在仓库根目录，**真实 `.env` 不要提交 git**（已在 `.gitignore` 中）。

```ini
# 复制 .env.example 为 .env，按需填写
PAPERVAULT_DEV_BASE_URL=https://api.openai.com/v1
PAPERVAULT_DEV_API_KEY=sk-xxx
PAPERVAULT_DEV_MODEL=gpt-4o-mini
```

注意：当前实现不读取 `.env`，AI 配置通过 UI 设置页保存到 `papers.db` 的 `ai_provider_config` 表。

## 待办事项（v1.5 候选）

### 测试

- [ ] 补 `tests/setup.ts` + 关键 store 单元测试
- [ ] 补 `src-tauri/src/services/*` 单元测试（tokio::test）
- [ ] 端到端测试脚本（`scripts/e2e.ts`，使用 vitest + happy-dom）

### 体验

- [ ] CodeMirror 暗色主题（已装 `@codemirror/theme-one-dark`，未应用）
- [ ] PDF 全文搜索结果跳转精确页码（已传 page 字段，前端未实现跳转）
- [ ] 笔记删除（v1 仅"删除条目"，笔记文件保留）
- [ ] 论文列表批量选择（导出 UI 已支持 selectedIds，但 PaperListPane 未提供复选框）
- [ ] 论文编辑自动保存（当前每次手动"保存"）
- [ ] 大 PDF 性能优化（pdfjs lazy load + 分块渲染）

### 安全 / 备份

- [ ] HTTP 客户端加 60s timeout
- [ ] 数据库备份自动轮转（保留 N 个）
- [ ] 单库密码 / 加密（v2 考虑 SQLCipher）

### AI

- [ ] AI 运行历史（成功 / 失败 / 耗时 / 用量）
- [ ] 流式输出（SSE / chunked）
- [ ] 提示词变量类型校验

## 已知限制（v1 接受）

| 项 | 状态 | 计划 |
|---|---|---|
| 智能集合 | 未实现 | v1.5 |
| 多论文综述草稿 | 未实现 | v1.5 |
| PDF 高亮批注 | 未实现 | v1.5 |
| 相关论文查找 UI | 部分（结果展示，无主动 UI） | v1.5 |
| RIS 导出 | 未实现 | v1.5 |
| 双向链接 | 未实现 | v2 |
| 知识图谱 | 未实现 | v2 |
| Canvas 研究地图 | 未实现 | v2 |
| 云同步 | 不做 | 手动复制 PaperVault/ 目录 |
| 多端协同 | 不做 | v1 单机 |

## 仓库结构速查

```
f:\learn\study\ai\trea\
  package.json              # 前端依赖
  README.md
  docs/paper-vault/         # 6 份规范 + 3 份交付文档
  src/                      # React 前端
    routes/                 # LibraryPage / ReaderPage / SettingsPage
    components/
      library/              # 三栏
      reader/               # 阅读工作台
      notes/                # CodeMirror 笔记
      ai/                   # AI 工具栏
      search/               # 搜索
      settings/             # 设置
      ui/                   # shadcn 基础组件
    lib/                    # api / utils / tauri
    stores/                 # Zustand
    types/                  # TS 类型
  src-tauri/                # Rust 后端
    src/
      commands/             # 33 个 IPC 命令
      services/             # 业务服务
      db/migrations/        # SQL 迁移
      ai/                   # AI 子模块
      export/               # 引用导出
    tauri.conf.json
    capabilities/default.json
```

## 验证清单（手动）

```bash
# 1. 类型检查
cd f:\learn\study\ai\trea
pnpm typecheck

# 2. 启动
pnpm tauri:dev

# 3. 浏览器预览（无 Tauri 也能看 UI）
pnpm dev
# 访问 http://localhost:1420 — 会显示 VaultInitDialog 提示"在 Tauri 中初始化"
```

## 联系

如遇问题：先看 `docs/paper-vault/ACCEPTANCE_paper-vault.md` 的"已知限制"部分，再决定是否进入 v1.5。
