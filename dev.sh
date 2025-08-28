#!/usr/bin/env bash
set -euo pipefail

DEFAULT_FILE="lichess_db_standard_rated_2013-01.pgn.zst"
FILE="${1:-${LICHESS_FILE:-$DEFAULT_FILE}}"

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  cat <<EOF
Usage: ./dev.sh [pgn.zst]

Streams a Lichess monthly PGN (.zst) into the Rust app inside the dev container.
- If no file is given, uses: $DEFAULT_FILE
- File path should be relative to the repo root (this folder), which is mounted at /app in the container.
EOF
  exit 0
fi

docker compose up -d dev >/dev/null

if [[ ! -f "$FILE" ]]; then
  echo "❌ PGN file not found: $FILE" >&2
  exit 1
fi

echo "▶️  Counting games from: $FILE"
docker compose exec -T dev bash -c "zstdcat '/app/$FILE' | cargo run --quiet"
