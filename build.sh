#!/usr/bin/env bash
#
# build.sh - Build the LichessTrends Aggregator using Docker
#
# Produces a statically-linked Linux executable at ./target/lta
# that can be run directly on any Linux system (including WSL).
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

IMAGE_NAME="lta-builder"
CONTAINER_NAME="lta-build-temp"
OUTPUT_DIR="./target"
OUTPUT_BINARY="$OUTPUT_DIR/lta"

echo "Building LichessTrends Aggregator v0.2.0..."

# Build the Docker image
echo "Building Docker image..."
docker build -f Dockerfile.build -t "$IMAGE_NAME" .

# Create output directory
mkdir -p "$OUTPUT_DIR"

# Clean up any existing container with same name
docker rm -f "$CONTAINER_NAME" 2>/dev/null || true

# Extract the binary from the image
echo "Extracting binary..."
docker create --name "$CONTAINER_NAME" "$IMAGE_NAME" >/dev/null 2>&1
docker cp "$CONTAINER_NAME:/build/target/x86_64-unknown-linux-musl/release/lta" - | tar -xf - -O > "$OUTPUT_BINARY"
docker rm "$CONTAINER_NAME" >/dev/null 2>&1

# Make it executable (should already be, but just in case)
chmod +x "$OUTPUT_BINARY"

# Show result
echo ""
echo "Build complete!"
echo "   Binary: $OUTPUT_BINARY"
echo "   Size:   $(du -h "$OUTPUT_BINARY" | cut -f1)"
echo ""
echo "Usage:"
echo "   $OUTPUT_BINARY --help"
echo "   $OUTPUT_BINARY --remote --until 2013-02 -v"
echo "   $OUTPUT_BINARY sample/lichess_sample.pgn.zst"
