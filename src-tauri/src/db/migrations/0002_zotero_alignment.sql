-- =====================================================================
-- 0002_zotero_alignment.sql
--
-- P0 Zotero 数据模型对齐迁移。
--
-- 设计原则（详见 docs/paper-vault/PLAN_zotero_alignment.md §0.1 / §3）：
--   - 保留 TEXT 主键体系。
--   - 旧 `papers.authors / keywords / tags` JSON 列保留作为“逻辑废弃”
--     备份，物理删除放到 P0 最后一步或后续 P 阶段。本迁移不强制
--     DROP COLUMN，避免 SQLite ALTER TABLE 大表重建成本。
--   - `abstract` 是 SQL 关键字，旧 SQL 里如有以 `abstract` 命名的列
--     需要重命名。本迁移对 papers 增加 `abstract_text`；若旧表已存在
--     同名列则跳过（数据迁移在 Rust 端 migrate_v2.rs 中处理）。
--   - 旧 `status` 字段的 `未读/阅读中/已读/重点重读` 值正规化为
--     `unread/reading/read`（重点重读 → read），由 Rust 端
--     migrate_v2.rs 在数据搬运时执行，SQL 端仅负责 CHECK 约束。
-- =====================================================================

-- 1. creators：作者/编辑者等贡献者主表（全局共享去重）。
CREATE TABLE IF NOT EXISTS creators (
    id              TEXT PRIMARY KEY,
    family_name     TEXT NOT NULL DEFAULT '',
    given_name      TEXT NOT NULL DEFAULT '',
    display_name    TEXT NOT NULL,
    -- 兼容旧 JSON 列表里直接写 "Alice Smith" 这种扁平字符串
    -- 的情况，存储原始字符串以便向后兼容。
    raw             TEXT NOT NULL DEFAULT '',
    created_at      INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000)
);
CREATE INDEX IF NOT EXISTS idx_creators_display ON creators(display_name);
CREATE INDEX IF NOT EXISTS idx_creators_family  ON creators(family_name);

-- 2. paper_creators：论文 ↔ 贡献者 多对多（带顺序与角色）。
CREATE TABLE IF NOT EXISTS paper_creators (
    paper_id    TEXT NOT NULL,
    creator_id  TEXT NOT NULL,
    position    INTEGER NOT NULL DEFAULT 0,
    role        TEXT NOT NULL DEFAULT 'author',  -- author / editor / translator / …
    PRIMARY KEY (paper_id, creator_id, role),
    FOREIGN KEY (paper_id)   REFERENCES papers(id)   ON DELETE CASCADE,
    FOREIGN KEY (creator_id) REFERENCES creators(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_pc_paper   ON paper_creators(paper_id);
CREATE INDEX IF NOT EXISTS idx_pc_creator ON paper_creators(creator_id);

-- 3. identifiers：DOI / arXiv / PMID / ISBN / ISSN 等。
CREATE TABLE IF NOT EXISTS identifiers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    paper_id    TEXT NOT NULL,
    type        TEXT NOT NULL,         -- doi / arxiv / pmid / isbn / issn / url
    value       TEXT NOT NULL,
    is_primary  INTEGER NOT NULL DEFAULT 0,
    FOREIGN KEY (paper_id) REFERENCES papers(id) ON DELETE CASCADE,
    UNIQUE (paper_id, type, value)
);
CREATE INDEX IF NOT EXISTS idx_identifiers_paper ON identifiers(paper_id);
CREATE INDEX IF NOT EXISTS idx_identifiers_type  ON identifiers(type, value);

-- 4. keywords：关键词主表（全局共享，source 区分来源）。
CREATE TABLE IF NOT EXISTS keywords (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL UNIQUE,
    -- 'auto' = 系统/AI 提取；'manual' = 用户手工添加。
    source      TEXT NOT NULL DEFAULT 'manual',
    created_at  INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000)
);
CREATE INDEX IF NOT EXISTS idx_keywords_name ON keywords(name);

-- 5. paper_keywords：论文 ↔ 关键词 多对多。
CREATE TABLE IF NOT EXISTS paper_keywords (
    paper_id    TEXT NOT NULL,
    keyword_id  TEXT NOT NULL,
    PRIMARY KEY (paper_id, keyword_id),
    FOREIGN KEY (paper_id)   REFERENCES papers(id)   ON DELETE CASCADE,
    FOREIGN KEY (keyword_id) REFERENCES keywords(id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS idx_pk_paper   ON paper_keywords(paper_id);
CREATE INDEX IF NOT EXISTS idx_pk_keyword ON paper_keywords(keyword_id);

-- 6. attachments：PDF / 笔记 / 其它附件。
CREATE TABLE IF NOT EXISTS attachments (
    id              TEXT PRIMARY KEY,
    paper_id        TEXT NOT NULL,
    -- 'pdf' / 'note' / 'supplement' / 'snapshot' …
    kind            TEXT NOT NULL,
    rel_path        TEXT NOT NULL,            -- 相对 vault 根的路径
    abs_path        TEXT,                     -- 可选：缓存绝对路径
    mime_type       TEXT,
    title           TEXT,
    -- 笔记 frontmatter 序列化（仅 kind='note' 用）；其它 kind 留空。
    frontmatter     TEXT,
    sha256          TEXT,
    imported_at     INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
    -- 'active' / 'missing' / 'deleted'。
    status          TEXT NOT NULL DEFAULT 'active',
    FOREIGN KEY (paper_id) REFERENCES papers(id) ON DELETE CASCADE,
    CHECK (status IN ('active', 'missing', 'deleted'))
);
CREATE INDEX IF NOT EXISTS idx_attachments_paper ON attachments(paper_id);
CREATE INDEX IF NOT EXISTS idx_attachments_kind   ON attachments(paper_id, kind);

-- 7. paper_relations：论文之间的引用 / 关联。
CREATE TABLE IF NOT EXISTS paper_relations (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    src_paper_id    TEXT NOT NULL,
    dst_paper_id    TEXT NOT NULL,
    -- 'cites' / 'cited_by' / 'related' / 'replaces' …
    relation        TEXT NOT NULL DEFAULT 'related',
    note            TEXT,
    created_at      INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
    FOREIGN KEY (src_paper_id) REFERENCES papers(id) ON DELETE CASCADE,
    FOREIGN KEY (dst_paper_id) REFERENCES papers(id) ON DELETE CASCADE,
    UNIQUE (src_paper_id, dst_paper_id, relation)
);
CREATE INDEX IF NOT EXISTS idx_relations_src ON paper_relations(src_paper_id);
CREATE INDEX IF NOT EXISTS idx_relations_dst ON paper_relations(dst_paper_id);

-- 8. annotations：PDF / 笔记高亮与批注。
CREATE TABLE IF NOT EXISTS annotations (
    id              TEXT PRIMARY KEY,
    paper_id        TEXT NOT NULL,
    attachment_id   TEXT,
    -- 'highlight' / 'note' / 'underline' / 'strike' / 'image' …
    kind            TEXT NOT NULL,
    page            INTEGER,
    rect            TEXT,                       -- JSON 序列化的矩形
    color           TEXT,
    text            TEXT,
    comment         TEXT,
    created_at      INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
    modified_at     INTEGER,
    FOREIGN KEY (paper_id)      REFERENCES papers(id)      ON DELETE CASCADE,
    FOREIGN KEY (attachment_id) REFERENCES attachments(id)  ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_annotations_paper      ON annotations(paper_id);
CREATE INDEX IF NOT EXISTS idx_annotations_attachment ON annotations(attachment_id);

-- 9. merge_log：合并 / 重复合并审计。
CREATE TABLE IF NOT EXISTS merge_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    -- 保留的主论文。
    canonical_id    TEXT NOT NULL,
    -- 被合并掉的论文。
    duplicate_id    TEXT NOT NULL,
    -- 哪些字段被覆盖（JSON 数组）。
    fields_merged   TEXT,
    -- 合并前自动备份的快照 JSON。
    snapshot        TEXT,
    merged_at       INTEGER NOT NULL DEFAULT (CAST(strftime('%s','now') AS INTEGER) * 1000),
    merged_by       TEXT,
    FOREIGN KEY (canonical_id) REFERENCES papers(id) ON DELETE SET NULL
);
CREATE INDEX IF NOT EXISTS idx_merge_log_canonical ON merge_log(canonical_id);
CREATE INDEX IF NOT EXISTS idx_merge_log_duplicate ON merge_log(duplicate_id);

-- 10. papers_fts：FTS5 虚拟表（仅元数据，PDF 内容由 fulltext_index
--     维护，本表不重复）。
CREATE VIRTUAL TABLE IF NOT EXISTS papers_fts USING fts5(
    paper_id UNINDEXED,
    title,
    abstract,
    authors,
    keywords,
    venue,
    doi,
    tokenize = "unicode61 remove_diacritics 2"
);

-- 11. 状态 CHECK 约束。
--     SQLite 不支持直接给已有列加 CHECK 约束；这里我们用触发器
--     兜底，确保写入时 status 必须是允许值之一。
DROP TRIGGER IF EXISTS trg_papers_status_check_ins;
DROP TRIGGER IF EXISTS trg_papers_status_check_upd;
CREATE TRIGGER IF NOT EXISTS trg_papers_status_check_ins
BEFORE INSERT ON papers
FOR EACH ROW
WHEN NEW.status IS NOT NULL
     AND NEW.status NOT IN ('unread', 'reading', 'read')
BEGIN
    SELECT RAISE(ABORT, 'papers.status must be one of unread|reading|read');
END;
CREATE TRIGGER IF NOT EXISTS trg_papers_status_check_upd
BEFORE UPDATE ON papers
FOR EACH ROW
WHEN NEW.status IS NOT NULL
     AND NEW.status NOT IN ('unread', 'reading', 'read')
BEGIN
    SELECT RAISE(ABORT, 'papers.status must be one of unread|reading|read');
END;

-- 12. keywords.source CHECK 约束（M-B 加固：M-A 阶段漏写，触发器兜底）。
DROP TRIGGER IF EXISTS trg_keywords_source_check_ins;
DROP TRIGGER IF EXISTS trg_keywords_source_check_upd;
CREATE TRIGGER IF NOT EXISTS trg_keywords_source_check_ins
BEFORE INSERT ON keywords
FOR EACH ROW
WHEN NEW.source IS NOT NULL
     AND NEW.source NOT IN ('auto', 'manual')
BEGIN
    SELECT RAISE(ABORT, 'keywords.source must be one of auto|manual');
END;
CREATE TRIGGER IF NOT EXISTS trg_keywords_source_check_upd
BEFORE UPDATE ON keywords
FOR EACH ROW
WHEN NEW.source IS NOT NULL
     AND NEW.source NOT IN ('auto', 'manual')
BEGIN
    SELECT RAISE(ABORT, 'keywords.source must be one of auto|manual');
END;
