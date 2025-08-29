#!/usr/bin/env bash
# dev.sh — run local files or remote ingest into the dockerized Rust + SQLite setup
set -euo pipefail

DEFAULT_FILE="lichess_db_standard_rated_2013-01.pgn.zst"

REMOTE=0
UNTIL=""
OUT_HOST="${OUT:-}"   # also supports env OUT=...
FILES=()
LIST_URL=""           # optional override (passes through to app)

usage() {
  cat <<'EOF'
Usage:
  Local file(s):
    ./dev.sh [--out agg.csv] [file1.zst [file2.zst ...]]

  Remote ingest (stream from Lichess without saving .zst):
    ./dev.sh --remote [--until YYYY-MM] [--out out] [--list-url URL]

Options:
  --remote               Stream all missing months from Lichess (oldest -> newest)
  --until YYYY-MM        Stop after this month (inclusive) in remote mode
  --out, -o PATH         Write CSV output (local mode: single file; remote mode: dir or base file)
                         - local: PATH is a file (e.g., out/agg.csv)
                         - remote: if PATH is a directory, writes one CSV per month inside;
                                   if PATH is a file, writes <base>-YYYY-MM.<ext>
  --list-url URL         Override the list.txt endpoint (default in app)
  -h, --help             Show this help

Notes:
  - DB UI: http://localhost:8080  (sqlite-web)
  - SQLite persists in ./data/lichess.db
  - Local mode prints: "<file> | <secs>s | games=<n>"
EOF
}

# --- parse args ---
while [[ $# -gt 0 ]]; do
  case "$1" in
    --remote|--ingest-remote) REMOTE=1; shift ;;
    --until)  UNTIL="${2:-}"; shift 2 ;;
    --out|-o) OUT_HOST="${2:-}"; shift 2 ;;
    --list-url) LIST_URL="${2:-}"; shift 2 ;;
    -h|--help) usage; exit 0 ;;
    --) shift; break ;; # end of options
    -*)
      echo "Unknown option: $1" >&2
      usage
      exit 1
      ;;
    *)
      FILES+=("$1")
      shift
      ;;
  esac
done

# defaults for local mode
if [[ $REMOTE -eq 0 && ${#FILES[@]} -eq 0 ]]; then
  FILES=("${LICHESS_FILE:-$DEFAULT_FILE}")
fi

# Ensure services are up and data dir exists (for SQLite bind mount)
docker compose up -d dev dbui >/dev/null
mkdir -p data

# Normalize OUT path: strip /app/ if given; create parent dir on host; compute container path
OUT_CONTAINER=""
if [[ -n "$OUT_HOST" ]]; then
  [[ "$OUT_HOST" == /app/* ]] && OUT_HOST="${OUT_HOST#/app/}"
  # Create parent dir (if it's a file path) and also create the path itself if it's a dir
  mkdir -p "$(dirname "$OUT_HOST")"
  if [[ "$OUT_HOST" == */ || "$OUT_HOST" != *.* ]]; then
    # looks like a directory (trailing slash or no dot-extension)
    mkdir -p "$OUT_HOST"
  fi
  OUT_CONTAINER="/app/$OUT_HOST"
fi

if [[ $REMOTE -eq 1 ]]; then
  # Build app args
  APP_ARGS=(--ingest-remote)
  [[ -n "$UNTIL" ]]        && APP_ARGS+=(--until "$UNTIL")
  [[ -n "$OUT_CONTAINER" ]] && APP_ARGS+=(--out "$OUT_CONTAINER")
  [[ -n "$LIST_URL" ]]     && APP_ARGS+=(--list-url "$LIST_URL")

  echo "▶️  Remote ingest starting..."
  # Run inside container (HTTP stream -> zstd decode -> aggregate -> DB)
  docker compose exec -T dev bash -c "cargo run --quiet -- ${APP_ARGS[*]}"
  echo "🗄  DB UI: http://localhost:8080"
  exit 0
fi

# --- Local file mode ---
for FILE in "${FILES[@]}"; do
  if [[ ! -f "$FILE" ]]; then
    echo "❌ PGN file not found: $FILE" >&2
    continue
  fi

  start_ms=$(date +%s%3N)

  if [[ -n "$OUT_CONTAINER" ]]; then
    # Pipe decompressed PGN to app; pass --out to write CSV
    games=$(docker compose exec -T dev bash -c \
      "zstdcat '/app/$FILE' | cargo run --quiet -- --out '$OUT_CONTAINER'")
  else
    games=$(docker compose exec -T dev bash -c \
      "zstdcat '/app/$FILE' | cargo run --quiet")
  fi

  end_ms=$(date +%s%3N)
  elapsed_ms=$((end_ms - start_ms))
  elapsed_s=$(awk "BEGIN { printf \"%.3f\", ${elapsed_ms}/1000 }")

  echo "$(basename "$FILE") | ${elapsed_s}s | games=${games}"

  if [[ -n "$OUT_CONTAINER" ]]; then
    if [[ -f "$OUT_HOST" ]]; then
      size=$(du -h "$OUT_HOST" | awk '{print $1}')
      echo "📄 Wrote CSV: $OUT_HOST (${size})"
    else
      echo "⚠️  Expected CSV not found at $OUT_HOST" >&2
    fi
  fi
done

echo "🗄  DB UI: http://localhost:8080"
