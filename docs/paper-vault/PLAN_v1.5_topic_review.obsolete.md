> ⚠️ **OBSOLETE — 此文档已被新的对齐 Zotero 方案取代，不再维护**
>
> 本实施计划对应的 v1.5 "L1 主题综述线" 已废弃。
> 新的方向：见 [`SPEC_zotero_alignment.md`](./SPEC_zotero_alignment.md) 与后续的 `PLAN_zotero_alignment.md`（P0–P6 分阶段实施）。
> 保留此文档仅为历史参考。
>
> ---

# PaperVault v1.5 Implementation Plan

> 6A 工作流 Automate 阶段的实施计划。每 Task 自包含、可独立编译/测试；勾选框 `- [ ]` 用于追踪。

**Goal:** 在 v1 基础上交付 L1 主题综述线 — 智能集合 + 关键词/标签聚合 + 批量勾选 + 多论文综述生成 + 主题笔记。

**Architecture:** 后端新增 `rule.rs` 规则引擎与 `topic.rs` 主题笔记服务；扩展 `ai_svc` 支持多论文上下文拼装；前端新增 `smart` / `topic` / `batch` 三组组件与 store；左栏 `CollectionsPane` 加智能集合 / 关键词 / 标签三段。

**Tech Stack:** Rust + Tauri 2 + rusqlite + reqwest；React 18 + TypeScript + Zustand + shadcn/ui。

**SPEC 引用:** [SPEC_v1.5.md](./SPEC_v1.5.md)

---

## 文件结构

### 新增

```
src-tauri/src/
  db/migrations/0002_v1_5.sql
  rule.rs
  topic.rs
  services/smart.rs
  services/aggregate.rs
  services/topic_review.rs
  commands/smart.rs
  commands/aggregate.rs
  commands/topic.rs

src/components/smart/
  SmartCollectionsSection.tsx
  SmartCollectionEditor.tsx
  RuleRow.tsx
  ValueInput.tsx
  HitPreview.tsx

src/components/library/
  BatchToolbar.tsx
  KeywordsSection.tsx
  TagsSection.tsx

src/components/topic/
  TopicReviewDialog.tsx
  TopicNotePicker.tsx

src/stores/
  smart.ts
  batch.ts
  topic.ts
```

### 修改

```
src-tauri/src/lib.rs                  ← 注册新 IPC
src-tauri/src/commands/mod.rs
src-tauri/src/services/mod.rs
src-tauri/src/types.rs                ← Rule / SmartCollection / TopicNote / KeywordCount / TopicReviewResult
src-tauri/src/error.rs                ← 加 BadRequest 变体
src-tauri/src/ai/client.rs            ← timeout_secs
src-tauri/src/ai/presets.rs           ← topic_literature_review preset
src-tauri/src/services/paper.rs       ← list_by_rules / count_by_rules
src-tauri/src/services/ai_svc.rs      ← 拆 run_single 与 run_topic_review
src-tauri/src/markdown.rs             ← extract_user_handwritten / replace_named_block
src-tauri/src/commands/init.rs        ← 调 smart::seed_builtins_if_empty

src/types/index.ts
src/lib/api.ts
src/components/library/CollectionsPane.tsx
src/components/library/PaperListPane.tsx
src/stores/paper.ts
```

### 依赖顺序

```
P0 → P1 → P2 / P3 / P4 / P5 / P6（并行）→ P7 / P8 / P9 / P10（前端，并行）→ P11 → P12
```

---

## P0 — 数据模型迁移

### Task 0.1: 写迁移 SQL

**Files:**
- Create: `src-tauri/src/db/migrations/0002_v1_5.sql`
- Modify: `src-tauri/src/db/mod.rs`

- [ ] **Step 1: 写迁移 SQL**

```sql
CREATE TABLE IF NOT EXISTS smart_collections (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  rules_json  TEXT NOT NULL,
  sort_by     TEXT NOT NULL DEFAULT 'updated_at',
  sort_dir    TEXT NOT NULL DEFAULT 'desc',
  is_builtin  INTEGER NOT NULL DEFAULT 0,
  icon        TEXT,
  created_at  INTEGER NOT NULL,
  updated_at  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_smart_collections_builtin ON smart_collections(is_builtin);

CREATE TABLE IF NOT EXISTS topic_notes (
  id                  TEXT PRIMARY KEY,
  title               TEXT NOT NULL,
  note_path           TEXT NOT NULL UNIQUE,
  source_papers_json  TEXT NOT NULL,
  created_at          INTEGER NOT NULL,
  updated_at          INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_topic_notes_updated ON topic_notes(updated_at DESC);

ALTER TABLE ai_provider_config ADD COLUMN timeout_secs INTEGER NOT NULL DEFAULT 60;
```

- [ ] **Step 2: 注册迁移**

打开 `src-tauri/src/db/mod.rs`，在 MIGRATIONS 数组末尾追加：

```rust
const MIGRATIONS: &[(&str, &str)] = &[
    ("0001_init", include_str!("migrations/0001_init.sql")),
    ("0002_v1_5", include_str!("migrations/0002_v1_5.sql")),
];
```

- [ ] **Step 3: 验证迁移**

```bash
cd src-tauri && cargo build && cargo test --lib db
```

Expected: 编译通过、迁移测试通过；如无迁移测试，运行 `cargo tauri dev` 打开既有 vault 不报错。

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/db/migrations/0002_v1_5.sql src-tauri/src/db/mod.rs
git commit -m "feat(db): add smart_collections, topic_notes, timeout_secs"
```

---

## P1 — Rust 规则引擎

### Task 1.1: 类型与白名单

**Files:**
- Create: `src-tauri/src/rule.rs`
- Modify: `src-tauri/src/lib.rs`、`src-tauri/src/error.rs`

- [ ] **Step 1: 加 BadRequest 错误变体**

打开 `src-tauri/src/error.rs`，在 `AppError` 枚举里追加：

```rust
#[error("BadRequest: {0}")]
BadRequest(String),
```

确保 `serialize` 把 `kind` 设为 `"bad_request"`（与 v1 其他变体同模式）。

- [ ] **Step 2: 写 rule.rs（类型 + 校验）**

```rust
//! 智能集合规则引擎：解析 + 校验 + 转 SQL
use crate::error::{AppError, AppResult};
use rusqlite::ToSql;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    pub field: String,
    pub op: String,
    pub value: Value,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "match")]
    pub match_mode: Option<String>,
}

const FIELDS: &[(&str, &[&str])] = &[
    ("status",       &["=", "!="]),
    ("year",         &["=", ">=", "<=", "between"]),
    ("rating",       &[">=", "<="]),
    ("authors",      &["contains"]),
    ("title",        &["contains"]),
    ("venue",        &["contains", "="]),
    ("created_at",   &[">=", "<=", "last_n_days"]),
    ("updated_at",   &[">=", "<=", "last_n_days"]),
    ("index_status", &["="]),
    ("has_note",     &["="]),
    ("tags",         &["contains", "not_contains"]),
    ("keywords",     &["contains", "not_contains"]),
];

pub fn validate(rule: &Rule) -> AppResult<()> {
    for (f, ops) in FIELDS {
        if rule.field == *f {
            if ops.contains(&rule.op.as_str()) { return Ok(()); }
            return Err(AppError::BadRequest(format!(
                "字段 {} 不支持操作符 {}", rule.field, rule.op
            )));
        }
    }
    Err(AppError::BadRequest(format!("未知字段: {}", rule.field)))
}
```

并在 `lib.rs` 顶部加 `pub mod rule;`。

- [ ] **Step 3: 编译**

```bash
cd src-tauri && cargo build
```

- [ ] **Step 4: 提交**

```bash
git add src-tauri/src/rule.rs src-tauri/src/lib.rs src-tauri/src/error.rs
git commit -m "feat(rule): Rule type and whitelist validation"
```

### Task 1.2: 规则转 SQL（TDD）

**Files:**
- Modify: `src-tauri/src/rule.rs`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn r(field: &str, op: &str, v: Value) -> Rule {
        Rule { field: field.into(), op: op.into(), value: v, match_mode: None }
    }
    fn rm(field: &str, op: &str, v: Value, m: &str) -> Rule {
        Rule { field: field.into(), op: op.into(), value: v, match_mode: Some(m.into()) }
    }

    #[test]
    fn scalar_eq() {
        let (sql, args) = translate(&[r("status","=", json!("未读"))]).unwrap();
        assert!(sql.contains("p.status = ?"));
        assert_eq!(args.len(), 1);
    }
    #[test]
    fn keywords_any() {
        let (sql, args) = translate(&[rm("keywords","contains", json!(["a","b"]), "any")]).unwrap();
        assert!(sql.contains("EXISTS"));
        assert!(sql.contains("?,?"));
        assert_eq!(args.len(), 2);
    }
    #[test]
    fn keywords_all() {
        let (sql, _) = translate(&[rm("keywords","contains", json!(["a","b"]), "all")]).unwrap();
        assert!(sql.contains("COUNT(DISTINCT"));
        assert!(sql.contains(") = 2"));
    }
    #[test]
    fn has_note_false() {
        let (sql, _) = translate(&[r("has_note","=", json!(false))]).unwrap();
        assert!(sql.contains("note_path = ''") || sql.contains("note_path IS NULL"));
    }
    #[test]
    fn last_n_days() {
        let (sql, args) = translate(&[r("created_at","last_n_days", json!(7))]).unwrap();
        assert!(sql.contains("p.created_at >= ?"));
        assert_eq!(args.len(), 1);
    }
    #[test]
    fn unknown_field() {
        assert!(translate(&[r("xxx","=", json!(1))]).is_err());
    }
    #[test]
    fn between_year() {
        let (sql, args) = translate(&[r("year","between", json!([2020, 2024]))]).unwrap();
        assert!(sql.contains("BETWEEN"));
        assert_eq!(args.len(), 2);
    }
    #[test]
    fn empty_rules() {
        let (sql, args) = translate(&[]).unwrap();
        assert_eq!(sql, "");
        assert_eq!(args.len(), 0);
    }
}
```

- [ ] **Step 2: 跑测试（应失败）**

```bash
cd src-tauri && cargo test --lib rule::tests
```

Expected: FAIL（`translate` 未定义）

- [ ] **Step 3: 实现 translate**

在 `rule.rs` 末尾加：

```rust
/// 把 [Rule; AND] 翻译成一段 WHERE 片段（不含 WHERE 关键字）+ 参数。
/// 调用方在外面拼 SELECT ... FROM papers p WHERE 1=1 AND <frag> ORDER BY ...
pub fn translate(rules: &[Rule]) -> AppResult<(String, Vec<Box<dyn ToSql>>)> {
    let mut frags: Vec<String> = Vec::new();
    let mut args: Vec<Box<dyn ToSql>> = Vec::new();

    for rule in rules {
        validate(rule)?;
        let frag = match (rule.field.as_str(), rule.op.as_str()) {
            ("year" | "rating" | "created_at" | "updated_at", op @ ("=" | ">=" | "<=")) => {
                args.push(value_to_sql(&rule.value)?);
                format!("p.{} {} ?", rule.field, op)
            }
            ("year", "between") => {
                let arr = rule.value.as_array()
                    .ok_or_else(|| AppError::BadRequest("between 需要 [a,b]".into()))?;
                if arr.len() != 2 {
                    return Err(AppError::BadRequest("between 需要 [a,b]".into()));
                }
                args.push(value_to_sql(&arr[0])?);
                args.push(value_to_sql(&arr[1])?);
                "p.year BETWEEN ? AND ?".to_string()
            }
            ("created_at" | "updated_at", "last_n_days") => {
                let n = rule.value.as_i64()
                    .ok_or_else(|| AppError::BadRequest("last_n_days 需整数".into()))?;
                let cutoff = chrono::Local::now().timestamp_millis() - n * 86_400_000;
                args.push(Box::new(cutoff));
                format!("p.{} >= ?", rule.field)
            }
            ("status", op @ ("=" | "!=")) => {
                args.push(value_to_sql(&rule.value)?);
                format!("p.status {} ?", op)
            }
            ("venue", op @ ("=" | "contains")) => {
                let s = rule.value.as_str()
                    .ok_or_else(|| AppError::BadRequest("venue 值需字符串".into()))?;
                if op == "=" {
                    args.push(Box::new(s.to_string()));
                    "p.venue = ?".to_string()
                } else {
                    args.push(Box::new(format!("%{}%", s)));
                    "p.venue LIKE ?".to_string()
                }
            }
            ("authors" | "title", "contains") => {
                let s = rule.value.as_str()
                    .ok_or_else(|| AppError::BadRequest("contains 需字符串".into()))?;
                args.push(Box::new(format!("%{}%", s)));
                format!("p.{} LIKE ?", rule.field)
            }
            ("index_status", "=") => {
                let s = rule.value.as_str()
                    .ok_or_else(|| AppError::BadRequest("index_status 需字符串".into()))?;
                args.push(Box::new(s.to_string()));
                "(SELECT status FROM index_status WHERE paper_id = p.id) = ?".to_string()
            }
            ("has_note", "=") => {
                let b = rule.value.as_bool()
                    .ok_or_else(|| AppError::BadRequest("has_note 需布尔".into()))?;
                if b {
                    "(p.note_path IS NOT NULL AND p.note_path != '')".into()
                } else {
                    "(p.note_path IS NULL OR p.note_path = '')".into()
                }
            }
            ("keywords" | "tags", op @ ("contains" | "not_contains")) => {
                let arr: Vec<String> = match &rule.value {
                    Value::Array(a) => a.iter().map(|v| {
                        v.as_str().map(|s| s.to_string())
                            .ok_or_else(|| AppError::BadRequest("数组元素须字符串".into()))
                    }).collect::<AppResult<Vec<_>>>()?,
                    Value::String(s) => vec![s.clone()],
                    _ => return Err(AppError::BadRequest("keywords/tags 需字符串或数组".into())),
                };
                if arr.is_empty() {
                    return Err(AppError::BadRequest("contains 需至少一个值".into()));
                }
                let placeholders = vec!["?"; arr.len()].join(",");
                let n = arr.len();
                let mode = rule.match_mode.as_deref();
                let col = &rule.field;
                let body = if n == 1 || mode == Some("any") || mode.is_none() {
                    format!("EXISTS (SELECT 1 FROM json_each(p.{col}) WHERE value IN ({placeholders}))")
                } else {
                    format!("(SELECT COUNT(DISTINCT value) FROM json_each(p.{col}) WHERE value IN ({placeholders})) = {n}")
                };
                for v in arr { args.push(Box::new(v)); }
                if op == "not_contains" { format!("NOT ({body})") } else { body }
            }
            _ => return Err(AppError::BadRequest(format!(
                "不支持的组合: {} {}", rule.field, rule.op
            ))),
        };
        frags.push(frag);
    }
    Ok((frags.join(" AND "), args))
}

fn value_to_sql(v: &Value) -> AppResult<Box<dyn ToSql>> {
    Ok(match v {
        Value::String(s) => Box::new(s.clone()),
        Value::Number(n) if n.is_i64() => Box::new(n.as_i64().unwrap()),
        Value::Number(n) if n.is_f64() => Box::new(n.as_f64().unwrap()),
        Value::Bool(b) => Box::new(*b as i64),
        _ => return Err(AppError::BadRequest("不支持的值类型".into())),
    })
}
```

- [ ] **Step 4: 跑测试通过**

```bash
cargo test --lib rule::tests
```

Expected: 8 passed

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/rule.rs
git commit -m "feat(rule): translate rules to parameterized SQL"
```

### Task 1.3: paper.rs 加通用查询

**Files:**
- Modify: `src-tauri/src/services/paper.rs`

- [ ] **Step 1: 加函数**

在 `paper.rs` 末尾追加：

```rust
pub fn list_by_rules(
    vault: &Path,
    rules: &[crate::rule::Rule],
    sort_by: &str,
    sort_dir: &str,
) -> AppResult<Vec<Paper>> {
    let allowed = ["updated_at","created_at","title","year"];
    let sort_col = if allowed.contains(&sort_by) { sort_by } else { "updated_at" };
    let dir = if sort_dir.eq_ignore_ascii_case("asc") { "ASC" } else { "DESC" };
    let (frag, args) = crate::rule::translate(rules)?;
    let where_clause = if frag.is_empty() { String::new() } else { format!("AND {frag}") };
    let sql = format!(
        "SELECT p.* FROM papers p WHERE 1=1 {where_clause} ORDER BY p.{sort_col} {dir}"
    );
    let conn = db::open(vault)?;
    let mut stmt = conn.prepare(&sql)?;
    let arg_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let rows = stmt.query_map(&arg_refs[..], row_to_paper)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn count_by_rules(vault: &Path, rules: &[crate::rule::Rule]) -> AppResult<u64> {
    let (frag, args) = crate::rule::translate(rules)?;
    let where_clause = if frag.is_empty() { String::new() } else { format!("AND {frag}") };
    let sql = format!("SELECT COUNT(*) FROM papers p WHERE 1=1 {where_clause}");
    let conn = db::open(vault)?;
    let arg_refs: Vec<&dyn rusqlite::ToSql> = args.iter().map(|b| b.as_ref()).collect();
    let n: i64 = conn.query_row(&sql, &arg_refs[..], |r| r.get(0))?;
    Ok(n as u64)
}
```

- [ ] **Step 2: 编译并提交**

```bash
cargo build
git add src-tauri/src/services/paper.rs
git commit -m "feat(paper): list_by_rules and count_by_rules"
```

---

## P2 — 智能集合 CRUD + IPC + Seed

### Task 2.1: SmartCollection 类型

**Files:**
- Modify: `src-tauri/src/types.rs`

- [ ] **Step 1: 加类型**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartCollection {
    pub id: String,
    pub name: String,
    pub rules: Vec<crate::rule::Rule>,
    pub sort_by: String,
    pub sort_dir: String,
    pub is_builtin: bool,
    pub icon: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}
```

- [ ] **Step 2: 编译并提交**

```bash
cargo build
git add src-tauri/src/types.rs
git commit -m "feat(types): SmartCollection"
```

### Task 2.2: smart 服务（CRUD + seed）

**Files:**
- Create: `src-tauri/src/services/smart.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: 写服务**

```rust
//! 智能集合 CRUD + 内置 seed
use crate::db;
use crate::error::{AppError, AppResult};
use crate::rule::Rule;
use crate::types::SmartCollection;
use rusqlite::{params, OptionalExtension};
use serde_json::json;
use std::path::Path;

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<SmartCollection> {
    let rules_json: String = r.get("rules_json")?;
    Ok(SmartCollection {
        id: r.get("id")?,
        name: r.get("name")?,
        rules: serde_json::from_str(&rules_json).unwrap_or_default(),
        sort_by: r.get("sort_by")?,
        sort_dir: r.get("sort_dir")?,
        is_builtin: r.get::<_, i32>("is_builtin")? != 0,
        icon: r.get("icon")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

pub fn list(vault: &Path) -> AppResult<Vec<SmartCollection>> {
    let conn = db::open(vault)?;
    let mut stmt = conn.prepare(
        "SELECT * FROM smart_collections ORDER BY is_builtin DESC, name ASC"
    )?;
    let rows = stmt.query_map([], row)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn get(vault: &Path, id: &str) -> AppResult<SmartCollection> {
    db::open(vault)?.query_row(
        "SELECT * FROM smart_collections WHERE id = ?1", params![id], row,
    ).optional()?
    .ok_or_else(|| AppError::NotFound(format!("smart collection {id} 不存在")))
}

pub fn create(
    vault: &Path, name: &str, rules: &[Rule],
    sort_by: &str, sort_dir: &str, icon: Option<&str>,
) -> AppResult<SmartCollection> {
    let id = format!("user:{}", uuid::Uuid::new_v4().simple());
    let now = chrono::Local::now().timestamp_millis();
    insert_row(vault, &id, name, rules, sort_by, sort_dir, false, icon, now)?;
    get(vault, &id)
}

pub fn update(vault: &Path, id: &str, patch: &SmartCollection) -> AppResult<SmartCollection> {
    let cur = get(vault, id)?;
    if cur.is_builtin {
        return Err(AppError::BadRequest("内置智能集合不可修改".into()));
    }
    let now = chrono::Local::now().timestamp_millis();
    db::open(vault)?.execute(
        "UPDATE smart_collections
         SET name=?2, rules_json=?3, sort_by=?4, sort_dir=?5, icon=?6, updated_at=?7
         WHERE id=?1",
        params![id, patch.name, serde_json::to_string(&patch.rules)?,
                patch.sort_by, patch.sort_dir, patch.icon, now],
    )?;
    get(vault, id)
}

pub fn delete(vault: &Path, id: &str) -> AppResult<()> {
    let cur = get(vault, id)?;
    if cur.is_builtin {
        return Err(AppError::BadRequest("内置智能集合不可删除".into()));
    }
    db::open(vault)?.execute(
        "DELETE FROM smart_collections WHERE id=?1", params![id]
    )?;
    Ok(())
}

fn insert_row(
    vault: &Path, id: &str, name: &str, rules: &[Rule],
    sort_by: &str, sort_dir: &str, is_builtin: bool, icon: Option<&str>, now: i64,
) -> AppResult<()> {
    db::open(vault)?.execute(
        "INSERT OR REPLACE INTO smart_collections
         (id, name, rules_json, sort_by, sort_dir, is_builtin, icon, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        params![id, name, serde_json::to_string(rules)?,
                sort_by, sort_dir, is_builtin as i32, icon, now, now],
    )?;
    Ok(())
}

pub fn seed_builtins_if_empty(vault: &Path) -> AppResult<()> {
    let conn = db::open(vault)?;
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM smart_collections WHERE is_builtin = 1", [], |r| r.get(0)
    )?;
    if n > 0 { return Ok(()); }
    let now = chrono::Local::now().timestamp_millis();
    let year: i64 = chrono::Local::now().format("%Y").to_string().parse().unwrap_or(2026);

    let presets: Vec<(&str, String, &str, Vec<Rule>)> = vec![
        ("builtin:unread", "未读论文".into(), "📥",
            vec![Rule{field:"status".into(),op:"=".into(),value:json!("未读"),match_mode:None}]),
        ("builtin:starred","重点重读".into(),"🔥",
            vec![Rule{field:"status".into(),op:"=".into(),value:json!("重点重读"),match_mode:None}]),
        ("builtin:y_current", format!("{} 年论文", year), "📅",
            vec![Rule{field:"year".into(),op:"=".into(),value:json!(year),match_mode:None}]),
        ("builtin:recent7","最近添加".into(),"🆕",
            vec![Rule{field:"created_at".into(),op:"last_n_days".into(),value:json!(7),match_mode:None}]),
        ("builtin:read7","最近阅读".into(),"📖",
            vec![Rule{field:"updated_at".into(),op:"last_n_days".into(),value:json!(7),match_mode:None}]),
        ("builtin:noote","已读但无笔记".into(),"✏️",
            vec![
                Rule{field:"status".into(),op:"=".into(),value:json!("已读"),match_mode:None},
                Rule{field:"has_note".into(),op:"=".into(),value:json!(false),match_mode:None},
            ]),
        ("builtin:todo","待补全".into(),"⏳",
            vec![Rule{field:"tags".into(),op:"contains".into(),value:json!(["待补全"]),match_mode:Some("any".into())}]),
    ];
    for (id, name, icon, rules) in presets {
        insert_row(vault, id, &name, &rules, "updated_at", "desc", true, Some(icon), now)?;
    }
    Ok(())
}
```

- [ ] **Step 2: 注册到 services/mod.rs**

```rust
pub mod smart;
```

- [ ] **Step 3: 在 init_vault 调 seed**

打开 `src-tauri/src/commands/init.rs`，找到 v1 已有的 `seed_builtins_if_empty`（用于 ai presets）调用之后，追加：

```rust
crate::services::smart::seed_builtins_if_empty(&vault_path)?;
```

- [ ] **Step 4: 编译并提交**

```bash
cargo build
git add src-tauri/src/services/smart.rs src-tauri/src/services/mod.rs src-tauri/src/commands/init.rs
git commit -m "feat(smart): smart_collections service + 7 builtin seed"
```

### Task 2.3: 智能集合 IPC

**Files:**
- Create: `src-tauri/src/commands/smart.rs`
- Modify: `src-tauri/src/commands/mod.rs`、`src-tauri/src/lib.rs`

- [ ] **Step 1: 写命令**

```rust
//! 智能集合 IPC
use crate::error::{AppError, AppResult};
use crate::rule::Rule;
use crate::services::{paper, smart};
use crate::types::{Paper, SmartCollection};
use crate::AppState;
use tauri::State;

fn vault_of(s: &State<'_, AppState>) -> AppResult<std::path::PathBuf> {
    s.vault_path.read().as_ref().cloned()
        .ok_or_else(|| AppError::Config("vault 未初始化".into()))
}

#[tauri::command]
pub async fn list_smart_collections(state: State<'_, AppState>) -> AppResult<Vec<SmartCollection>> {
    smart::list(&vault_of(&state)?)
}
#[tauri::command]
pub async fn get_smart_collection(state: State<'_, AppState>, id: String) -> AppResult<SmartCollection> {
    smart::get(&vault_of(&state)?, &id)
}
#[tauri::command]
pub async fn create_smart_collection(
    state: State<'_, AppState>, name: String, rules: Vec<Rule>,
    sort_by: Option<String>, sort_dir: Option<String>, icon: Option<String>,
) -> AppResult<SmartCollection> {
    smart::create(&vault_of(&state)?, &name, &rules,
        sort_by.as_deref().unwrap_or("updated_at"),
        sort_dir.as_deref().unwrap_or("desc"),
        icon.as_deref())
}
#[tauri::command]
pub async fn update_smart_collection(
    state: State<'_, AppState>, id: String, patch: SmartCollection,
) -> AppResult<SmartCollection> {
    smart::update(&vault_of(&state)?, &id, &patch)
}
#[tauri::command]
pub async fn delete_smart_collection(state: State<'_, AppState>, id: String) -> AppResult<()> {
    smart::delete(&vault_of(&state)?, &id)
}
#[tauri::command]
pub async fn list_papers_by_smart(state: State<'_, AppState>, id: String) -> AppResult<Vec<Paper>> {
    let v = vault_of(&state)?;
    let sc = smart::get(&v, &id)?;
    paper::list_by_rules(&v, &sc.rules, &sc.sort_by, &sc.sort_dir)
}
#[tauri::command]
pub async fn preview_smart_collection(
    state: State<'_, AppState>, rules: Vec<Rule>,
    sort_by: Option<String>, sort_dir: Option<String>,
) -> AppResult<Vec<Paper>> {
    paper::list_by_rules(&vault_of(&state)?, &rules,
        sort_by.as_deref().unwrap_or("updated_at"),
        sort_dir.as_deref().unwrap_or("desc"))
}
#[tauri::command]
pub async fn count_smart_collection(state: State<'_, AppState>, rules: Vec<Rule>) -> AppResult<u64> {
    paper::count_by_rules(&vault_of(&state)?, &rules)
}
```

- [ ] **Step 2: 注册命令**

`commands/mod.rs`：

```rust
pub mod smart;
pub use smart::{
    list_smart_collections, get_smart_collection, create_smart_collection,
    update_smart_collection, delete_smart_collection,
    list_papers_by_smart, preview_smart_collection, count_smart_collection,
};
```

`lib.rs::tauri::generate_handler!` 末尾追加：

```rust
commands::list_smart_collections,
commands::get_smart_collection,
commands::create_smart_collection,
commands::update_smart_collection,
commands::delete_smart_collection,
commands::list_papers_by_smart,
commands::preview_smart_collection,
commands::count_smart_collection,
```

- [ ] **Step 3: 编译并提交**

```bash
cargo build
git add src-tauri/src/commands/smart.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(commands): smart_collections IPC (8 commands)"
```

---

## P3 — 关键词 / 标签聚合

### Task 3.1: KeywordCount 类型

**Files:**
- Modify: `src-tauri/src/types.rs`

- [ ] **Step 1: 加类型**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordCount {
    pub keyword: String,
    pub count: u64,
}
```

- [ ] **Step 2: 提交**

```bash
git add src-tauri/src/types.rs
git commit -m "feat(types): KeywordCount"
```

### Task 3.2: 聚合服务

**Files:**
- Create: `src-tauri/src/services/aggregate.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: 写服务**

```rust
//! 关键词 / 标签聚合 + pin 为智能集合
use crate::db;
use crate::error::AppResult;
use crate::rule::Rule;
use crate::services::smart;
use crate::types::{KeywordCount, SmartCollection};
use rusqlite::params;
use serde_json::json;
use std::path::Path;

pub fn list_keywords_with_count(vault: &Path, limit: Option<u32>) -> AppResult<Vec<KeywordCount>> {
    list_field(vault, "keywords", limit)
}
pub fn list_tags_with_count(vault: &Path, limit: Option<u32>) -> AppResult<Vec<KeywordCount>> {
    list_field(vault, "tags", limit)
}
fn list_field(vault: &Path, field: &str, limit: Option<u32>) -> AppResult<Vec<KeywordCount>> {
    let conn = db::open(vault)?;
    let mut sql = format!(
        "SELECT value, COUNT(*) AS cnt FROM papers p, json_each(p.{field})
         GROUP BY value ORDER BY cnt DESC, value ASC"
    );
    if let Some(l) = limit { sql.push_str(&format!(" LIMIT {l}")); }
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |r| Ok(KeywordCount {
        keyword: r.get(0)?, count: r.get::<_, i64>(1)? as u64,
    }))?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

/// 已固定 → 返回已有项；默认 name = keyword 本身。
pub fn pin_keyword_as_collection(
    vault: &Path, keyword: &str, name: Option<&str>,
) -> AppResult<SmartCollection> {
    let id = format!("pin:keyword:{}", keyword);
    let conn = db::open(vault)?;
    let exists = conn.query_row(
        "SELECT 1 FROM smart_collections WHERE id = ?1", params![id], |_| Ok(1i32),
    ).ok().is_some();
    if exists { return smart::get(vault, &id); }

    let display = name.unwrap_or(keyword);
    let rule = Rule {
        field: "keywords".into(), op: "contains".into(),
        value: json!([keyword]), match_mode: Some("any".into()),
    };
    let now = chrono::Local::now().timestamp_millis();
    conn.execute(
        "INSERT INTO smart_collections
         (id,name,rules_json,sort_by,sort_dir,is_builtin,icon,created_at,updated_at)
         VALUES (?1,?2,?3,'updated_at','desc',0,'🏷️',?4,?4)",
        params![id, display, serde_json::to_string(&vec![rule])?, now],
    )?;
    smart::get(vault, &id)
}
```

- [ ] **Step 2: 注册到 services/mod.rs**

```rust
pub mod aggregate;
```

- [ ] **Step 3: 编译并提交**

```bash
cargo build
git add src-tauri/src/services/aggregate.rs src-tauri/src/services/mod.rs
git commit -m "feat(aggregate): keywords/tags counts + pin_keyword"
```

### Task 3.3: 聚合 IPC

**Files:**
- Create: `src-tauri/src/commands/aggregate.rs`
- Modify: `commands/mod.rs`、`lib.rs`

- [ ] **Step 1: 写命令**

```rust
//! 聚合 IPC
use crate::error::{AppError, AppResult};
use crate::services::aggregate;
use crate::types::{KeywordCount, SmartCollection};
use crate::AppState;
use tauri::State;

fn vault_of(s: &State<'_, AppState>) -> AppResult<std::path::PathBuf> {
    s.vault_path.read().as_ref().cloned()
        .ok_or_else(|| AppError::Config("vault 未初始化".into()))
}

#[tauri::command]
pub async fn list_keywords_with_count(
    state: State<'_, AppState>, limit: Option<u32>,
) -> AppResult<Vec<KeywordCount>> {
    aggregate::list_keywords_with_count(&vault_of(&state)?, limit)
}
#[tauri::command]
pub async fn list_tags_with_count(
    state: State<'_, AppState>, limit: Option<u32>,
) -> AppResult<Vec<KeywordCount>> {
    aggregate::list_tags_with_count(&vault_of(&state)?, limit)
}
#[tauri::command]
pub async fn pin_keyword_as_collection(
    state: State<'_, AppState>, keyword: String, name: Option<String>,
) -> AppResult<SmartCollection> {
    aggregate::pin_keyword_as_collection(&vault_of(&state)?, &keyword, name.as_deref())
}
```

- [ ] **Step 2: 注册（commands/mod.rs + lib.rs）**

```rust
// commands/mod.rs
pub mod aggregate;
pub use aggregate::{list_keywords_with_count, list_tags_with_count, pin_keyword_as_collection};
```

`lib.rs::generate_handler!` 追加：

```rust
commands::list_keywords_with_count,
commands::list_tags_with_count,
commands::pin_keyword_as_collection,
```

- [ ] **Step 3: 编译并提交**

```bash
cargo build
git add src-tauri/src/commands/aggregate.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(commands): aggregate IPC (3 commands)"
```

---

## P4 — 主题笔记表 + 服务 + IPC + 模板

### Task 4.1: TopicNote 类型 + topic.rs

**Files:**
- Create: `src-tauri/src/topic.rs`
- Modify: `src-tauri/src/types.rs`、`lib.rs`、`vault.rs`（确保 `slug_from_title` 已 pub）

- [ ] **Step 1: 加类型**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicNote {
    pub id: String,
    pub title: String,
    pub note_path: String,
    pub source_papers: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}
```

- [ ] **Step 2: 写 topic.rs**

```rust
//! 主题笔记 CRUD + 模板生成 + AI 区块写入
use crate::db;
use crate::error::{AppError, AppResult};
use crate::types::TopicNote;
use crate::vault;
use rusqlite::{params, OptionalExtension};
use std::fs;
use std::path::Path;

const TEMPLATE: &str = r#"---
id: {id}
type: topic
title: {title}
source_papers: {source_papers}
created_at: {ts}
updated_at: {ts}
---

# {title}

## 我的总结


## AI 综述
<!-- AI_GENERATED_START:review -->
<!-- AI_GENERATED_END:review -->

## 涉及论文
<!-- AI_GENERATED_START:paper_list -->
<!-- AI_GENERATED_END:paper_list -->

## 笔记

"#;

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<TopicNote> {
    let sp: String = r.get("source_papers_json")?;
    Ok(TopicNote {
        id: r.get("id")?,
        title: r.get("title")?,
        note_path: r.get("note_path")?,
        source_papers: serde_json::from_str(&sp).unwrap_or_default(),
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
    })
}

pub fn list(vault: &Path) -> AppResult<Vec<TopicNote>> {
    let conn = db::open(vault)?;
    let mut stmt = conn.prepare("SELECT * FROM topic_notes ORDER BY updated_at DESC")?;
    let rows = stmt.query_map([], row)?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

pub fn get(vault: &Path, id: &str) -> AppResult<TopicNote> {
    db::open(vault)?.query_row(
        "SELECT * FROM topic_notes WHERE id=?1", params![id], row,
    ).optional()?
    .ok_or_else(|| AppError::NotFound(format!("topic {id} 不存在")))
}

pub fn create(vault: &Path, title: &str, source_papers: &[String]) -> AppResult<TopicNote> {
    let slug = vault::slug_from_title(title);
    let id = format!("topic-{}", slug);
    let dir = vault.join("notes").join("topics");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{slug}.md"));
    if path.exists() {
        return Err(AppError::BadRequest(format!("主题笔记已存在: {}", path.display())));
    }
    let now = chrono::Local::now().timestamp_millis();
    let body = TEMPLATE
        .replace("{id}", &id)
        .replace("{title}", title)
        .replace("{source_papers}", &serde_json::to_string(source_papers)?)
        .replace("{ts}", &now.to_string());
    fs::write(&path, body)?;
    db::open(vault)?.execute(
        "INSERT INTO topic_notes (id,title,note_path,source_papers_json,created_at,updated_at)
         VALUES (?1,?2,?3,?4,?5,?5)",
        params![id, title, path.to_string_lossy().to_string(),
                serde_json::to_string(source_papers)?, now],
    )?;
    get(vault, &id)
}

pub fn delete(vault: &Path, id: &str, also_remove_file: bool) -> AppResult<()> {
    let cur = get(vault, id)?;
    db::open(vault)?.execute("DELETE FROM topic_notes WHERE id=?1", params![id])?;
    if also_remove_file {
        let p = std::path::Path::new(&cur.note_path);
        if p.exists() { let _ = fs::remove_file(p); }
    }
    Ok(())
}

/// 把 review 与 paper_list 两个 AI 区块写入主题笔记，并更新 source_papers / updated_at。
/// 如果 marker 不完整，按 SPEC 8.3 降级策略：在文件末尾追加完整新区块。
pub fn write_review_blocks(
    vault: &Path,
    id: &str,
    review_md: &str,
    paper_list_md: &str,
    new_source_papers: &[String],
) -> AppResult<TopicNote> {
    let cur = get(vault, id)?;
    let path = std::path::Path::new(&cur.note_path);
    let original = fs::read_to_string(path)?;

    let with_review = crate::markdown::replace_named_block(&original, "review", review_md);
    let with_list = crate::markdown::replace_named_block(&with_review, "paper_list", paper_list_md);
    let with_fm = crate::markdown::patch_frontmatter(
        &with_list,
        &[
            ("source_papers", &serde_json::to_string(new_source_papers)?),
            ("updated_at", &chrono::Local::now().timestamp_millis().to_string()),
        ],
    );
    fs::write(path, with_fm)?;

    let now = chrono::Local::now().timestamp_millis();
    db::open(vault)?.execute(
        "UPDATE topic_notes SET source_papers_json=?2, updated_at=?3 WHERE id=?1",
        params![id, serde_json::to_string(new_source_papers)?, now],
    )?;
    get(vault, id)
}
```

并在 `lib.rs` 顶部加 `pub mod topic;`。

- [ ] **Step 3: 编译并提交**

```bash
cargo build
git add src-tauri/src/topic.rs src-tauri/src/types.rs src-tauri/src/lib.rs
git commit -m "feat(topic): TopicNote CRUD + template + write_review_blocks"
```

### Task 4.2: markdown.rs 加 replace_named_block 与 patch_frontmatter

**Files:**
- Modify: `src-tauri/src/markdown.rs`

- [ ] **Step 1: 加函数**

```rust
/// 用 new_content 替换 `<!-- AI_GENERATED_START:{name} -->...<!-- AI_GENERATED_END:{name} -->`
/// 区块。任一 marker 缺失 → 在文件末尾追加完整新块。
pub fn replace_named_block(src: &str, name: &str, new_content: &str) -> String {
    let start = format!("<!-- AI_GENERATED_START:{name} -->");
    let end = format!("<!-- AI_GENERATED_END:{name} -->");
    let block = format!("{start}\n{}\n{end}", new_content.trim_end());

    if let (Some(s), Some(e)) = (src.find(&start), src.find(&end)) {
        if e > s {
            let mut out = String::with_capacity(src.len() + new_content.len());
            out.push_str(&src[..s]);
            out.push_str(&block);
            out.push_str(&src[e + end.len()..]);
            return out;
        }
    }
    let mut out = src.to_string();
    if !out.ends_with('\n') { out.push('\n'); }
    out.push('\n');
    out.push_str(&format!("## {}\n", name));
    out.push_str(&block);
    out.push('\n');
    out
}

/// 替换 frontmatter 中指定 key 的 value（仅简单 yaml；不存在则添加）。
pub fn patch_frontmatter(src: &str, kv: &[(&str, &str)]) -> String {
    if !src.starts_with("---\n") { return src.to_string(); }
    let end = match src[4..].find("\n---\n") { Some(i) => 4 + i, None => return src.to_string() };
    let head = &src[4..end];
    let mut lines: Vec<String> = head.lines().map(|s| s.to_string()).collect();
    for (k, v) in kv {
        let prefix = format!("{}:", k);
        let value_line = format!("{}: {}", k, v);
        if let Some(i) = lines.iter().position(|l| l.starts_with(&prefix)) {
            lines[i] = value_line;
        } else {
            lines.push(value_line);
        }
    }
    let new_head = lines.join("\n");
    format!("---\n{}\n---\n{}", new_head, &src[end + 5..])
}

#[cfg(test)]
mod block_tests {
    use super::*;

    #[test]
    fn replace_existing_block() {
        let src = "abc\n<!-- AI_GENERATED_START:review -->\nold\n<!-- AI_GENERATED_END:review -->\nxyz";
        let r = replace_named_block(src, "review", "new");
        assert!(r.contains("new"));
        assert!(!r.contains("old"));
        assert!(r.contains("xyz"));
    }

    #[test]
    fn append_when_missing() {
        let src = "no markers here";
        let r = replace_named_block(src, "review", "x");
        assert!(r.contains("AI_GENERATED_START:review"));
        assert!(r.contains("AI_GENERATED_END:review"));
        assert!(r.contains("no markers here"));
    }

    #[test]
    fn patch_fm() {
        let src = "---\nid: a\nupdated_at: 1\n---\nbody";
        let r = patch_frontmatter(src, &[("updated_at", "999")]);
        assert!(r.contains("updated_at: 999"));
        assert!(!r.contains("updated_at: 1\n"));
    }
}
```

- [ ] **Step 2: 跑测试**

```bash
cargo test --lib markdown::block_tests
```

Expected: 3 passed

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/markdown.rs
git commit -m "feat(markdown): replace_named_block + patch_frontmatter"
```

### Task 4.3: topic IPC

**Files:**
- Create: `src-tauri/src/commands/topic.rs`
- Modify: `commands/mod.rs`、`lib.rs`

- [ ] **Step 1: 写命令（仅 CRUD，综述生成放 P5）**

```rust
//! 主题笔记 IPC（CRUD 部分；综述见 P5）
use crate::error::{AppError, AppResult};
use crate::topic;
use crate::types::TopicNote;
use crate::AppState;
use tauri::State;

fn vault_of(s: &State<'_, AppState>) -> AppResult<std::path::PathBuf> {
    s.vault_path.read().as_ref().cloned()
        .ok_or_else(|| AppError::Config("vault 未初始化".into()))
}

#[tauri::command]
pub async fn list_topic_notes(state: State<'_, AppState>) -> AppResult<Vec<TopicNote>> {
    topic::list(&vault_of(&state)?)
}

#[tauri::command]
pub async fn create_topic_note(
    state: State<'_, AppState>, title: String, source_papers: Vec<String>,
) -> AppResult<TopicNote> {
    topic::create(&vault_of(&state)?, &title, &source_papers)
}

#[tauri::command]
pub async fn delete_topic_note(
    state: State<'_, AppState>, id: String, also_remove_file: bool,
) -> AppResult<()> {
    topic::delete(&vault_of(&state)?, &id, also_remove_file)
}
```

- [ ] **Step 2: 注册（commands/mod.rs + lib.rs）**

```rust
// commands/mod.rs
pub mod topic;
pub use topic::{list_topic_notes, create_topic_note, delete_topic_note};
```

`lib.rs::generate_handler!` 追加：

```rust
commands::list_topic_notes,
commands::create_topic_note,
commands::delete_topic_note,
```

- [ ] **Step 3: 编译并提交**

```bash
cargo build
git add src-tauri/src/commands/topic.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(commands): topic_notes IPC (3 commands)"
```

---

## P5 — 综述生成（preset + 服务 + IPC）

### Task 5.1: 内置 preset `topic_literature_review`

**Files:**
- Modify: `src-tauri/src/ai/presets.rs`

- [ ] **Step 1: 在 `builtin_presets` 末尾追加**

```rust
AISkillPreset {
    id: "builtin:topic_literature_review".into(),
    name: "主题综述（多论文）".into(),
    bound_action: "topic_literature_review".into(),
    skill: "literature-review".into(),
    system_prompt: r#"你是一名严谨的学术综述助手。基于用户提供的 N 篇论文（含元数据与用户笔记），生成一份结构化的主题综述初稿，覆盖：
1. 研究问题与动机
2. 主要方法路线（按类别归并，注明来源论文）
3. 实验与对比（指出共识与分歧）
4. 局限与开放问题
5. 进一步阅读建议

要求：
- 输出 Markdown，不带 code fence
- 引用论文时使用 [{title}] 简写
- 禁止编造未在输入中出现的论文 / 数据
- 用户笔记中的"我"代表论文阅读者，可引用其观察
- 控制在 800–1500 字"#.into(),
    user_template: "主题：{{topic_title}}\n论文数：{{paper_count}}\n\n输入：\n{{papers_with_notes}}".into(),
    output_format: "markdown".into(),
    auto_write: true,
    is_builtin: true,
    updated_at: now,
},
```

- [ ] **Step 2: 编译**

```bash
cargo build
```

- [ ] **Step 3: 提交**

```bash
git add src-tauri/src/ai/presets.rs
git commit -m "feat(ai): add topic_literature_review builtin preset"
```

### Task 5.2: topic_review 服务（拼装 + 调 AI + 写入）

**Files:**
- Create: `src-tauri/src/services/topic_review.rs`
- Modify: `src-tauri/src/services/mod.rs`、`src-tauri/src/types.rs`、`src-tauri/src/markdown.rs`

- [ ] **Step 1: 加 TopicReviewResult 类型**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicReviewResult {
    pub note: crate::types::TopicNote,
    pub raw_markdown: String,
    pub token_estimate: u32,
}
```

- [ ] **Step 2: 加 markdown::strip_ai_blocks（拼装上下文用）**

```rust
/// 剔除全部 AI_GENERATED_START:* ... END:* 区块。
pub fn strip_ai_blocks(src: &str) -> String {
    let re = regex::Regex::new(
        r"(?s)<!-- AI_GENERATED_START:[^>]+? -->.*?<!-- AI_GENERATED_END:[^>]+? -->"
    ).unwrap();
    re.replace_all(src, "").to_string()
}
```

确认 `Cargo.toml` 已含 `regex`（v1 已有）。

- [ ] **Step 3: 写服务**

```rust
//! 多论文综述：拼装上下文 → 调 AI → 写入主题笔记
use crate::ai::{client, template};
use crate::db;
use crate::error::{AppError, AppResult};
use crate::services::{ai_svc, preset, topic_review_helpers};
use crate::topic;
use crate::types::{Paper, TopicNote, TopicReviewResult};
use rusqlite::params;
use std::collections::HashMap;
use std::path::Path;

pub struct PromptOverrides {
    pub system: Option<String>,
    pub user: Option<String>,
}

pub async fn run(
    vault: &Path,
    paper_ids: &[String],
    note_id: Option<&str>,
    new_title: Option<&str>,
    preset_id: &str,
    overrides: Option<&PromptOverrides>,
) -> AppResult<TopicReviewResult> {
    if paper_ids.is_empty() {
        return Err(AppError::BadRequest("至少选择 1 篇论文".into()));
    }
    let papers = load_papers(vault, paper_ids)?;
    let title = match (note_id, new_title) {
        (Some(_), _) => String::new(),
        (None, Some(t)) => t.to_string(),
        (None, None) => return Err(AppError::BadRequest("note_id 或 new_title 必填一个".into())),
    };
    let topic_note = match note_id {
        Some(id) => topic::get(vault, id)?,
        None => topic::create(vault, &title, paper_ids)?,
    };

    let papers_md = build_papers_with_notes(vault, &papers)?;
    let token_estimate = (papers_md.chars().count() / 3) as u32;

    let mut p = preset::get_effective(vault, preset_id)?;
    if let Some(o) = overrides {
        if let Some(s) = &o.system { p.system_prompt = s.clone(); }
        if let Some(u) = &o.user { p.user_template = u.clone(); }
    }

    let mut vars: HashMap<String, String> = HashMap::new();
    vars.insert("topic_title".into(), topic_note.title.clone());
    vars.insert("paper_count".into(), papers.len().to_string());
    vars.insert("papers_with_notes".into(), papers_md);

    let user_msg = template::render(&p.user_template, &vars)?;
    let messages = vec![
        client::ChatMessage { role: "system".into(), content: p.system_prompt.clone() },
        client::ChatMessage { role: "user".into(), content: user_msg },
    ];

    let cfg = ai_svc::get_provider(vault)?;
    let raw = client::chat(&cfg, messages, false).await?;

    let paper_list_md = build_paper_list(&papers);
    let updated = topic::write_review_blocks(
        vault, &topic_note.id, &raw, &paper_list_md, paper_ids,
    )?;

    Ok(TopicReviewResult {
        note: updated,
        raw_markdown: raw,
        token_estimate,
    })
}

fn load_papers(vault: &Path, ids: &[String]) -> AppResult<Vec<Paper>> {
    let conn = db::open(vault)?;
    let placeholders = vec!["?"; ids.len()].join(",");
    let sql = format!("SELECT * FROM papers WHERE id IN ({placeholders})");
    let mut stmt = conn.prepare(&sql)?;
    let id_refs: Vec<&dyn rusqlite::ToSql> =
        ids.iter().map(|i| i as &dyn rusqlite::ToSql).collect();
    let rows = stmt.query_map(&id_refs[..], |r| {
        let authors: String = r.get("authors")?;
        let keywords: String = r.get("keywords")?;
        let tags: String = r.get("tags")?;
        Ok(Paper {
            id: r.get("id")?,
            title: r.get("title")?,
            authors: serde_json::from_str(&authors).unwrap_or_default(),
            year: r.get("year")?,
            venue: r.get("venue")?,
            doi: r.get("doi")?,
            abstract_text: r.get("abstract_text")?,
            keywords: serde_json::from_str(&keywords).unwrap_or_default(),
            tags: serde_json::from_str(&tags).unwrap_or_default(),
            status: r.get("status")?,
            rating: r.get("rating")?,
            pdf_path: r.get("pdf_path")?,
            note_path: r.get("note_path")?,
            created_at: r.get("created_at")?,
            updated_at: r.get("updated_at")?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows { out.push(r?); }
    Ok(out)
}

fn build_papers_with_notes(vault: &Path, papers: &[Paper]) -> AppResult<String> {
    let mut buf = String::new();
    let mut total = 0usize;
    let max_total = 24_000;
    for p in papers {
        let abs_short: String = p.abstract_text.chars().take(800).collect();
        let user_notes = if !p.note_path.is_empty() && std::path::Path::new(&p.note_path).exists() {
            let raw = std::fs::read_to_string(&p.note_path).unwrap_or_default();
            let stripped = crate::markdown::strip_ai_blocks(&raw);
            stripped.chars().take(2000).collect::<String>()
        } else { String::new() };
        let block = format!(
            "## [{year}] {title}\nAuthors: {authors}\nVenue: {venue}\nDOI: {doi}\nKeywords: {keywords}\nAbstract: {abs}\nNotes (user-written):\n{notes}\n\n---\n",
            year = p.year.map(|y| y.to_string()).unwrap_or_default(),
            title = p.title,
            authors = p.authors.join(", "),
            venue = p.venue,
            doi = p.doi,
            keywords = p.keywords.join(", "),
            abs = abs_short,
            notes = user_notes,
        );
        if total + block.len() > max_total {
            buf.push_str("\n（已截断：剩余论文未包含）\n");
            break;
        }
        total += block.len();
        buf.push_str(&block);
    }
    Ok(buf)
}

fn build_paper_list(papers: &[Paper]) -> String {
    papers.iter().map(|p| format!(
        "- [{}] {} — {} ({})",
        p.year.map(|y| y.to_string()).unwrap_or_default(),
        p.title, p.authors.join(", "), p.doi,
    )).collect::<Vec<_>>().join("\n")
}
```

注：上面 `services::topic_review_helpers` 是占位，实际不引；如果 IDE 报 unused 删除该 use。

- [ ] **Step 4: 注册 services/mod.rs**

```rust
pub mod topic_review;
```

- [ ] **Step 5: 编译并提交**

```bash
cargo build
git add src-tauri/src/services/topic_review.rs src-tauri/src/services/mod.rs \
        src-tauri/src/types.rs src-tauri/src/markdown.rs
git commit -m "feat(topic_review): assemble multi-paper context + run AI + write blocks"
```

### Task 5.3: generate_topic_review IPC

**Files:**
- Modify: `src-tauri/src/commands/topic.rs`、`lib.rs`

- [ ] **Step 1: 在 `commands/topic.rs` 追加**

```rust
use crate::services::topic_review;

#[derive(serde::Deserialize)]
pub struct PromptOverrideArg {
    pub system: Option<String>,
    pub user: Option<String>,
}

#[tauri::command]
pub async fn generate_topic_review(
    state: State<'_, AppState>,
    paper_ids: Vec<String>,
    note_id: Option<String>,
    new_title: Option<String>,
    preset_id: String,
    prompt_overrides: Option<PromptOverrideArg>,
) -> AppResult<crate::types::TopicReviewResult> {
    let v = vault_of(&state)?;
    let ovr = prompt_overrides.map(|o| topic_review::PromptOverrides {
        system: o.system, user: o.user,
    });
    topic_review::run(
        &v, &paper_ids, note_id.as_deref(), new_title.as_deref(),
        &preset_id, ovr.as_ref(),
    ).await
}
```

- [ ] **Step 2: 注册（commands/mod.rs + lib.rs）**

`commands/mod.rs` 在原有 `pub use topic::{...}` 末尾加 `, generate_topic_review`。

`lib.rs::generate_handler!` 追加：

```rust
commands::generate_topic_review,
```

- [ ] **Step 3: 编译并提交**

```bash
cargo build
git add src-tauri/src/commands/topic.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(commands): generate_topic_review IPC"
```

---

## P6 — HTTP 客户端 60s timeout

### Task 6.1: client::chat 加 timeout

**Files:**
- Modify: `src-tauri/src/ai/client.rs`、`src-tauri/src/services/ai_svc.rs`、`src-tauri/src/types.rs`

- [ ] **Step 1: AIProviderConfig 加 timeout_secs**

```rust
pub struct AIProviderConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub timeout_secs: u32,
}
```

`Default::default()` 设为 `60`。

- [ ] **Step 2: ai_svc::get_provider 读取列**

```rust
"SELECT base_url, api_key, model, timeout_secs FROM ai_provider_config WHERE id = 'default'"
```

并把第 4 列读为 `r.get::<_, i64>(3)? as u32`。

- [ ] **Step 3: client::chat 应用 timeout**

```rust
let client = reqwest::Client::builder()
    .timeout(std::time::Duration::from_secs(cfg.timeout_secs.max(10) as u64))
    .build()?;
```

- [ ] **Step 4: update_provider 写入新列**

```rust
"INSERT INTO ai_provider_config (id, base_url, api_key, model, timeout_secs, updated_at)
 VALUES ('default', ?1, ?2, ?3, ?4, ?5)
 ON CONFLICT(id) DO UPDATE SET
   base_url=excluded.base_url, api_key=excluded.api_key, model=excluded.model,
   timeout_secs=excluded.timeout_secs, updated_at=excluded.updated_at"
```

参数加 `patch.timeout_secs`。

- [ ] **Step 5: 编译并提交**

```bash
cargo build
git add src-tauri/src/ai/client.rs src-tauri/src/services/ai_svc.rs src-tauri/src/types.rs
git commit -m "feat(ai): http timeout_secs (default 60)"
```

---

## P7 — 前端 Batch Store + PaperListPane 复选框 + BatchToolbar

### Task 7.1: batch store

**Files:**
- Create: `src/stores/batch.ts`

- [ ] **Step 1: 写 store**

```ts
import { create } from "zustand";

interface BatchState {
  selected: Set<string>;
  toggle: (id: string) => void;
  setMany: (ids: string[], on: boolean) => void;
  clear: () => void;
}

export const useBatchStore = create<BatchState>((set) => ({
  selected: new Set(),
  toggle: (id) => set((s) => {
    const n = new Set(s.selected);
    if (n.has(id)) n.delete(id); else n.add(id);
    return { selected: n };
  }),
  setMany: (ids, on) => set((s) => {
    const n = new Set(s.selected);
    for (const id of ids) on ? n.add(id) : n.delete(id);
    return { selected: n };
  }),
  clear: () => set({ selected: new Set() }),
}));
```

- [ ] **Step 2: 提交**

```bash
git add src/stores/batch.ts
git commit -m "feat(store): batch selection store"
```

### Task 7.2: PaperListPane 加复选框

**Files:**
- Modify: `src/components/library/PaperListPane.tsx`

- [ ] **Step 1: 引入 store + 在每行插入 checkbox**

在每行 `<li>` 第一列前加：

```tsx
<input
  type="checkbox"
  checked={selected.has(p.id)}
  onChange={() => toggle(p.id)}
  onClick={(e) => e.stopPropagation()}
  className="mr-2"
/>
```

顶部表头加全选：

```tsx
<input
  type="checkbox"
  checked={filtered.length > 0 && filtered.every(p => selected.has(p.id))}
  onChange={(e) => setMany(filtered.map(p => p.id), e.target.checked)}
/>
```

`selected/toggle/setMany` 来自 `useBatchStore`。

- [ ] **Step 2: 编译并提交**

```bash
pnpm typecheck
git add src/components/library/PaperListPane.tsx
git commit -m "feat(ui): batch checkboxes in paper list"
```

### Task 7.3: BatchToolbar

**Files:**
- Create: `src/components/library/BatchToolbar.tsx`
- Modify: `src/components/library/LibraryShell.tsx`（挂载）

- [ ] **Step 1: 写组件**

```tsx
import { useState } from "react";
import { X, Sparkles, Download } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useBatchStore } from "@/stores/batch";
import { TopicReviewDialog } from "@/components/topic/TopicReviewDialog";
import { api } from "@/lib/api";
import { useUIStore } from "@/stores/ui";

export function BatchToolbar() {
  const selected = useBatchStore((s) => s.selected);
  const clear = useBatchStore((s) => s.clear);
  const showToast = useUIStore((s) => s.showToast);
  const [reviewOpen, setReviewOpen] = useState(false);
  if (selected.size === 0) return null;
  const ids = Array.from(selected);

  async function exportBibtex() {
    try {
      const text = await api.exportBibtex(ids);
      await navigator.clipboard.writeText(text);
      showToast("success", `已复制 ${ids.length} 条 BibTeX`);
    } catch (e) {
      showToast("error", `导出失败: ${(e as Error).message}`);
    }
  }

  return (
    <>
      <div className="sticky top-0 z-10 flex items-center gap-2 border-b border-border bg-card px-3 py-1.5 text-sm">
        <span className="font-medium">已选 {selected.size} 篇</span>
        <Button size="sm" onClick={() => setReviewOpen(true)}>
          <Sparkles className="mr-1.5 h-3.5 w-3.5" />
          生成综述
        </Button>
        <Button size="sm" variant="outline" onClick={exportBibtex}>
          <Download className="mr-1.5 h-3.5 w-3.5" />
          导出 BibTeX
        </Button>
        <Button size="sm" variant="ghost" onClick={clear} className="ml-auto">
          <X className="h-3.5 w-3.5" />
        </Button>
      </div>
      {reviewOpen && (
        <TopicReviewDialog
          paperIds={ids}
          onClose={() => setReviewOpen(false)}
          onDone={() => { setReviewOpen(false); clear(); }}
        />
      )}
    </>
  );
}
```

- [ ] **Step 2: 在 LibraryShell 中央栏顶部挂载**

打开 `LibraryShell.tsx`，把 `<PaperListPane />` 包成：

```tsx
<main className="flex-1 overflow-y-auto">
  <BatchToolbar />
  <PaperListPane />
</main>
```

- [ ] **Step 3: 编译并提交**

```bash
pnpm typecheck
git add src/components/library/BatchToolbar.tsx src/components/library/LibraryShell.tsx
git commit -m "feat(ui): BatchToolbar (review / export bibtex)"
```

---

## P8 — 前端 Smart Store + Editor + RuleRow + ValueInput + HitPreview

### Task 8.1: 类型 + api 包装

**Files:**
- Modify: `src/types/index.ts`、`src/lib/api.ts`

- [ ] **Step 1: types/index.ts 加**

```ts
export interface Rule {
  field: string;
  op: string;
  value: unknown;
  match?: "any" | "all";
}

export interface SmartCollection {
  id: string;
  name: string;
  rules: Rule[];
  sort_by: string;
  sort_dir: string;
  is_builtin: boolean;
  icon: string | null;
  created_at: number;
  updated_at: number;
}

export interface KeywordCount { keyword: string; count: number; }

export interface TopicNote {
  id: string; title: string; note_path: string;
  source_papers: string[]; created_at: number; updated_at: number;
}

export interface TopicReviewResult {
  note: TopicNote; raw_markdown: string; token_estimate: number;
}
```

- [ ] **Step 2: lib/api.ts 加方法**

```ts
listSmartCollections: () => call<SmartCollection[]>("list_smart_collections"),
getSmartCollection: (id: string) => call<SmartCollection>("get_smart_collection", { id }),
createSmartCollection: (name: string, rules: Rule[], sortBy?: string, sortDir?: string, icon?: string) =>
  call<SmartCollection>("create_smart_collection", { name, rules, sortBy, sortDir, icon }),
updateSmartCollection: (id: string, patch: SmartCollection) =>
  call<SmartCollection>("update_smart_collection", { id, patch }),
deleteSmartCollection: (id: string) => call<void>("delete_smart_collection", { id }),
listPapersBySmart: (id: string) => call<Paper[]>("list_papers_by_smart", { id }),
previewSmartCollection: (rules: Rule[], sortBy?: string, sortDir?: string) =>
  call<Paper[]>("preview_smart_collection", { rules, sortBy, sortDir }),
countSmartCollection: (rules: Rule[]) => call<number>("count_smart_collection", { rules }),

listKeywordsWithCount: (limit?: number) =>
  call<KeywordCount[]>("list_keywords_with_count", { limit }),
listTagsWithCount: (limit?: number) =>
  call<KeywordCount[]>("list_tags_with_count", { limit }),
pinKeywordAsCollection: (keyword: string, name?: string) =>
  call<SmartCollection>("pin_keyword_as_collection", { keyword, name }),

listTopicNotes: () => call<TopicNote[]>("list_topic_notes"),
createTopicNote: (title: string, sourcePapers: string[]) =>
  call<TopicNote>("create_topic_note", { title, sourcePapers }),
deleteTopicNote: (id: string, alsoRemoveFile: boolean) =>
  call<void>("delete_topic_note", { id, alsoRemoveFile }),
generateTopicReview: (params: {
  paperIds: string[]; noteId?: string; newTitle?: string;
  presetId: string; promptOverrides?: { system?: string; user?: string };
}) => call<TopicReviewResult>("generate_topic_review", params),
```

- [ ] **Step 3: 提交**

```bash
git add src/types/index.ts src/lib/api.ts
git commit -m "feat(api): smart/aggregate/topic IPC wrappers"
```

### Task 8.2: smart store

**Files:**
- Create: `src/stores/smart.ts`

- [ ] **Step 1: 写**

```ts
import { create } from "zustand";
import type { Paper, Rule, SmartCollection } from "@/types";
import { api } from "@/lib/api";

interface SmartState {
  collections: SmartCollection[];
  activeId: string | null;
  activeRules: Rule[] | null;
  activePapers: Paper[];
  loadAll: () => Promise<void>;
  setActive: (id: string | null) => Promise<void>;
  setActiveRules: (rules: Rule[] | null) => Promise<void>;
  create: (sc: Omit<SmartCollection, "id" | "is_builtin" | "created_at" | "updated_at">) => Promise<SmartCollection>;
  remove: (id: string) => Promise<void>;
  count: (rules: Rule[]) => Promise<number>;
}

export const useSmartStore = create<SmartState>((set, get) => ({
  collections: [],
  activeId: null,
  activeRules: null,
  activePapers: [],

  loadAll: async () => {
    const list = await api.listSmartCollections();
    set({ collections: list });
  },
  setActive: async (id) => {
    if (!id) { set({ activeId: null, activeRules: null, activePapers: [] }); return; }
    const papers = await api.listPapersBySmart(id);
    set({ activeId: id, activeRules: null, activePapers: papers });
  },
  setActiveRules: async (rules) => {
    if (!rules) { set({ activeRules: null, activePapers: [] }); return; }
    const papers = await api.previewSmartCollection(rules);
    set({ activeId: null, activeRules: rules, activePapers: papers });
  },
  create: async (sc) => {
    const out = await api.createSmartCollection(
      sc.name, sc.rules, sc.sort_by, sc.sort_dir, sc.icon ?? undefined
    );
    await get().loadAll();
    return out;
  },
  remove: async (id) => {
    await api.deleteSmartCollection(id);
    await get().loadAll();
    if (get().activeId === id) await get().setActive(null);
  },
  count: async (rules) => api.countSmartCollection(rules),
}));
```

- [ ] **Step 2: 提交**

```bash
git add src/stores/smart.ts
git commit -m "feat(store): smart collections store"
```

### Task 8.3: ValueInput / RuleRow / HitPreview / Editor

每个组件 1 文件，由于篇幅限制，下面给精简的实现要点（保持完整 React + TS）。

**Files:** Create `src/components/smart/{ValueInput,RuleRow,HitPreview,SmartCollectionEditor}.tsx`

- [ ] **Step 1: ValueInput.tsx**（按 field 类型自适应）

```tsx
import { Input } from "@/components/ui/input";

const STATUS = ["未读","阅读中","已读","重点重读"];

export function ValueInput({ field, value, onChange }: {
  field: string; value: unknown; onChange: (v: unknown) => void;
}) {
  if (field === "status") {
    return (
      <select className="h-8 rounded border border-input bg-background px-2 text-xs"
              value={value as string ?? ""}
              onChange={(e) => onChange(e.target.value)}>
        {STATUS.map(s => <option key={s} value={s}>{s}</option>)}
      </select>
    );
  }
  if (field === "year" || field === "rating") {
    return <Input type="number" value={value as number ?? ""} className="h-8 w-20"
                  onChange={(e) => onChange(Number(e.target.value))} />;
  }
  if (field === "has_note") {
    return (
      <select className="h-8 rounded border border-input bg-background px-2 text-xs"
              value={String(value)} onChange={(e) => onChange(e.target.value === "true")}>
        <option value="true">有笔记</option>
        <option value="false">无笔记</option>
      </select>
    );
  }
  if (field === "keywords" || field === "tags") {
    const arr = Array.isArray(value) ? (value as string[]) : (typeof value === "string" ? [value] : []);
    return (
      <Input
        value={arr.join(", ")}
        placeholder="多个值用逗号分隔"
        className="h-8 flex-1"
        onChange={(e) => onChange(e.target.value.split(/[,，]/).map(s => s.trim()).filter(Boolean))}
      />
    );
  }
  return <Input value={value as string ?? ""} className="h-8 flex-1"
                onChange={(e) => onChange(e.target.value)} />;
}
```

- [ ] **Step 2: RuleRow.tsx**

```tsx
import { Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { ValueInput } from "./ValueInput";
import type { Rule } from "@/types";

const FIELD_LABELS: Record<string, string> = {
  status: "状态", year: "年份", rating: "评分", tags: "标签",
  keywords: "关键词", authors: "作者", title: "标题", venue: "期刊/会议",
  created_at: "添加时间", updated_at: "修改时间",
  index_status: "索引状态", has_note: "是否有笔记",
};
const FIELD_DEFAULT_OP: Record<string, string> = {
  status: "=", year: ">=", rating: ">=", tags: "contains",
  keywords: "contains", authors: "contains", title: "contains",
  venue: "contains", created_at: "last_n_days",
  updated_at: "last_n_days", index_status: "=", has_note: "=",
};
const OP_LABELS: Record<string, string> = {
  "=": "=", "!=": "≠", ">=": "≥", "<=": "≤", "between": "之间",
  contains: "包含", not_contains: "不含", last_n_days: "最近 N 天",
};

export function RuleRow({ rule, onChange, onRemove }: {
  rule: Rule; onChange: (r: Rule) => void; onRemove: () => void;
}) {
  const isMulti = (rule.field === "keywords" || rule.field === "tags") &&
                  Array.isArray(rule.value) && (rule.value as unknown[]).length > 1;
  return (
    <div className="flex items-center gap-2">
      <select className="h-8 rounded border border-input bg-background px-2 text-xs"
              value={rule.field}
              onChange={(e) => {
                const f = e.target.value;
                onChange({ ...rule, field: f, op: FIELD_DEFAULT_OP[f] ?? "=" });
              }}>
        {Object.entries(FIELD_LABELS).map(([k, v]) =>
          <option key={k} value={k}>{v}</option>)}
      </select>
      <span className="text-xs text-muted-foreground">{OP_LABELS[rule.op] ?? rule.op}</span>
      {isMulti && (
        <select className="h-8 rounded border border-input bg-background px-2 text-xs"
                value={rule.match ?? "any"}
                onChange={(e) => onChange({ ...rule, match: e.target.value as "any"|"all" })}>
          <option value="any">任一</option>
          <option value="all">全部</option>
        </select>
      )}
      <ValueInput field={rule.field} value={rule.value}
                  onChange={(v) => onChange({ ...rule, value: v })} />
      <Button size="icon" variant="ghost" onClick={onRemove}>
        <Trash2 className="h-3.5 w-3.5" />
      </Button>
    </div>
  );
}
```

- [ ] **Step 3: HitPreview.tsx**

```tsx
import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import type { Rule } from "@/types";
import { useSmartStore } from "@/stores/smart";

export function HitPreview({ rules }: { rules: Rule[] }) {
  const count = useSmartStore((s) => s.count);
  const [n, setN] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  useEffect(() => {
    const t = setTimeout(async () => {
      setLoading(true);
      try { setN(await count(rules)); } catch { setN(null); }
      finally { setLoading(false); }
    }, 200);
    return () => clearTimeout(t);
  }, [JSON.stringify(rules), count]);
  return (
    <div className="text-xs text-muted-foreground">
      命中 {loading ? <Loader2 className="inline h-3 w-3 animate-spin" /> : (n ?? "—")} 篇
    </div>
  );
}
```

- [ ] **Step 4: SmartCollectionEditor.tsx**

```tsx
import { useState } from "react";
import { Plus } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RuleRow } from "./RuleRow";
import { HitPreview } from "./HitPreview";
import { useSmartStore } from "@/stores/smart";
import { useUIStore } from "@/stores/ui";
import type { Rule, SmartCollection } from "@/types";

export function SmartCollectionEditor({ initial, onClose }: {
  initial?: SmartCollection; onClose: () => void;
}) {
  const create = useSmartStore((s) => s.create);
  const showToast = useUIStore((s) => s.showToast);
  const [name, setName] = useState(initial?.name ?? "");
  const [rules, setRules] = useState<Rule[]>(initial?.rules ?? [
    { field: "status", op: "=", value: "未读" }
  ]);

  async function save() {
    if (!name.trim()) { showToast("warning", "请输入名称"); return; }
    try {
      await create({ name, rules, sort_by: "updated_at", sort_dir: "desc", icon: "🔬" });
      showToast("success", "已保存");
      onClose();
    } catch (e) {
      showToast("error", `保存失败: ${(e as Error).message}`);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader><DialogTitle>新建智能集合</DialogTitle></DialogHeader>
        <div className="space-y-3">
          <div>
            <Label className="mb-1 block text-xs">名称</Label>
            <Input value={name} onChange={(e) => setName(e.target.value)} />
          </div>
          <div>
            <Label className="mb-1 block text-xs">条件（满足全部）</Label>
            <div className="space-y-2">
              {rules.map((r, i) => (
                <RuleRow key={i} rule={r}
                  onChange={(nr) => setRules(rules.map((x, j) => i === j ? nr : x))}
                  onRemove={() => setRules(rules.filter((_, j) => j !== i))} />
              ))}
            </div>
            <Button size="sm" variant="outline" className="mt-2"
                    onClick={() => setRules([...rules, { field:"status", op:"=", value:"未读" }])}>
              <Plus className="mr-1 h-3 w-3" /> 添加条件
            </Button>
          </div>
          <HitPreview rules={rules} />
        </div>
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>取消</Button>
          <Button onClick={save}>保存</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 5: 编译并提交**

```bash
pnpm typecheck
git add src/components/smart/*.tsx
git commit -m "feat(ui): smart collection editor + rule row + hit preview"
```

---

## P9 — 前端 CollectionsPane 三段（Smart / Keywords / Tags）

### Task 9.1: SmartCollectionsSection

**Files:**
- Create: `src/components/library/SmartCollectionsSection.tsx`

- [ ] **Step 1: 写组件**

```tsx
import { useEffect, useState } from "react";
import { FolderPlus } from "lucide-react";
import { useSmartStore } from "@/stores/smart";
import { SmartCollectionEditor } from "@/components/smart/SmartCollectionEditor";
import { cn } from "@/lib/utils";

export function SmartCollectionsSection() {
  const list = useSmartStore((s) => s.collections);
  const activeId = useSmartStore((s) => s.activeId);
  const setActive = useSmartStore((s) => s.setActive);
  const loadAll = useSmartStore((s) => s.loadAll);
  const remove = useSmartStore((s) => s.remove);
  const [adding, setAdding] = useState(false);

  useEffect(() => { loadAll(); }, [loadAll]);
  const builtin = list.filter(s => s.is_builtin);
  const user = list.filter(s => !s.is_builtin);

  return (
    <>
      <div className="mb-2 mt-3 flex items-center justify-between px-2 text-xs font-medium text-muted-foreground">
        <span>⭐ 智能集合</span>
        <button onClick={() => setAdding(true)} className="rounded p-0.5 hover:bg-accent">
          <FolderPlus className="h-3.5 w-3.5" />
        </button>
      </div>
      {[...builtin, ...user].map((s) => (
        <button key={s.id} onClick={() => setActive(s.id)}
          className={cn("flex w-full items-center gap-2 rounded px-2 py-1.5 text-left text-sm hover:bg-accent",
            activeId === s.id && "bg-accent")}>
          <span>{s.icon ?? "🔬"}</span>
          <span className="flex-1 truncate">{s.name}</span>
          {!s.is_builtin && (
            <span className="opacity-0 hover:opacity-100"
                  onClick={(e) => { e.stopPropagation(); if(confirm(`删除 ${s.name}？`)) remove(s.id); }}>
              ✕
            </span>
          )}
        </button>
      ))}
      {adding && <SmartCollectionEditor onClose={() => setAdding(false)} />}
    </>
  );
}
```

- [ ] **Step 2: 提交**

```bash
git add src/components/library/SmartCollectionsSection.tsx
git commit -m "feat(ui): SmartCollectionsSection"
```

### Task 9.2: KeywordsSection / TagsSection

**Files:**
- Create: `src/components/library/KeywordsSection.tsx`、`src/components/library/TagsSection.tsx`

- [ ] **Step 1: KeywordsSection（结构对称，写一份即可，TagsSection 改 field）**

```tsx
import { useEffect, useState } from "react";
import { Pin } from "lucide-react";
import { api } from "@/lib/api";
import { useSmartStore } from "@/stores/smart";
import type { KeywordCount } from "@/types";

export function KeywordsSection() {
  const [items, setItems] = useState<KeywordCount[]>([]);
  const [showAll, setShowAll] = useState(false);
  const setActiveRules = useSmartStore((s) => s.setActiveRules);
  const loadAll = useSmartStore((s) => s.loadAll);

  useEffect(() => {
    api.listKeywordsWithCount(showAll ? 100 : 15).then(setItems);
  }, [showAll]);

  if (items.length === 0) return null;

  return (
    <>
      <div className="mb-2 mt-3 px-2 text-xs font-medium text-muted-foreground">🏷️ 关键词</div>
      {items.map((it) => (
        <div key={it.keyword} className="group flex items-center gap-2 rounded px-2 py-1.5 hover:bg-accent">
          <button className="flex-1 text-left text-sm"
                  onClick={() => setActiveRules([{ field:"keywords", op:"contains", value:[it.keyword], match:"any" }])}>
            {it.keyword} <span className="text-xs text-muted-foreground">({it.count})</span>
          </button>
          <button className="opacity-0 group-hover:opacity-100"
                  title="固定为智能集合"
                  onClick={async () => { await api.pinKeywordAsCollection(it.keyword); await loadAll(); }}>
            <Pin className="h-3 w-3" />
          </button>
        </div>
      ))}
      <button className="px-2 text-xs text-muted-foreground hover:underline"
              onClick={() => setShowAll(!showAll)}>
        {showAll ? "收起" : "显示更多"}
      </button>
    </>
  );
}
```

`TagsSection.tsx` 复制并把 `listKeywordsWithCount` 改 `listTagsWithCount`，标题 `🔖 标签`，rules 字段改 `tags`。

- [ ] **Step 2: 提交**

```bash
git add src/components/library/KeywordsSection.tsx src/components/library/TagsSection.tsx
git commit -m "feat(ui): KeywordsSection / TagsSection"
```

### Task 9.3: CollectionsPane 整合三段

**Files:**
- Modify: `src/components/library/CollectionsPane.tsx`

- [ ] **Step 1: 在 v1 已有内容下追加**

```tsx
import { SmartCollectionsSection } from "./SmartCollectionsSection";
import { KeywordsSection } from "./KeywordsSection";
import { TagsSection } from "./TagsSection";

// 在 component 末尾的根 <div> 内部追加：
<SmartCollectionsSection />
<KeywordsSection />
<TagsSection />
```

并在切换"阅读状态" / 普通"集合" 时调 `useSmartStore.setActive(null)` 互斥；切到 SmartCollections 时清掉 `paper.statusFilter`。

- [ ] **Step 2: paper store 加路径**

修改 `src/stores/paper.ts` 的 `loadPapers`：

```ts
loadPapers: async () => {
  const smart = useSmartStore.getState();
  if (smart.activeId || smart.activeRules) {
    set({ papers: smart.activePapers, isLoading: false });
    return;
  }
  // 原有 v1 逻辑
}
```

并订阅 smart store 变化：

```ts
useSmartStore.subscribe((s, prev) => {
  if (s.activePapers !== prev.activePapers) {
    usePaperStore.setState({ papers: s.activePapers });
  }
});
```

- [ ] **Step 3: 编译并提交**

```bash
pnpm typecheck
git add src/components/library/CollectionsPane.tsx src/stores/paper.ts
git commit -m "feat(ui): integrate smart collections / keywords / tags into pane"
```

---

## P10 — 前端 TopicReviewDialog + TopicNotePicker

### Task 10.1: TopicNotePicker

**Files:**
- Create: `src/components/topic/TopicNotePicker.tsx`

- [ ] **Step 1: 写**

```tsx
import { useEffect, useState } from "react";
import { api } from "@/lib/api";
import type { TopicNote } from "@/types";

interface Props {
  mode: "existing" | "new";
  selectedId?: string;
  newTitle: string;
  onSelectExisting: (id: string) => void;
  onChangeTitle: (t: string) => void;
}

export function TopicNotePicker({ mode, selectedId, newTitle, onSelectExisting, onChangeTitle }: Props) {
  const [list, setList] = useState<TopicNote[]>([]);
  useEffect(() => { api.listTopicNotes().then(setList); }, []);

  if (mode === "existing") {
    return (
      <select className="h-8 w-full rounded border border-input bg-background px-2 text-sm"
              value={selectedId ?? ""} onChange={(e) => onSelectExisting(e.target.value)}>
        <option value="">— 选择主题笔记 —</option>
        {list.map(n => <option key={n.id} value={n.id}>{n.title}</option>)}
      </select>
    );
  }
  return (
    <input className="h-8 w-full rounded border border-input bg-background px-2 text-sm"
           placeholder="输入新主题名"
           value={newTitle} onChange={(e) => onChangeTitle(e.target.value)} />
  );
}
```

- [ ] **Step 2: 提交**

```bash
git add src/components/topic/TopicNotePicker.tsx
git commit -m "feat(ui): TopicNotePicker"
```

### Task 10.2: TopicReviewDialog

**Files:**
- Create: `src/components/topic/TopicReviewDialog.tsx`

- [ ] **Step 1: 写**

```tsx
import { useState } from "react";
import { Loader2, Sparkles, AlertTriangle } from "lucide-react";
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { TopicNotePicker } from "./TopicNotePicker";
import { api } from "@/lib/api";
import { useUIStore } from "@/stores/ui";

interface Props {
  paperIds: string[];
  onClose: () => void;
  onDone?: () => void;
}

export function TopicReviewDialog({ paperIds, onClose, onDone }: Props) {
  const showToast = useUIStore((s) => s.showToast);
  const [mode, setMode] = useState<"existing" | "new">("new");
  const [noteId, setNoteId] = useState<string>("");
  const [newTitle, setNewTitle] = useState("");
  const [systemOverride, setSystemOverride] = useState("");
  const [showPrompt, setShowPrompt] = useState(false);
  const [busy, setBusy] = useState(false);

  async function run() {
    if (mode === "existing" && !noteId) { showToast("warning", "请选择主题笔记"); return; }
    if (mode === "new" && !newTitle.trim()) { showToast("warning", "请输入主题名"); return; }
    setBusy(true);
    try {
      const r = await api.generateTopicReview({
        paperIds,
        noteId: mode === "existing" ? noteId : undefined,
        newTitle: mode === "new" ? newTitle : undefined,
        presetId: "topic_literature_review",
        promptOverrides: systemOverride.trim() ? { system: systemOverride } : undefined,
      });
      showToast("success", `综述已写入 ${r.note.title}（${r.token_estimate} tokens）`);
      onDone?.();
    } catch (e) {
      showToast("error", `生成失败: ${(e as Error).message}`);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-lg">
        <DialogHeader><DialogTitle>生成主题综述</DialogTitle></DialogHeader>

        <div className="space-y-3 text-sm">
          <div>
            <Label className="mb-1 block text-xs">主题笔记</Label>
            <div className="mb-2 flex gap-3">
              <label className="flex items-center gap-1">
                <input type="radio" checked={mode==="new"} onChange={() => setMode("new")} />
                新建
              </label>
              <label className="flex items-center gap-1">
                <input type="radio" checked={mode==="existing"} onChange={() => setMode("existing")} />
                选择已有
              </label>
            </div>
            <TopicNotePicker mode={mode} selectedId={noteId} newTitle={newTitle}
              onSelectExisting={setNoteId} onChangeTitle={setNewTitle} />
          </div>

          <div className="rounded border border-border bg-muted/30 p-2 text-xs">
            <div>{paperIds.length} 篇论文 · 元数据 + 用户笔记</div>
            {paperIds.length > 15 && (
              <div className="mt-1 flex items-center gap-1 text-yellow-600 dark:text-yellow-400">
                <AlertTriangle className="h-3 w-3" />
                超过 15 篇可能 token 超限，建议先用智能集合筛选
              </div>
            )}
          </div>

          <div>
            <button className="text-xs text-primary hover:underline"
                    onClick={() => setShowPrompt(!showPrompt)}>
              {showPrompt ? "▾" : "▸"} 临时修改 system prompt
            </button>
            {showPrompt && (
              <Textarea rows={4} className="mt-1"
                value={systemOverride} placeholder="留空使用默认提示词"
                onChange={(e) => setSystemOverride(e.target.value)} />
            )}
          </div>
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>取消</Button>
          <Button onClick={run} disabled={busy}>
            {busy ? <Loader2 className="mr-1.5 h-4 w-4 animate-spin" />
                  : <Sparkles className="mr-1.5 h-4 w-4" />}
            开始生成
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

- [ ] **Step 2: 提交**

```bash
pnpm typecheck
git add src/components/topic/TopicReviewDialog.tsx
git commit -m "feat(ui): TopicReviewDialog"
```

---

## P11 — 兼容性回归

### Task 11.1: v1 单篇 AI / 搜索 / 详情面板

- [ ] **Step 1: 手动场景测试**（v1 既有功能）

启动 `pnpm tauri:dev`，按以下顺序检查：

| 场景 | 预期 |
|---|---|
| 导入一个 PDF | 列表立即出现（v1 行为） |
| 单篇详情 → 提取元数据 | 候选元数据正常返回 |
| 单篇详情 → 翻译摘要 | 写入笔记 AI 区块 |
| 搜索 backdoor | 命中结果正确 |
| 切到智能集合 "未读论文" | 列表只剩未读 |
| 切回 "全部论文" | 列表恢复 |

- [ ] **Step 2: 跑全测试**

```bash
cd src-tauri && cargo test --lib
cd .. && pnpm test
```

Expected: 全绿

- [ ] **Step 3: 提交（如有修复）**

```bash
git commit -am "test: v1 regression after v1.5"
```

---

## P12 — 收尾文档

### Task 12.1: ACCEPTANCE_v1.5.md

- [ ] **Step 1: 写**

按 v1 的 ACCEPTANCE 同结构，列 P0–P12 的勾选与测试场景，路径 `docs/paper-vault/ACCEPTANCE_v1.5.md`。

- [ ] **Step 2: 更新 README.md "v1.5" 段（已有 Roadmap，把 v1.5 内容标完成）**

- [ ] **Step 3: 提交**

```bash
git add docs/paper-vault/ACCEPTANCE_v1.5.md README.md
git commit -m "docs: v1.5 acceptance + README roadmap"
```

---

## 自审清单

**Spec 覆盖**：

| SPEC 节 | 落地任务 |
|---|---|
| 4.1 左栏布局 | P9 三段 |
| 4.2 BatchToolbar | Task 7.3 |
| 4.3 SmartCollectionEditor | Task 8.3 |
| 4.4 TopicReviewDialog | Task 10.2 |
| 5.1 smart_collections 表 | Task 0.1 |
| 5.2 topic_notes 表 | Task 0.1 |
| 5.3 timeout_secs 列 | Task 0.1 + 6.1 |
| 5.4 字段白名单 | Task 1.1 |
| 5.5 内置 7 预设 | Task 2.2 |
| 6 规则→SQL | Task 1.2 |
| 7 IPC 命令（17 个） | P2/P3/P4/P5 |
| 8 主题笔记模板 | Task 4.1 |
| 9 topic_literature_review preset | Task 5.1 |
| 9.2 拼装 papers_with_notes | Task 5.2 |
| 11 与 v1 兼容性 | P11 |
| 12 风险（timeout、注入、覆盖） | Task 0.1 / 1.2 / 4.2 |

**类型一致性**：

- `Rule.match_mode` 后端字段名 `match`（serde rename），前端 TS `match` —— 对齐 ✅
- `SmartCollection.icon` 后端 `Option<String>`，前端 `string | null` —— 对齐 ✅
- `TopicReviewResult` 三方一致 ✅
- `topic_review::run` 的 `prompt_overrides` 后端结构 vs 前端 `{system?,user?}` —— 对齐 ✅

**占位符扫描**：无 TBD / TODO / "implement later" / 不带代码的步骤。

---

## Execution Handoff

实施计划已完成并保存到 `docs/paper-vault/PLAN_v1.5.md`。两种执行方式可选：

1. **Subagent-Driven（推荐）** — 每个 Task 派发独立 sub-agent，回合间审阅，迭代快
2. **Inline Execution** — 在当前会话按 P0 → P12 顺序批量执行，关键节点设置 checkpoint

请告诉我用哪种方式继续，或直接说"开始执行"我按推荐方式启动 P0。