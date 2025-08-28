#!/usr/bin/env bash
set -euo pipefail

DEFAULT_FILE="lichess_db_standard_rated_2013-01.pgn.zst"

OUT_HOST="${OUT:-}"   # still supports env OUT=...
FILES=()

# --- parse args ---
while [[ $# -gt 0 ]]; do
  case "$1" in
    --out|-o)
      OUT_HOST="$2"
      shift 2
      ;;
    -h|--help)
      cat <<EOF
Usage: ./dev.sh [--out agg.csv] [file1.zst [file2.zst ...]]

Streams .pgn.zst into the Rust app inside the dev container.
If --out is set, writes CSV inside the repo (bind-mounted at /app).

Examples:
  ./dev.sh
  ./dev.sh --out agg.csv lichess_db_standard_rated_2013-01.pgn.zst
  OUT=out/agg.csv ./dev.sh dump1.zst dump2.zst
EOF
      exit 0
      ;;
    *)
      FILES+=("$1")
      shift
      ;;
  esac
done

# default file if none provided
if [[ ${#FILES[@]} -eq 0 ]]; then
  FILES=("${LICHESS_FILE:-$DEFAULT_FILE}")
fi

# ensure dev container is up
docker compose up -d dev >/dev/null

# normalize OUT path: strip /app/ if user passed it; create parent dir on host
OUT_CONTAINER=""
if [[ -n "$OUT_HOST" ]]; then
  # if they gave /app/... convert to repo-relative for host
  if [[ "$OUT_HOST" == /app/* ]]; then
    OUT_HOST="${OUT_HOST#/app/}"
  fi
  mkdir -p "$(dirname "$OUT_HOST")"  # create folder on host
  OUT_CONTAINER="/app/$OUT_HOST"     # container path
fi

for FILE in "${FILES[@]}"; do
  if [[ ! -f "$FILE" ]]; then
    echo "❌ PGN file not found: $FILE" >&2
    continue
  fi

  start_ms=$(date +%s%3N)

  if [[ -n "$OUT_CONTAINER" ]]; then
    games=$(docker compose exec -T dev bash -c \
      "zstdcat '/app/$FILE' | cargo run --quiet -- --out \"$OUT_CONTAINER\"")
  else
    games=$(docker compose exec -T dev bash -c \
      "zstdcat '/app/$FILE' | cargo run --quiet")
  fi

  end_ms=$(date +%s%3N)
  elapsed_ms=$((end_ms - start_ms))
  elapsed_s=$(awk "BEGIN { printf \"%.3f\", ${elapsed_ms}/1000 }")

  echo "$(basename "$FILE") | ${elapsed_s}s | games=${games}"

  if [[ -n "$OUT_CONTAINER" ]]; then
    # announce and verify file on host
    if [[ -f "$OUT_HOST" ]]; then
      size=$(du -h "$OUT_HOST" | awk '{print $1}')
      echo "📄 Wrote CSV: $OUT_HOST (${size})"
    else
      echo "⚠️ Expected CSV not found at $OUT_HOST" >&2
    fi
  fi
done
