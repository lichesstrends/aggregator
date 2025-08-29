# ♔♕Lichess Trends Aggregator (`lta`)

The **Lichess Trends Aggregator** is a fast, streaming **Rust** tool that turns the massive monthly **Lichess PGN** dumps into compact, queryable statistics. It:

- fetches monthly PGN dumps from **Lichess** (or reads your local `.pgn.zst` files);
- **streams** and decodes them on the fly (no giant temp files);
- **aggregates** results by:
  - **month** (e.g. `2013-07`),
  - **ECO group** (letter + tens: e.g. `B33 → B30`; missing → `U00`),
  - **White Elo bucket** (default size `200`),
  - **Black Elo bucket** (default size `200`);
- stores **counts** only: `games`, `white_wins`, `black_wins`, `draws`.

Why this is nice 🙌:
- You can quickly see opening trends by month and rating bands.
- It’s designed for scale: stream → parse → aggregate in batches → optionally save to DB or CSV.
- Default run is **safe**: it’s a **dry-run** that doesn’t touch any database unless you say so with `--save`.

---

## Requirements
- **Docker**
- **Docker Compose**

That’s it! 🚀

---

## 🚀Getting started
A tiny sample dump is included in the repo at `sample/lichess_sample.pgn.zst`. The wrapper script **`lta`** builds and runs everything inside Docker for you.

```bash
# Show help and build on first run
./lta -h

# Run the sample (dry-run: no database writes)
./lta sample/lichess_sample.pgn.zst
```

### Other examples
**Remote stream from Lichess (dry-run):**
```bash
# Oldest → newest, stop at (and include) 2013-02
./lta --remote --until 2013-02 -v
```

**Save to local SQLite (creates ./data/lichess.db)**
```bash
cp .env.example .env       # defaults to local SQLite
./lta --save --remote --until 2013-02 -v
```

**Write aggregated CSVs (still dry-run for DB):**
```bash
# One CSV per month will be written into ./out/
./lta --remote --until 2013-02 --out out/ -v
```

**CSV columns**
```
month,eco_group,white_bucket,black_bucket,games,white_wins,black_wins,draws
```

> 🔎 Tip: In **local mode**, `--out` can be a **file path** (single CSV). In **remote mode**, `--out` is usually a **directory** (one CSV per month).

---

## ⚙️How it works
### 1. Streaming pipeline
- **Remote mode**: The app streams each monthly `*.pgn.zst` over HTTP and pipes it through a **zstd** decoder. There’s no need to store the whole file on disk.
- **Local mode**: The script uses `zstdcat` to decompress the `.zst` you already have and streams it into the app.

### 2. Processing in batches
- The PGN stream is divided into **game batches** (configurable). Each batch is parsed and aggregated in parallel (Rayon), then merged into a single in-memory map keyed by `(month, eco_group, white_bucket, black_bucket)`.

### 3. Database (optional)
- With `--save`, results are persisted using **SQLx** either to a **local SQLite file** or to a **Postgres** database (depending on your `DATABASE_URL`). Batched upserts and transactions are used for speed.
- Without `--save` → **no DB connections or writes**

The following tables are created (if not already present) when saving :
- **`aggregates`** — aggregated counts  
  - `month` (TEXT, e.g. `YYYY-MM`)  
  - `eco_group` (TEXT, e.g. `B20`, `C00`, `U00`)  
  - `white_bucket` (INTEGER, lower bound, e.g. `2200`)  
  - `black_bucket` (INTEGER, lower bound, e.g. `2000`)  
  - `games` (INTEGER)  
  - `white_wins` (INTEGER)  
  - `black_wins` (INTEGER)  
  - `draws` (INTEGER)  
  - **PRIMARY KEY** (`month`, `eco_group`, `white_bucket`, `black_bucket`)

- **`ingestions`** — tracks processed months (only in remote mode, see below)
  - `month` (TEXT, PRIMARY KEY)  
  - `url` (TEXT)  
  - `started_at` (TEXT, ISO8601)  
  - `finished_at` (TEXT, ISO8601)  
  - `games` (INTEGER, default 0)  
  - `duration_ms` (INTEGER, default 0)  
  - `status` (TEXT: `started` | `success` | `failed`)

- **`_sqlx_migrations`** — internal table used by SQLx to record executed migrations

You can reset your local SQLite to start fresh :
```bash
rm -f data/lichess.db data/lichess.db-wal data/lichess.db-shm
```
---

## 🗂️Local mode (details)
Use a local `.pgn.zst` file you already have (no extraction needed).

```bash
# Count games (dry-run)
./lta path/to/lichess_db_standard_rated_2013-07.pgn.zst

# Count and write a single CSV
./lta --out out/2013-07.csv path/to/lichess_db_standard_rated_2013-07.pgn.zst

# Persist counts to local SQLite
cp .env.example .env
./lta --save --out out/2013-07.csv path/to/lichess_db_standard_rated_2013-07.pgn.zst
```

What you’ll see in the terminal:
- per-file timing + number of games processed;
- optional “wrote CSV” message if `--out` is set.

---

## 🌐Remote mode (Lichess)
The app reads `list.txt` from Lichess (a list of monthly URLs), sorts **oldest → newest**, and processes month after month.

```bash
# Dry-run up to a given month
./lta --remote --until 2013-05 -v

# Dry-run with CSVs (one file per month)
./lta --remote --until 2013-05 --out out/ -v

# Persist results into your configured database (requires .env with DATABASE_URL)
./lta --remote --until 2013-05 --save -v

# Use a custom index (if you mirror Lichess)
./lta --remote --list-url https://my.mirror/standard/list.txt --until 2013-05
```

What you’ll see:
- per-month timing + number of games processed;
- optional CSV write messages if `--out` is set.
- with `--save`, results are written to the DB and each processed month is kept track of in the ingestions table.

---

## 🗄️Remote database setup (Postgres)
You can push results into a remote **Postgres** database. Create a `.env` file, then run with `--save`.

1) Create `.env` (mock URL example shown):
```ini
# .env
DATABASE_URL=postgresql://user:pass@host:5432/dbname?sslmode=require
DB_MAX_CONNECTIONS=10
```

2) Save results to Postgres:
```bash
./lta --save --remote --until 2013-05 -v
```

---

## 🛠️Configuration (`config.toml`)
All knobs live in `config.toml`:

```toml
bucket_size   = 200   # Elo bucket size for white/black buckets
list_url      = "https://database.lichess.org/standard/list.txt"
batch_size    = 1000  # games per aggregation batch (Rayon)
db_batch_rows = 1000  # rows per DB upsert batch (used for SQLite & Postgres)
# rayon_threads = 8   # pin Rayon threads; default = CPU count
```

- **bucket_size**: Elo bucket width (e.g., 200 → 1200–1399, 1400–1599, …).
- **list_url**: the Lichess monthly index; change if you mirror it.
- **batch_size**: number of games processed at a time before merging.
- **db_batch_rows**: how many rows are inserted/updated per DB batch.
- **rayon_threads**: set to force a specific parallelism; otherwise uses CPU count.

---

## 💻CLI reference
```
# Default is DRY-RUN: no DB connection and no writes.

--remote, --ingest-remote    Stream monthly dumps from Lichess (oldest → newest)
--until YYYY-MM              Stop after this month (inclusive) in remote mode
--out, -o PATH               CSV output
                             - local: a file path (e.g., out/agg.csv)
                             - remote: a directory (one CSV per month),
                                       or a base filename (becomes base-YYYY-MM.ext)
--list-url URL               Override the Lichess list.txt endpoint
--save                       Persist to DATABASE_URL (run migrations and writes)
-v, --verbose                Detailed timings (HTTP, zstd, aggregation, DB)
-h, --help                   Show built-in help
```

---

## 📜License
This project is licensed under the terms of the MIT license.

---

## 🤝Contribution
We welcome contributions! 💡 Issues, PRs, and ideas are all appreciated.
