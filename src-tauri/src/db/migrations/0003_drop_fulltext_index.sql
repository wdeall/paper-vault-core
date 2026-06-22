-- 0003_drop_fulltext_index.sql
--
-- P3: 移除旧的 fulltext_index (含 PDF/notes 全文),搜索切到 papers_fts (metadata-only)。
-- index_status 表保留 (reindex 仍用它追踪状态)。
DROP TABLE IF EXISTS fulltext_index;
