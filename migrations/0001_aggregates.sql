-- Aggregated results by month + opening ECO code + 200-pt elo buckets
CREATE TABLE IF NOT EXISTS aggregates (
  month        VARCHAR(7)  NOT NULL, -- "YYYY-MM"
  eco_group    VARCHAR(16) NOT NULL, -- e.g., B20, C00, E60, or U00 if unknown
  white_bucket INTEGER     NOT NULL, -- lower bound of bucket (e.g., 2200)
  black_bucket INTEGER     NOT NULL, -- lower bound of bucket (e.g., 2200)
  games        BIGINT      NOT NULL,
  white_wins   BIGINT      NOT NULL,
  black_wins   BIGINT      NOT NULL,
  draws        BIGINT      NOT NULL,
  PRIMARY KEY (month, eco_group, white_bucket, black_bucket)
);