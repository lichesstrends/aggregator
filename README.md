![LichessTrends logo](docs/lichesstrends.svg "LichessTrends logo")

The **LichessTrends Aggregator** is a fast, streaming **Rust** tool that turns the massive monthly **Lichess** PGN dumps (available at [database.lichess.org](database.lichess.org)) into compact, queryable statistics. It:

- **fetches** remote monthly PGN dumps from **Lichess** (or reads your local `.pgn.zst` files);
- **streams** and decodes them on the fly (no giant temp files);
- **aggregates** games with the same :
  - **Month played** (e.g. `2013-07`),
  - **ECO code group** (e.g. `B20-B99`→ Sicilian defence; `A56`→ Benoni defence),
  - **White Elo bucket** (default size `200`),
  - **Black Elo bucket** (default size `200`);
  - for each of these aggregates, the following counts are stored : `games`, `white_wins`, `black_wins` and `draws`.

It's nice because :
- The generated aggregates let you compute many kinds of high-level stats : opening popularity, win/draw rates, performance by Elo buckets, and trends over time.
- It’s blazing fast and designed for scale, with huge dump files streaming on the fly, parallel parsing and aggregates computation in batches.
- You can save your results as CSVs or to your remote database. By default, it's a dry-run that doesn't save anything unless you say so.

## Prerequisites
- **Docker** (for building)

## Building

```bash
./build.sh
```

This produces a statically-linked Linux binary at `./target/lta` that runs on any Linux system (including WSL).

## Getting started
A sample dump is included in the repo at `sample/lichess_sample.pgn.zst`.

```bash
# Build first
./build.sh

# Show help
./target/lta -h

# Run the sample (dry-run: no database/disk writes)
./target/lta sample/lichess_sample.pgn.zst
```

### Other usage examples
**Remote stream from Lichess:**
```bash
# Oldest → newest, stop at (and include) 2013-02
./target/lta --remote --until 2013-02 -v

# Range selection (inclusive on both ends): from 2015-01 to 2015-03
./target/lta --remote --since 2015-01 --until 2015-03 -v
```

**Remote stream from Lichess and save to local SQLite (creates ./data/lichess.db)**
```bash
cp .env.example .env       # defaults to local SQLite
./target/lta --save --remote --until 2013-02 -v
```

**Write aggregated CSVs:**
```bash
# One CSV per dump will be written into ./out/
./target/lta --remote --until 2013-02 --out out/ -v
```

> In **local mode**, `--out` may be a **file** (single CSV) or a **directory** (one CSV per input).  
> In **remote mode**, `--out` is usually a **directory** (one CSV per dump).

The produced CSV will have the following columns:
```
month,eco_group,white_bucket,black_bucket,games,white_wins,black_wins,draws
```

Here is an example row:
```
2013-05,C00-C19,1600,1400,523,280,180,63
```

> This means: In **May 2013** on Lichess, for games in the **C00-C19 ECO group** (French Defence family) where **White was rated in the 1600–1799 bucket** and **Black in the 1400–1599 bucket**, there were a total of **523 games**. Out of these, **White won 280**, **Black won 180**, and **63 were draws**.


## ⚙️How it works
### 1. Streaming pipeline
- **Remote mode**: The app streams each monthly `*.pgn.zst` over HTTP and pipes it through a `zstd` decoder. There’s no need to store the whole file on disk.
- **Local mode**: The app decompresses the `.zst` you already have and streams it into the app.

### 2. Processing in batches
- Each PGN stream is divided into **game batches** (configurable). Each batch is parsed and aggregated in parallel (Rayon), then merged into a single in-memory map keyed by `(month, eco_group, white_bucket, black_bucket)`.

### 3. Database (optional)
- With `--save`, results are persisted using **SQLx** either to a **local SQLite file** or to a remote database depending on your `DATABASE_URL` (**Postgres** and **MySQL** backends are supported). Batched upserts and transactions are used for speed.
- Without `--save` → **no DB connections or writes**
- **Upserts are additive**: reprocessing the same file is safe because we **skip it by hash**; mixing multiple files that cover overlapping months/slices naturally **accumulates**.

> Note on months: a monthly dump can contain a small tail of games from the previous month (UTC edge). We always use the **PGN’s own `UTCDate/Date`** to attribute each game to a month.

The following tables are created (if not already present) when saving:
#### `aggregates` - aggregated counts
**PRIMARY KEY** (`month`, `eco_group`, `white_bucket`, `black_bucket`)
Column | Type | Description | Example value
-- | -- | -- | --
`month` | VARCHAR(7) | The month of the aggregate | `2025-09`
`eco_group` | VARCHAR(16) | The opening ECO code of the aggregate | `B20-B99`, `C00-C19`
`white_bucket` | INTEGER | The lower bound of the white ELO bucket | `2200`
`black_bucket` | INTEGER | The lower bound of the black ELO bucket | `2000`
`games` | BIGINT | The number of games in the aggregate | `123`
`white_wins` | BIGINT | The number of white wins in the aggregate games | `101`
`black_wins` | BIGINT | The number of black wins in the aggregate games | `15`
`draws` | BIGINT | The number of draws in the aggregate games | `7`

#### `ingestions` - hash-keyed ingestions
**PRIMARY KEY** (`hash`)
Column | Type | Description | Example value
-- | -- | -- | --
`hash` | VARCHAR(64) | SHA-256 of the ingestion file | `03c387ed...` 
`url` | TEXT NOT NULL | Remote URL or local file path | `https://database.lichess.org/...pgn.zst`
`started_at` | TEXT | ISO 8601 timestamp of the ingestion start time | `2025-09-07T19:21:50.161093049+00:00`
`finished_at` | TEXT | ISO 8601 timestamp of the ingestion finish time | `2025-09-07T19:21:50.161093049+00:00`
`games` | BIGINT DEFAULT 0 | The number of games processed in this ingestion | `795173`
`duration_ms` | BIGINT DEFAULT 0 | The ingestion duration in milliseconds | `15144`
`status` | VARCHAR(16) NOT NULL | The ingestion status | `started`, `success` or `failed`



- Both **remote** and **local** runs write an entry here when `--save` is used.
- **Dedup**: Before processing a remote/local file, we check if its **hash** already exists with `status='success'`. If yes, we skip it. This prevents doing the same work twice.

> Another table is also created : `_sqlx_migrations`, an internal table used by SQLx to record executed migrations.

You can reset your local SQLite to start fresh:
```bash
rm -f data/lichess.db data/lichess.db-wal data/lichess.db-shm
```

## Local mode (details)
Use a local `.pgn.zst` file you already have (no extraction needed).

```bash
# Count games (dry-run)
./target/lta path/to/lichess_db_standard_rated_2013-07.pgn.zst

# Count and write a single CSV
./target/lta --out out/2013-07.csv path/to/lichess_db_standard_rated_2013-07.pgn.zst

# Persist counts to local SQLite
cp .env.example .env
./target/lta --save --out out/2013-07.csv path/to/lichess_db_standard_rated_2013-07.pgn.zst
```

What you’ll see in the terminal:
- per-file timing + number of games processed;
- optional “wrote CSV” message if `--out` is set.

## Remote mode (Lichess)
The app reads `list.txt` and `sha256sums.txt` [from Lichess](https://database.lichess.org/standard/list.txt) (a list of monthly URLs along with their SHA-256 hashes), sorts **oldest → newest**, and processes dump after dump.

```bash
# Dry-run up to a given monthly dump
./target/lta --remote --until 2013-05 -v

# Dry-run with CSVs (one file per monthly dump)
./target/lta --remote --until 2013-05 --out out/ -v

# Persist results into your configured database (requires .env with DATABASE_URL)
./target/lta --remote --until 2013-05 --save -v

# Use a custom index (if you mirror Lichess)
./target/lta --remote --remote-url https://my.mirror/standard --since 2015-01 --until 2015-03
```

What you’ll see:
- per-dump timing + number of games processed;
- optional CSV write messages if `--out` is set.
- with `--save`, results are written to the DB and each processed dump is kept track of in the ingestions table.

## Remote database setup
You can push results into a remote database (**Postgres** and **MySQL** are supported). Create a `.env` file, then run with `--save`.

1) Create `.env` (mock URL example shown):
```ini
# .env
DATABASE_URL=postgresql://user:pass@host:5432/dbname?sslmode=require
DB_MAX_CONNECTIONS=10
```

2) Save results to your remote database with the `--save` CLI option :
```bash
./target/lta --save --remote --until 2013-05 -v
```

## Configuration file (`config.toml`)
All knobs live in `config.toml`:

```toml
bucket_size     = 200
remote_base_url = "https://database.lichess.org/standard" 
db_batch_rows   = 1000  
batch_size      = 1000
#rayon_threads  = 8
```

- **bucket_size**: Elo bucket width (e.g., 200 → 1200–1399, 1400–1599, …).
- **remote_base_url**: the Lichess monthly index; change if you mirror it. You can also override this at runtime using the `--remote-url` CLI flag.
- **db_batch_rows**: how many rows are inserted/updated per DB batch.
- **batch_size**: number of games processed at a time before merging.
- **rayon_threads**: set to force a specific parallelism; otherwise uses CPU count.

## CLI reference
```
# Default is DRY-RUN: no DB connection and no writes.

# Modes
--remote, --ingest-remote       Stream monthly dumps from the configured base URL
[file1.zst file2.zst ...]       Process local compressed files (positional args)

# Remote filters (for convenience only; dedupe is hash-based)
--since YYYY-MM, --from         Start from this monthly dump (inclusive)
--until YYYY-MM                 Stop after this monthly dump (inclusive)

# Output
--out, -o PATH                  CSV output
                                - local: file or directory (per input)
                                - remote: directory (one CSV per monthly dump)
# Remote base
--remote-url URL                Base URL that provides {URL}/list.txt and {URL}/sha256sums.txt

# Persistence
--save                          Persist to DATABASE_URL (runs migrations and writes)

# Misc
-v, --verbose                   Detailed timings/logs
-h, --help                      Show built-in help
```

## GitHub Actions

The aggregator runs automatically every day at midnight via GitHub Actions, downloading the latest release binary and ingesting new Lichess data.

Setup:
1. Go to repository Settings > Secrets and variables > Actions
2. Add `DATABASE_URL` secret with your database connection string

## License
This project is licensed under the terms of the MIT license. Fork it, steal it, make it better (or worse), make it yours!

## Contribution
We welcome contributions! Issues, PRs, and ideas are all appreciated.