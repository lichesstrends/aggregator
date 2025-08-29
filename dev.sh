#!/usr/bin/env bash
set -euo pipefail

DEFAULT_FILE="lichess_db_standard_rated_2013-01.pgn.zst"
BIN="/app/target/debug/aggregator"     # binary path inside container
CARGO="/usr/local/cargo/bin/cargo"

REMOTE=0
UNTIL=""
OUT_HOST="${OUT:-}"
FILES=()
LIST_URL=""

usage() {
  cat <<'EOF'
Usage:
  Local file(s):
    ./dev.sh [--out agg.csv] [file1.zst [file2.zst ...]]

  Remote ingest (stream from Lichess without saving .zst):
    ./dev.sh --remote [--until YYYY-MM] [--out out] [--list-url URL]

Options:
  --remote               Stream all missing months (oldest -> newest)
  --until YYYY-MM        Stop after this month (inclusive)
  --out, -o PATH         CSV output. If directory, one CSV per month.
  --list-url URL         Override list.txt endpoint
  -h, --help             This help

Notes:
  - DB UI (SQLite only): http://localhost:8080
  - SQLite file persists in ./data/lichess.db
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
    --) shift; break ;;
    -*)
      echo "Unknown option: $1" >&2; usage; exit 1 ;;
    *)
      FILES+=("$1"); shift ;;
  esac
done

# defaults for local mode
if [[ $REMOTE -eq 0 && ${#FILES[@]} -eq 0 ]]; then
  FILES=("${LICHESS_FILE:-$DEFAULT_FILE}")
fi

mkdir -p data

# Bring up services (no recreate if already running)
docker compose up -d dev dbui >/dev/null

# Wait until dev container is running (avoid "not running" race)
wait_for_dev() {
  local tries=20
  local cid
  cid="$(docker compose ps -q dev || true)"
  while [[ -z "$cid" && $tries -gt 0 ]]; do
    sleep 0.2; tries=$((tries-1))
    cid="$(docker compose ps -q dev || true)"
  done
  if [[ -z "$cid" ]]; then
    echo "❌ dev container not created"; exit 1
  fi
  tries=50
  while [[ $tries -gt 0 ]]; do
    local state
    state="$(docker inspect -f '{{.State.Running}}' "$cid" 2>/dev/null || echo false)"
    if [[ "$state" == "true" ]]; then return 0; fi
    sleep 0.2; tries=$((tries-1))
  done
  echo "❌ dev container not running"; exit 1
}
wait_for_dev

# Ensure binary exists (first run or after clean)
docker compose exec -T dev bash -lc "[ -x '$BIN' ] || $CARGO build -q"

# Normalize OUT path (host->container)
OUT_CONTAINER=""
if [[ -n "$OUT_HOST" ]]; then
  [[ "$OUT_HOST" == /app/* ]] && OUT_HOST="${OUT_HOST#/app/}"
  mkdir -p "$(dirname "$OUT_HOST")"
  if [[ "$OUT_HOST" == */ || "$OUT_HOST" != *.* ]]; then
    mkdir -p "$OUT_HOST"
  fi
  OUT_CONTAINER="/app/$OUT_HOST"
fi

if [[ $REMOTE -eq 1 ]]; then
  APP_ARGS=(--ingest-remote)
  [[ -n "$UNTIL" ]]         && APP_ARGS+=(--until "$UNTIL")
  [[ -n "$OUT_CONTAINER" ]] && APP_ARGS+=(--out "$OUT_CONTAINER")
  [[ -n "$LIST_URL" ]]      && APP_ARGS+=(--list-url "$LIST_URL")

  echo "▶️  Remote ingest starting..."
  docker compose exec -T dev bash -lc "'$BIN' ${APP_ARGS[*]}"
  echo "🗄  DB UI (SQLite only): http://localhost:8080"
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
    cmd="zstdcat '/app/$FILE' | '$BIN' --out '$OUT_CONTAINER'"
  else
    cmd="zstdcat '/app/$FILE' | '$BIN'"
  fi

  games=$(docker compose exec -T dev bash -lc "$cmd")

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

echo "🗄  DB UI (SQLite only): http://localhost:8080"
