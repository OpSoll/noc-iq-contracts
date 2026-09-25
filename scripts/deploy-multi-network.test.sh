#!/usr/bin/env bash
# -----------------------------------------------------------------------
# deploy-multi-network.test.sh — Parameter validation tests for
# deploy-multi-network.sh. Exercises only --dry-run/--help paths so no
# stellar-cli calls or real deployments happen.
#
# Usage: ./scripts/deploy-multi-network.test.sh
# -----------------------------------------------------------------------

set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="$SCRIPT_DIR/deploy-multi-network.sh"
TMP_DIR="$(mktemp -d)"
FAILURES=0

trap 'rm -rf "$TMP_DIR"' EXIT

assert_exit_code() {
    local description="$1"
    local expected="$2"
    local actual="$3"
    if [[ "$actual" -eq "$expected" ]]; then
        echo "PASS: $description"
    else
        echo "FAIL: $description (expected exit $expected, got $actual)"
        FAILURES=$((FAILURES + 1))
    fi
}

assert_file_exists() {
    local description="$1"
    local path="$2"
    if [[ -f "$path" ]]; then
        echo "PASS: $description"
    else
        echo "FAIL: $description (file not found: $path)"
        FAILURES=$((FAILURES + 1))
    fi
}

# --- Test: missing --admin fails fast ---
set +e
"$TARGET" --dry-run >/dev/null 2>&1
CODE=$?
set -e
assert_exit_code "missing --admin exits non-zero" 1 "$CODE"

# --- Test: --help exits 0 without requiring --admin ---
set +e
"$TARGET" --help >/dev/null 2>&1
CODE=$?
set -e
assert_exit_code "--help exits 0" 0 "$CODE"

# --- Test: --dry-run with --admin succeeds and writes a manifest ---
MANIFEST="$TMP_DIR/manifest.json"
set +e
"$TARGET" --admin GADMIN123 --dry-run --manifest "$MANIFEST" >/dev/null 2>&1
CODE=$?
set -e
assert_exit_code "--dry-run with --admin exits 0" 0 "$CODE"
assert_file_exists "dry-run writes a deployment manifest" "$MANIFEST"

# --- Test: unknown network is reported as failed, not fatal ---
MANIFEST2="$TMP_DIR/manifest2.json"
set +e
"$TARGET" --admin GADMIN123 --networks BOGUSNET --dry-run --manifest "$MANIFEST2" >/dev/null 2>&1
CODE=$?
set -e
assert_exit_code "unrecognised network still exits 0 (reported, not fatal)" 0 "$CODE"

if [[ "$FAILURES" -eq 0 ]]; then
    echo ""
    echo "All deploy-multi-network.sh parameter tests passed."
    exit 0
else
    echo ""
    echo "$FAILURES test(s) failed."
    exit 1
fi
