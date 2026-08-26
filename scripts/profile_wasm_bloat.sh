#!/usr/bin/env bash
# SC-042: WASM size profiling via cargo-bloat.
#
# Usage:
#   ./scripts/profile_wasm_bloat.sh [TOP_N]
#
# Prerequisites:
#   cargo install cargo-bloat
#
# This script compiles the contract to wasm32-unknown-unknown and lists the
# top N functions contributing to WASM binary size (default: 20).
set -euo pipefail

TOP_N="${1:-20}"

cd "$(dirname "$0")/.."

if ! command -v cargo-bloat &>/dev/null; then
  echo "cargo-bloat not found. Installing..."
  cargo install cargo-bloat
fi

echo "=== WASM Size Profile (top ${TOP_N} functions by Rust source) ==="
echo ""

cargo bloat \
  --target wasm32-unknown-unknown \
  --release \
  -n "${TOP_N}" \
  2>&1 || {
    echo ""
    echo "Hint: If the wasm32-unknown-unknown target is not installed, run:"
    echo "  rustup target add wasm32-unknown-unknown"
    exit 1
  }

echo ""
echo "=== Done ==="
