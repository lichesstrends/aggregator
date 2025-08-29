-- Track which months were successfully ingested (and basic stats).
CREATE TABLE IF NOT EXISTS ingestions (
  month        TEXT PRIMARY KEY,   -- "YYYY-MM"
  url          TEXT NOT NULL,
  started_at   TEXT,               -- ISO8601
  finished_at  TEXT,               -- ISO8601
  games        INTEGER DEFAULT 0,
  duration_ms  INTEGER DEFAULT 0,
  status       TEXT NOT NULL       -- 'success' | 'failed' | 'started'
);