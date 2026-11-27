CREATE TABLE usage_event (
  dedup_key        TEXT PRIMARY KEY,
  tool             TEXT NOT NULL CHECK (tool IN ('claude-code','antigravity','opencode')),
  provider         TEXT NOT NULL CHECK (provider IN ('anthropic','google','deepseek','other')),
  plan_id          TEXT NOT NULL REFERENCES plan(id),
  model            TEXT NOT NULL,
  ts_utc           TEXT NOT NULL,              -- ISO 8601 con Z
  session_id       TEXT NOT NULL,
  project          TEXT,
  input            INTEGER NOT NULL DEFAULT 0,
  cache_write      INTEGER NOT NULL DEFAULT 0,
  cache_read       INTEGER NOT NULL DEFAULT 0,
  output           INTEGER NOT NULL DEFAULT 0,
  reasoning        INTEGER NOT NULL DEFAULT 0,
  cost_usd         REAL,                       -- NULL si no hay precio
  cost_kind        TEXT NOT NULL DEFAULT 'unknown' CHECK (cost_kind IN ('real','api_equivalent','unknown')),
  price_id         INTEGER REFERENCES price(id),
  cost_computed_at TEXT,
  source_file_id   INTEGER REFERENCES source_file(id)
);
CREATE INDEX ix_event_ts      ON usage_event(ts_utc);
CREATE INDEX ix_event_session ON usage_event(session_id, ts_utc);
CREATE INDEX ix_event_model   ON usage_event(model, ts_utc);
CREATE INDEX ix_event_plan    ON usage_event(plan_id, ts_utc);

CREATE TABLE hourly_agg (
  hour_utc    TEXT NOT NULL,                   -- 'YYYY-MM-DDTHH:00:00Z'
  tool        TEXT NOT NULL,
  provider    TEXT NOT NULL,
  plan_id     TEXT NOT NULL,
  model       TEXT NOT NULL,
  project     TEXT NOT NULL DEFAULT '',
  turns       INTEGER NOT NULL DEFAULT 0,
  input       INTEGER NOT NULL DEFAULT 0,
  cache_write INTEGER NOT NULL DEFAULT 0,
  cache_read  INTEGER NOT NULL DEFAULT 0,
  output      INTEGER NOT NULL DEFAULT 0,
  cost_real   REAL NOT NULL DEFAULT 0,
  cost_equiv  REAL NOT NULL DEFAULT 0,
  unpriced    INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (hour_utc, tool, provider, plan_id, model, project)
);

CREATE TABLE source_file (
  id          INTEGER PRIMARY KEY,
  collector   TEXT NOT NULL,
  path        TEXT NOT NULL UNIQUE,
  size        INTEGER NOT NULL,
  mtime       TEXT NOT NULL,
  read_offset INTEGER NOT NULL DEFAULT 0,
  last_read   TEXT NOT NULL
);

CREATE TABLE ingest_error (
  id        INTEGER PRIMARY KEY,
  source_file_id INTEGER NOT NULL REFERENCES source_file(id) ON DELETE CASCADE,
  line      INTEGER NOT NULL,
  reason    TEXT NOT NULL,
  at        TEXT NOT NULL,
  UNIQUE (source_file_id, line)
);

CREATE TABLE price (
  id            INTEGER PRIMARY KEY,
  provider      TEXT NOT NULL,
  model_pattern TEXT NOT NULL,
  valid_from    TEXT NOT NULL,                 -- 'YYYY-MM-DD'
  valid_to      TEXT,                          -- exclusivo; NULL = vigente
  input         REAL NOT NULL,                 -- $ por millón
  output        REAL NOT NULL,
  cache_write   REAL NOT NULL,
  cache_read    REAL NOT NULL,
  source_url    TEXT NOT NULL,
  approximate   INTEGER NOT NULL DEFAULT 0,
  user_edited   INTEGER NOT NULL DEFAULT 0,
  UNIQUE (model_pattern, valid_from)
);

CREATE TABLE cost_recalc_log (
  id         INTEGER PRIMARY KEY,
  at         TEXT NOT NULL,
  price_id   INTEGER NOT NULL REFERENCES price(id),
  events     INTEGER NOT NULL,
  old_total  REAL NOT NULL,
  new_total  REAL NOT NULL
);

CREATE TABLE plan (
  id              TEXT PRIMARY KEY,            -- 'claude-pro','google-ai-pro','google-ai-pro-claude','deepseek'
  name            TEXT NOT NULL,
  provider        TEXT NOT NULL,
  kind            TEXT NOT NULL CHECK (kind IN ('subscription','payg')),
  monthly_fee_usd REAL NOT NULL DEFAULT 0,
  budget_usd      REAL,
  windows         TEXT NOT NULL DEFAULT '[]'   -- JSON: [{"name":"5h","hours":5},{"name":"weekly","hours":168}]
);

CREATE TABLE plan_calibration (
  id       INTEGER PRIMARY KEY,
  plan_id  TEXT NOT NULL REFERENCES plan(id),
  win_name TEXT NOT NULL CHECK (win_name IN ('5h','weekly')),
  at       TEXT NOT NULL,
  percent  REAL NOT NULL CHECK (percent > 0 AND percent <= 100),
  used     REAL NOT NULL,
  limit_est REAL NOT NULL,
  source   TEXT NOT NULL DEFAULT 'user' CHECK (source IN ('user','agy')),
  resets_at TEXT                              -- solo source = 'agy': reinicio oficial de la ventana
);

CREATE TABLE alert_fired (
  rule   TEXT NOT NULL,
  key    TEXT NOT NULL,
  at     TEXT NOT NULL,
  PRIMARY KEY (rule, key)
);

CREATE TABLE setting (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
