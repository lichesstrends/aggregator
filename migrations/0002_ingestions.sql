-- Track which files (remote or local) were successfully ingested with their hash and basic info..
CREATE TABLE IF NOT EXISTS ingestions (
  hash        VARCHAR(64)  PRIMARY KEY, -- sha256 hex of the compressed file
  url         TEXT         NOT NULL,    -- remote URL or local file path
  started_at  TEXT,
  finished_at TEXT,
  games       BIGINT       DEFAULT 0,
  duration_ms BIGINT       DEFAULT 0,
  status      VARCHAR(16)  NOT NULL     -- 'success' | 'failed' | 'started'
);