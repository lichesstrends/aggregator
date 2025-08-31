-- Track which months were successfully ingested (and basic stats).
CREATE TABLE IF NOT EXISTS ingestions (
  month       VARCHAR(7)   PRIMARY KEY, -- "YYYY-MM"
  url         TEXT         NOT NULL,
  started_at  TEXT,
  finished_at TEXT,
  games       BIGINT       DEFAULT 0,
  duration_ms BIGINT       DEFAULT 0,
  status      VARCHAR(16)  NOT NULL     -- 'success' | 'failed' | 'started'
);