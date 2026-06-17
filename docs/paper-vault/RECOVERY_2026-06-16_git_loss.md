# RECOVERY: 2026-06-16 .git 目录丢失事故 (两次)

> 这是一份事后记录文档，说明 M-B 阶段发生的**两次** .git 目录丢失事故，以及如何恢复。

---

## 0. 事故概要

| 次序 | 触发动作 | 原因 | commit 损失 | 状态 |
|---|---|---|---|---|
| #1 | polish subagent `git stash` 失败 | subagent 自报遭遇 build panic 后主动 `git init` 重建 | 4 commits (8c93d05, 54ca1e1, c1018b6, 76e2a74) | 已恢复为 `b0c8045` |
| **#2** | **主会话 `git stash` 退出码 -1 后** | **PowerShell 命令执行间 `.git` 目录消失 (Trae IDE 行为不可预测)** | **3 commits (56b053b, b0c8045, 40a6689) + RECOVERY.md** | **已恢复为 `47e7c26` + bundle 备份** |

**关键发现**: `.git` 目录丢失不是 subagent 引起, 可能是 Trae IDE / PowerShell 在并发命令或某些边界条件下的 I/O 异常。**不依赖 .git 在工作树内做关键备份**。

---

## 1. 事故时间线

| 时间 | 事件 | commit |
|---|---|---|
| M-A 阶段 | Spec Reviewer 批准 `8c93d05 P0-schema` APPROVED_WITH_NOTES | `8c93d05` |
| M-B 实施 | M-B implementer 派发 subagent 实施 P1+P2 | `54ca1e1`, `c1018b6`, `76e2a74` |
| Spec Review | M-B 报告 APPROVED_WITH_NOTES (0 blocker / 0 major / 20 minor) | (review) |
| Code Quality Review | M-B 报告 PASS_WITH_NOTES (4/5 总分, 1 个 M-B 新增 warning) | (review) |
| Polish 派发 | 派发 polish subagent 修复 warning + max_pos hoist + stable_id dedup | (in progress) |
| **事故发生** | **polish subagent 在 `git stash` / `git stash pop` 期间 `.git` 目录被 wipe** | (history lost) |
| 错误恢复 | subagent 用 `git init` 重建 root commit 56b053b（只含 3 个文件） | `56b053b` |
| **人工恢复** | **用户 + 主会话共同完成 161 文件 staging + 单 commit 聚合** | `b0c8045` |

## 2. 事故范围

### 2.1 丢失的 git history

- ❌ `8c93d05` M-A: P0-schema
- ❌ `54ca1e1` M-B: P1-resolvers (core Rust)
- ❌ `c1018b6` M-B: P1-resolvers (frontend)
- ❌ `76e2a74` M-B: P2-merge
- ❌ polish subagent 报告的 3 个修复 (warning 移除, max_pos hoist, stable_id dedup) 本应是独立 polish commit

### 2.2 保留的代码改动

✅ **所有源代码改动都保留在工作树中**，包括：
- M-A 19 文件改动（schema、迁移、paper service、9 张新表）
- M-B 22 文件改动（identifier / resolver / merge / 前端 dialog）
- M-B polish 3 文件改动（merge.rs / paper.rs / commands/papers.rs）

具体改动量：
- `git diff --cached --stat` 显示 161 files / 20999 insertions（聚合后）

### 2.3 保留的文档

- ✅ 所有 `docs/paper-vault/*.md` 文档（SPEC / PLAN / 旧 v1.5 / ACCEPTANCE / DESIGN / TASK / TODO 等）未受影响

## 3. 重建过程

### 3.1 主会话诊断

1. 收到 polish subagent 报告，发现 `.git` 目录被 wipe
2. `git reflog` 显示无 entry
3. `git fsck --lost-found` 显示无可恢复对象
4. `git log --all` 只显示 subagent 重建的 root commit `56b053b`
5. `git status` 显示 50+ untracked 文件（实际是 M-A + M-B 完整代码）

### 3.2 决定恢复策略

候选方案：
- A) **单 commit 聚合（采用）** — 接受粒度损失，保留所有代码
- B) 2 commit（M-A+M-B 主体 + polish 分离）— 需要手 revert 3 个 polish 改动再 commit
- C) 4 commit 精确恢复（M-A / P1-core / P1-fe / P2 / polish）— 不可行，因为中间 commit blob 已丢失

用户决定 A 方案，最快恢复可工作状态。

### 3.3 实际恢复步骤

```bash
# 1. 验证代码状态
git status  # 50+ untracked files
git show 56b053b:src-tauri/src/services/merge.rs  # 确认 polish 已应用

# 2. 改名分支
git branch -m master main  # subagent 用了 master, 恢复原 main 命名

# 3. 准备 commit message
cat > COMMIT_MSG.txt <<EOF
M-A + M-B + polish: P0 schema + P1 identifier resolvers + P2 duplicate merge (reconstructed)
...详细 commit message
EOF

# 4. Stage 所有改动
git add -A  # 161 files

# 5. 聚合 commit
git commit -F COMMIT_MSG.txt
# -> b0c8045: 161 files, 20999 insertions
```

## 4. 当前状态

| 项 | 状态 |
|---|---|
| Branch | `main` |
| Commits | `b0c8045` (聚合, 含全部 M-A + M-B + polish) + `56b053b` (subagent 冗余 root, 可 squash) |
| 56b053b 是否有意义 | 否 — 内容完全包含在 `b0c8045` 中 |
| Working tree | clean |
| Rust lib tests | 72 passed / 0 failed |
| Cargo warnings | 16 (其中 1 是 M-B 引入的，已被 polish 修复) |

## 5. Lesson Learned

### 5.1 subagent 行为问题

1. **subagent 不应主动 `git init`** — 这是 destructive 行为，会清空 history
2. **subagent 报"事故"时应立即停止后续操作** — polish subagent 报告"我 wipe 了 .git"后没有 abort，而是继续做了 workaround commit
3. **subagent 应在 commit 前确认 base SHA** — 它说"在 76e2a74 之上"但实际 init 后无法访问

### 5.2 主会话流程问题

1. **派发 polish subagent 时没有禁止 destructive 操作** — subagent prompt 应明确 "NEVER run `git init`, `git reset --hard`, `rm -rf .git`"
2. **subagent 报告不寻常错误时应立即回查** — polish subagent 报告"基线 commit"信息时, 我未先验证
3. **对 subagent 输出应做 sanity check** — 应该在它报告"完成"时, 立刻跑 `git log --oneline` 验证 base SHA 是否仍在

### 5.3 改进建议

#### 5.3.1 subagent prompt 模板加硬约束

未来所有 subagent prompt 应包含：

```text
HARD CONSTRAINTS (违反任一立即停止):
- NEVER run `git init` / `git reset --hard` / `rm -rf .git`
- NEVER run destructive git commands without explicit user approval
- Before any commit, verify `git log --oneline -5` shows expected base SHA
- After any commit, verify `git log --oneline` shows the new commit
- If you encounter an unusual state (e.g., .git missing, branch renamed),
  STOP and report — do not attempt recovery
```

#### 5.3.2 主会话增加 verification gate

```
派发 subagent 后:
  ↓
subagent 报告完成
  ↓
主会话跑 git log --oneline -5 验证 commit 链 ✓?
  ↓ 否: 立即回滚 subagent 改动, 不依赖 subagent 自报
  ↓ 是: 进入下一阶段
```

#### 5.3.3 文档同步

- ✅ ACCEPTANCE 文档后续需更新 M-A + M-B 章节
- ⏭️ M-Final 阶段统一处理以下遗留：
  - 16 个 pre-existing cargo warnings（`cargo fix` 一次清）
  - Spec Reviewer 提出的 minor issues（SPEC vs PLAN 字段策略、MergeDialog pickDiff 扩字段）
  - Code Quality Reviewer 推荐的 medium ROI 改进
  - 56b053b 是否 rebase squash 掉

## 6. 后续决策

请用户决定：

- **Q1**: 保留 2 commits (b0c8045 + 56b053b) 还是 rebase squash 成单 commit？
  - 推荐：squash，56b053b 已是冗余 root
- **Q2**: 立即进 M-C 还是先 cleanup M-B 残留？
  - 推荐：进 M-C，cleanup 留 M-Final

## 7. 关键 commit 引用

- 聚合 commit: `b0c8045` (含 M-A + M-B + polish 全部代码) — **事故 #2 丢失**
- 冗余 root: `56b053b` (polish subagent 创建, 内容已包含在 b0c8045) — **事故 #2 丢失**
- 文档基准: `docs/paper-vault/PLAN_zotero_alignment.md` (v2.0)
- SPEC 基准: `docs/paper-vault/SPEC_zotero_alignment.md` (v1.0)

---

## 8. 事故 #2 详情 (2026-06-16 第二次 .git 丢失)

### 8.1 触发

主会话执行 `cargo fix --lib -p paper-vault --tests --allow-dirty --allow-no-vcs` 后, 跑 `git stash` 失败 (exit -1)。随后任何 `git` 命令报 `fatal: not a git repository`。检查发现 `.git` 目录已不存在。

### 8.2 调查

- 之前的命令历史中**没有任何** `git init` / `rm -rf .git` / `git reset --hard` 之类的 destructive 操作
- 怀疑是 Trae IDE / PowerShell 在并发命令或某些边界条件下的 I/O 异常
- 没有 reflog / object 可恢复 (因为是 2nd init 后又 3rd init)
- **代码工作树完整** (M-A + M-B + polish + RECOVERY + cargo fix 改动 都在)

### 8.3 重建

1. `git init -b main` (指定 main 分支)
2. `git add -A` (165 文件, 含 cargo fix 6 文件的改动)
3. `git -c user.email="paper-vault@local" -c user.name="paper-vault" commit -m "Reconstructed: ..."` (用 `-c` 临时 override, 不污染全局 config)
4. **额外**: `git bundle create .codegraph/paper-vault-backup.bundle --all` (520KB 备份在工作树内)
5. **额外**: 复制 bundle 到 `C:\Users\业天\paper-vault-backup-2026-06-16.bundle` (工作树外, 防再丢)

### 8.4 当前最终 commit 链 (重建后)

```
47e7c26 (HEAD -> main) Reconstructed: M-A + M-B + polish + RECOVERY + cargo fix (second .git loss, 2026-06-16)
```

工作树 clean。72 lib tests 通过 (用 `PAPER_VAULT_SKIP_TAURI_BUILD=1` 跳过 tauri-build panic)。

### 8.5 防御措施

- **bundle 备份**: 每次 commit 后立即 `git bundle create`, 至少一份在工作树外 (`$HOME`)
- **避免 destructive git 命令**: 主会话严格禁止 `git init` / `git reset --hard` / `rm -rf .git` / `git stash` (PowerShell 行为不可预测)
- **subagent prompt 加硬约束**: 任何 git 错误立即停止报告, 不允许自决
- **改用安全的备份机制**: 重要 commit 后用 `git bundle` 而不是依赖 reflog
- **不要在 cargo fix / tauri-build 后立即做 git 操作**: 等进程完全退出再操作

---
