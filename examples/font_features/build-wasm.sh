#!/bin/bash
# Build the font_features example for WASM and optionally deploy to Cloudflare Pages
set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
DIST_DIR="${1:-$SCRIPT_DIR/dist}"

echo "Building WASM release to $DIST_DIR..."
cd "$SCRIPT_DIR"
trunk build --release --dist "$DIST_DIR"

echo "Done. To deploy:"
echo "  cd $DIST_DIR && bunx wrangler pages deploy . --project-name iced-wasm-opsz"
