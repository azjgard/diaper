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

# --- Test 6: install-hooks creates both hook scripts ---
echo "[6] Install hooks"
"$DIAPER" install-hooks

STOP_SCRIPT="$HOME/.claude/hooks/diaper-check.sh"
PRE_EDIT_SCRIPT="$HOME/.claude/hooks/diaper-pre-edit.sh"

if [ -x "$STOP_SCRIPT" ]; then
  pass "stop hook script exists and is executable"
else
  fail "stop hook script not found or not executable" "expected $STOP_SCRIPT"
fi

if [ -x "$PRE_EDIT_SCRIPT" ]; then
  pass "pre-edit hook script exists and is executable"
else
  fail "pre-edit hook script not found or not executable" "expected $PRE_EDIT_SCRIPT"
fi

# --- Test 7: Stop hook script content ---
echo "[7] Stop hook script content"
STOP_CONTENT=$(cat "$STOP_SCRIPT")

if echo "$STOP_CONTENT" | grep -q '#!/bin/bash'; then
  pass "stop hook has bash shebang"
else
  fail "missing shebang" "$STOP_CONTENT"
fi

if echo "$STOP_CONTENT" | grep -q 'api-gateway'; then
  pass "stop hook checks for api-gateway"
else
  fail "missing api-gateway check" "$STOP_CONTENT"
fi

if echo "$STOP_CONTENT" | grep -q '/tmp/diaper-check-accept'; then
  pass "stop hook has accept escape hatch"
else
  fail "missing escape hatch" "$STOP_CONTENT"
fi

# --- Test 8: Pre-edit hook script content ---
echo "[8] Pre-edit hook script content"
PRE_EDIT_CONTENT=$(cat "$PRE_EDIT_SCRIPT")

if echo "$PRE_EDIT_CONTENT" | grep -q '#!/bin/bash'; then
  pass "pre-edit hook has bash shebang"
else
  fail "missing shebang" "$PRE_EDIT_CONTENT"
fi

if echo "$PRE_EDIT_CONTENT" | grep -q 'api-gateway'; then
  pass "pre-edit hook checks for api-gateway"
else
  fail "missing api-gateway check" "$PRE_EDIT_CONTENT"
fi

if echo "$PRE_EDIT_CONTENT" | grep -q 'tool_input.file_path'; then
  pass "pre-edit hook reads file path from stdin"
else
  fail "missing file_path extraction" "$PRE_EDIT_CONTENT"
fi

if echo "$PRE_EDIT_CONTENT" | grep -q '\-\-rule missing-test'; then
  pass "pre-edit hook runs missing-test rule"
else
  fail "missing --rule missing-test" "$PRE_EDIT_CONTENT"
fi

if echo "$PRE_EDIT_CONTENT" | grep -q 'additionalContext'; then
  pass "pre-edit hook uses additionalContext"
else
  fail "missing additionalContext" "$PRE_EDIT_CONTENT"
fi

if echo "$PRE_EDIT_CONTENT" | grep -q 'permissionDecision'; then
  pass "pre-edit hook sets permissionDecision"
else
  fail "missing permissionDecision" "$PRE_EDIT_CONTENT"
fi

# --- Test 9: settings.json structure ---
echo "[9] settings.json structure"
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

# Verify the stop hook command references diaper-check.sh
STOP_CMD=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['hooks']['Stop'][0]['hooks'][0]['command'])" 2>&1)
if echo "$STOP_CMD" | grep -q 'diaper-check.sh'; then
  pass "stop hook command references diaper-check.sh"
else
  fail "stop hook command doesn't reference diaper-check.sh" "$STOP_CMD"
fi

# Verify stop hook statusMessage
STATUS_MSG=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['hooks']['Stop'][0]['hooks'][0]['statusMessage'])" 2>&1)
if [ "$STATUS_MSG" = "Running diaper check..." ]; then
  pass "stop hook has correct statusMessage"
else
  fail "unexpected statusMessage" "$STATUS_MSG"
fi

# Verify stop hook type is "command"
STOP_TYPE=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['hooks']['Stop'][0]['hooks'][0]['type'])" 2>&1)
if [ "$STOP_TYPE" = "command" ]; then
  pass "stop hook type is 'command'"
else
  fail "expected hook type 'command', got '$STOP_TYPE'" "$SETTINGS"
fi

# --- Test 10: settings.json PreToolUse hook ---
echo "[10] settings.json PreToolUse hook"

PRE_COUNT=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['hooks']['PreToolUse']))" 2>&1)
if [ "$PRE_COUNT" = "1" ]; then
  pass "hooks.PreToolUse has exactly 1 entry"
else
  fail "expected hooks.PreToolUse to have 1 entry, got $PRE_COUNT" "$SETTINGS"
fi

# Verify matcher is Edit|Write|Bash
PRE_MATCHER=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['hooks']['PreToolUse'][0]['matcher'])" 2>&1)
if [ "$PRE_MATCHER" = "Edit|Write|Bash" ]; then
  pass "pre-edit hook matcher is 'Edit|Write|Bash'"
else
  fail "expected matcher 'Edit|Write|Bash', got '$PRE_MATCHER'" "$SETTINGS"
fi

# Verify the pre-edit hook command references diaper-pre-edit.sh
PRE_CMD=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['hooks']['PreToolUse'][0]['hooks'][0]['command'])" 2>&1)
if echo "$PRE_CMD" | grep -q 'diaper-pre-edit.sh'; then
  pass "pre-edit hook command references diaper-pre-edit.sh"
else
  fail "pre-edit hook command doesn't reference diaper-pre-edit.sh" "$PRE_CMD"
fi

# Verify pre-edit hook statusMessage
PRE_STATUS=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['hooks']['PreToolUse'][0]['hooks'][0]['statusMessage'])" 2>&1)
if [ "$PRE_STATUS" = "Checking for missing tests..." ]; then
  pass "pre-edit hook has correct statusMessage"
else
  fail "unexpected statusMessage" "$PRE_STATUS"
fi

# Verify pre-edit hook type is "command"
PRE_TYPE=$(echo "$SETTINGS" | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['hooks']['PreToolUse'][0]['hooks'][0]['type'])" 2>&1)
if [ "$PRE_TYPE" = "command" ]; then
  pass "pre-edit hook type is 'command'"
else
  fail "expected hook type 'command', got '$PRE_TYPE'" "$SETTINGS"
fi

# --- Test 11: install-hooks is idempotent ---
echo "[11] Install hooks idempotency"
"$DIAPER" install-hooks

STOP_COUNT_AFTER=$(cat "$SETTINGS_FILE" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['hooks']['Stop']))" 2>&1)
if [ "$STOP_COUNT_AFTER" = "1" ]; then
  pass "running install-hooks twice doesn't duplicate Stop entry"
else
  fail "expected 1 Stop entry after second install, got $STOP_COUNT_AFTER" "$(cat "$SETTINGS_FILE")"
fi

PRE_COUNT_AFTER=$(cat "$SETTINGS_FILE" | python3 -c "import json,sys; d=json.load(sys.stdin); print(len(d['hooks']['PreToolUse']))" 2>&1)
if [ "$PRE_COUNT_AFTER" = "1" ]; then
  pass "running install-hooks twice doesn't duplicate PreToolUse entry"
else
  fail "expected 1 PreToolUse entry after second install, got $PRE_COUNT_AFTER" "$(cat "$SETTINGS_FILE")"
fi

# --- Summary ---
echo ""
echo "=== Results: $PASS passed, $FAIL failed ==="

if [ "$FAIL" -gt 0 ]; then
  exit 1
fi
