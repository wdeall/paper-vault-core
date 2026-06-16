-- PaperVault v1 初始 schema
-- 所有 IF NOT EXISTS，跑多次不会出错。

CREATE TABLE IF NOT EXISTS papers (
  id            TEXT PRIMARY KEY,
  title         TEXT NOT NULL DEFAULT '',
  authors       TEXT NOT NULL DEFAULT '[]',
  year          INTEGER,
  venue         TEXT NOT NULL DEFAULT '',
  doi           TEXT NOT NULL DEFAULT '',
  abstract_text TEXT NOT NULL DEFAULT '',
  keywords      TEXT NOT NULL DEFAULT '[]',
  tags          TEXT NOT NULL DEFAULT '[]',
  status        TEXT NOT NULL DEFAULT '未读',
  rating        INTEGER,
  pdf_path      TEXT NOT NULL DEFAULT '',
  note_path     TEXT NOT NULL DEFAULT '',
  created_at    INTEGER NOT NULL,
  updated_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_papers_status ON papers(status);
CREATE INDEX IF NOT EXISTS idx_papers_year   ON papers(year);
CREATE INDEX IF NOT EXISTS idx_papers_doi    ON papers(doi);

CREATE TABLE IF NOT EXISTS reading_progress (
  paper_id         TEXT PRIMARY KEY REFERENCES papers(id) ON DELETE CASCADE,
  current_page     INTEGER NOT NULL DEFAULT 0,
  total_pages      INTEGER NOT NULL DEFAULT 0,
  progress_percent REAL NOT NULL DEFAULT 0,
  last_read_at     INTEGER
);

CREATE TABLE IF NOT EXISTS collections (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  parent_id   TEXT,
  created_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS paper_collections (
  paper_id      TEXT NOT NULL REFERENCES papers(id) ON DELETE CASCADE,
  collection_id TEXT NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
  PRIMARY KEY (paper_id, collection_id)
);

CREATE TABLE IF NOT EXISTS index_status (
  paper_id    TEXT PRIMARY KEY REFERENCES papers(id) ON DELETE CASCADE,
  status      TEXT NOT NULL DEFAULT '未索引',
  error       TEXT,
  indexed_at  INTEGER
);

CREATE VIRTUAL TABLE IF NOT EXISTS fulltext_index USING fts5(
  paper_id UNINDEXED,
  source_type UNINDEXED,
  content,
  page UNINDEXED,
  tokenize = 'unicode61 remove_diacritics 2'
);

CREATE TABLE IF NOT EXISTS ai_skill_presets (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  bound_action  TEXT NOT NULL,
  skill         TEXT NOT NULL,
  system_prompt TEXT NOT NULL,
  user_template TEXT NOT NULL,
  output_format TEXT NOT NULL DEFAULT 'json',
  auto_write    INTEGER NOT NULL DEFAULT 0,
  is_builtin    INTEGER NOT NULL DEFAULT 0,
  updated_at    INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS ai_provider_config (
  id          TEXT PRIMARY KEY,
  base_url    TEXT NOT NULL DEFAULT '',
  api_key     TEXT NOT NULL DEFAULT '',
  model       TEXT NOT NULL DEFAULT '',
  updated_at  INTEGER NOT NULL
);
