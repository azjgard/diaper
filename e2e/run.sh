#!/usr/bin/env bash
set -euo pipefail

PASS=0
FAIL=0

pass() { echo "  PASS: $1"; PASS=$((PASS + 1)); }
fail() { echo "  FAIL: $1"; echo "        $2"; FAIL=$((FAIL + 1)); }

echo "=== diaper E2E tests ==="
echo ""

# --- Test 1: Install script runs successfully ---
echo "[1] Running install script..."
bash /tmp/install.sh
echo ""

DIAPER="$HOME/.diaper/bin/diaper"

# --- Test 2: Binary exists and is executable ---
echo "[2] Binary is executable"
if [ -x "$DIAPER" ]; then
  pass "binary exists at $DIAPER"
else
  fail "binary not found or not executable" "expected $DIAPER to exist"
fi

# --- Test 3: Version output ---
echo "[3] Version output"
VERSION_OUTPUT=$("$DIAPER" --version 2>&1)
if echo "$VERSION_OUTPUT" | grep -q "^diaper "; then
  pass "version: $VERSION_OUTPUT"
else
  fail "unexpected version output" "$VERSION_OUTPUT"
fi

# --- Test 4: Check a file with violations ---
echo "[4] Check sample file with violations"
set +e
JSON_OUTPUT=$("$DIAPER" check --json /test/sample.js 2>&1)
EXIT_CODE=$?
set -e

if [ "$EXIT_CODE" -eq 1 ]; then
  pass "exit code 1 (blowout detected)"
else
  fail "expected exit code 1, got $EXIT_CODE" "$JSON_OUTPUT"
fi

if echo "$JSON_OUTPUT" | grep -q '"rule": "async-await"'; then
  pass "async-await rule triggered"
else
  fail "async-await rule not found in output" "$JSON_OUTPUT"
fi

if echo "$JSON_OUTPUT" | grep -q '"stinkScore"'; then
  pass "JSON output contains stinkScore"
else
  fail "stinkScore not found in JSON output" "$JSON_OUTPUT"
fi

# --- Test 5: Check a clean file ---
echo "[5] Check a clean file"
echo 'const x = 1;' > /tmp/clean.js
set +e
CLEAN_OUTPUT=$("$DIAPER" check --json /tmp/clean.js 2>&1)
CLEAN_EXIT=$?
set -e

if [ "$CLEAN_EXIT" -eq 0 ]; then
  pass "exit code 0 (no blowout)"
else
  fail "expected exit code 0, got $CLEAN_EXIT" "$CLEAN_OUTPUT"
fi

# --- Summary ---
echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
