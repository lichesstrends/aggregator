-- Aggregated results by month + opening + 100-pt elo buckets
CREATE TABLE IF NOT EXISTS aggregates (
  month        TEXT NOT NULL,
  opening      TEXT NOT NULL,
  white_bucket INTEGER NOT NULL,
  black_bucket INTEGER NOT NULL,
  games        INTEGER NOT NULL,
  white_wins   INTEGER NOT NULL,
  black_wins   INTEGER NOT NULL,
  draws        INTEGER NOT NULL,
  white_pct    REAL NOT NULL,
  black_pct    REAL NOT NULL,
  draw_pct     REAL NOT NULL,
  PRIMARY KEY (month, opening, white_bucket, black_bucket)
);