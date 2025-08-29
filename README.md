# Lichess Trends Aggregator

A fast, streaming **Rust** tool that ingests monthly **Lichess PGN dumps** and aggregates results by:

- **month**
- **ECO group** (letter + tens: e.g. `B33 → B30`, missing → `U00`)
- **White Elo bucket** (default size: **200**)
- **Black Elo bucket** (default size: **200**)

For each key it stores **counts**: `games`, `white_wins`, `black_wins`, `draws`.

It runs in two modes:

1) **Local file mode** — read a `.pgn.zst` you already have (no temp extraction).
2) **Remote mode** — stream monthly dumps *directly* from Lichess (no files saved).

> **Default is DRY-RUN**: the app **does not** connect to any database and **does not** write anything unless you pass `--save`.

---

## Requirements

- **Docker** and **Docker Compose**
- Internet access (for remote mode)
- (Optional) A Postgres DB (e.g. **Neon**) if you want to persist results remotely.  
  Local development uses **SQLite** by default.

---

## Quick start (TL;DR)

```bash
# 0) First build happens automatically the first time you run the script
./dev.sh -v    # prints help and builds the debug binary

# 1) DRY-RUN on the tiny example file (no DB writes)
./dev.sh sample/lichess_sample.pgn.zst

# 2) DRY-RUN streaming from Lichess (no DB writes)
./dev.sh --remote --until 2013-02 -v

# 3) Save results to local SQLite (./data/lichess.db)
cp .env.example .env
./dev.sh --save --remote --until 2013-02 -v
```

Open SQLite web UI (only for SQLite): <http://localhost:8080>

---

## How it works

- **Streaming**: remote mode uses HTTP streaming + `zstd` decode; local mode uses `zstdcat` piping.
- **Aggregation**: parallel in-memory hashmap keyed by `(month, eco_group, white_bucket, black_bucket)`.
- **ECO grouping**: `ECO` letter + tens (e.g., `C09 → C00`).
- **Buckets**: Elo bucket size configurable in `config.toml` (default `200`).
- **Database** (opt-in with `--save`):
  - **SQLite** (local file at `./data/lichess.db`) — plus a simple web UI via `sqlite-web`.
  - **Postgres** (e.g., Neon) — fast batched upserts with `INSERT ... VALUES (...), ... ON CONFLICT ...`.

---

## Local file mode

A small **sample PGN** is provided: `sample/lichess_sample.pgn.zst`.

**Dry-run, just count games (and optional CSV):**
```bash
# count only
./dev.sh sample/lichess_sample.pgn.zst

# count and write aggregated CSV
./dev.sh --out out/ sample/lichess_sample.pgn.zst
# → writes out/aggregates.csv (or per-file name if directory)
```

**Persist to DB (requires `.env` and `--save`):**
```bash
cp .env.example .env             # defaults to local sqlite
./dev.sh --save --out out/ sample/lichess_sample.pgn.zst
```

> Without `--save`, **no** DB connection, migrations, or writes happen.

---

## Remote mode (Lichess)

**Dry-run from Lichess (no DB):**
```bash
# Process months oldest → newest, stop at YYYY-MM (inclusive)
./dev.sh --remote --until 2013-05 -v

# Example output:
# 2013-03 | 2.804s | games=158635
# 2013-04 | 3.056s | games=157871
```

**Write monthly CSVs while still dry-run:**
```bash
./dev.sh --remote --until 2013-05 --out out/ -v
# → out/2013-03.csv, out/2013-04.csv, ...
```

**Persist to DB (explicit opt-in with `--save`):**
```bash
cp .env.example .env
./dev.sh --save --remote --until 2013-05 -v   # runs migrations and writes
```

Use a custom list index if you mirror Lichess:
```bash
./dev.sh --remote --list-url https://my.mirror/standard/list.txt --until 2013-05
```

---

## CSV output

CSV columns (counts only):
```
month,eco_group,white_bucket,black_bucket,games,white_wins,black_wins,draws
```

- In **remote mode**, pass `--out` with a **directory** to write **one CSV per month**.
- In **local mode**, pass `--out` with a **file path** to write a single CSV.

---

## Database setup

### Local SQLite (development)

1) Create `.env` from the example:
   ```bash
   cp .env.example .env
   ```
2) Run with `--save` to create tables and persist:
   ```bash
   ./dev.sh --save --remote --until 2013-02 -v
   ```
3) Optional UI: <http://localhost:8080>

Schema (via migrations):
- `aggregates(month, eco_group, white_bucket, black_bucket, games, white_wins, black_wins, draws)`
- `ingestions(month, url, started_at, finished_at, games, duration_ms, status)`

### Remote Postgres (e.g., Neon)

1) Create a database (Neon free tier is fine for testing).
2) Put your URL in `.env`:
   ```ini
   DATABASE_URL=postgresql://user:pass@host/dbname?sslmode=require
   DB_MAX_CONNECTIONS=10
   ```
3) Run with `--save` to run migrations and write:
   ```bash
   ./dev.sh --save --remote --until 2013-05 -v
   ```

> Batched upserts are enabled for Postgres, so remote writes are fast.  
> Dry-run (`--save` omitted) avoids DB entirely.

---

## Configuration

Edit `config.toml` to tune defaults:

```toml
bucket_size = 200                     # Elo bucket size
list_url    = "https://database.lichess.org/standard/list.txt"
batch_size  = 1000                    # games per parallel batch
# rayon_threads = 8                   # pin Rayon threads; default = CPU count
```

Environment variables (via `.env`):

```ini
# Local development: SQLite database in ./data/lichess.db
DATABASE_URL=sqlite:///data/lichess.db
DB_MAX_CONNECTIONS=10

# Example Postgres (Neon):
# DATABASE_URL=postgresql://user:pass@host/dbname?sslmode=require
# DB_MAX_CONNECTIONS=10
```

---

## CLI reference

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

## Troubleshooting

- **No extra logs with -v**: the script rebuilds the binary every run; if you changed code, run again with `-v`.
- **“syntax error at/near VALUES”** on Postgres: ensure you’ve pulled the version with **batched upserts**.
- **Container not running**: `./dev.sh` auto-starts `dev` and `dbui`, or run `docker compose up -d dev dbui`.
- **Slow Postgres writes**: batching is enabled; if still slow, verify network latency to your DB region.
- **Dry-run confusion**: remember `--save` is required to touch any DB.

---

## License

MIT