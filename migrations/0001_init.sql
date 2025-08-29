-- Aggregated results by month + opening ECO code + 200-pt elo buckets
CREATE TABLE IF NOT EXISTS aggregates (
  month        TEXT NOT NULL,
  eco_group    TEXT NOT NULL,       -- e.g., B20, C00, E60, or U00 if unknown
  white_bucket INTEGER NOT NULL,    -- lower bound of bucket (e.g., 2200)
  black_bucket INTEGER NOT NULL,    -- lower bound of bucket (e.g., 2200)
  games        INTEGER NOT NULL,
  white_wins   INTEGER NOT NULL,
  black_wins   INTEGER NOT NULL,
  draws        INTEGER NOT NULL,
  PRIMARY KEY (month, eco_group, white_bucket, black_bucket)
);