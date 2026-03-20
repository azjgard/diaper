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

# --- Test 6: install-hook creates hook script ---
echo "[6] Install hook"
"$DIAPER" install-hook

HOOK_SCRIPT="$HOME/.claude/hooks/diaper-check.sh"
if [ -x "$HOOK_SCRIPT" ]; then
  pass "hook script exists and is executable"
else
  fail "hook script not found or not executable" "expected $HOOK_SCRIPT"
fi

# --- Test 7: Hook script contains expected content ---
echo "[7] Hook script content"
HOOK_CONTENT=$(cat "$HOOK_SCRIPT")

if echo "$HOOK_CONTENT" | grep -q '#!/bin/bash'; then
  pass "hook script has bash shebang"
else
  fail "missing shebang" "$HOOK_CONTENT"
fi

if echo "$HOOK_CONTENT" | grep -q 'api-gateway'; then
  pass "hook script checks for api-gateway"
else
  fail "missing api-gateway check" "$HOOK_CONTENT"
fi

if echo "$HOOK_CONTENT" | grep -q '/tmp/diaper-check-accept'; then
  pass "hook script has accept escape hatch"
else
  fail "missing escape hatch" "$HOOK_CONTENT"
fi

# --- Test 8: settings.json structure ---
echo "[8] settings.json structure"
SETTINGS_FILE="$HOME/.claude/settings.json"

if [ -f "$SETTINGS_FILE" ]; then
  pass "settings.json exists"
else
  fail "settings.json not found" "expected $SETTINGS_FILE"
fi

SETTINGS=$(cat "$SETTINGS_FILE")

# Verify it's valid JSON
if echo "$SETTINGS" | python3 -m json.tool > /dev/null 2>&1; then
  pass "settings.json is valid JSON"
else
  fail "settings.json is not valid JSON" "$SETTINGS"
fi

# Verify hooks.Stop exists and is an array with one entry
STOP_COUNT=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['hooks']['Stop']))" 2>&1)
if [ "$STOP_COUNT" = "1" ]; then
  pass "hooks.Stop has exactly 1 entry"
else
  fail "expected hooks.Stop to have 1 entry, got $STOP_COUNT" "$SETTINGS"
fi

# Verify the hook command references diaper-check.sh
HOOK_CMD=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['hooks']['Stop'][0]['hooks'][0]['command'])" 2>&1)
if echo "$HOOK_CMD" | grep -q 'diaper-check.sh'; then
  pass "hook command references diaper-check.sh"
else
  fail "hook command doesn't reference diaper-check.sh" "$HOOK_CMD"
fi

# Verify statusMessage is set
STATUS_MSG=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['hooks']['Stop'][0]['hooks'][0]['statusMessage'])" 2>&1)
if [ "$STATUS_MSG" = "Running diaper check..." ]; then
  pass "hook has correct statusMessage"
else
  fail "unexpected statusMessage" "$STATUS_MSG"
fi

# Verify hook type is "command"
HOOK_TYPE=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['hooks']['Stop'][0]['hooks'][0]['type'])" 2>&1)
if [ "$HOOK_TYPE" = "command" ]; then
  pass "hook type is 'command'"
else
  fail "expected hook type 'command', got '$HOOK_TYPE'" "$SETTINGS"
fi

# --- Test 9: install-hook is idempotent ---
echo "[9] Install hook idempotency"
"$DIAPER" install-hook

STOP_COUNT_AFTER=$(cat "$SETTINGS_FILE" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['hooks']['Stop']))" 2>&1)
if [ "$STOP_COUNT_AFTER" = "1" ]; then
  pass "running install-hook twice doesn't duplicate the entry"
else
  fail "expected 1 Stop entry after second install, got $STOP_COUNT_AFTER" "$(cat "$SETTINGS_FILE")"
fi

# --- Summary ---
echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
