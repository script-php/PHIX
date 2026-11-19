#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
echo "BEFORE", [1] + 2, "AFTER";
echo "UNREACHABLE";
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19716}}
JSON
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
STATUS=000
for i in $(seq 1 80); do
 STATUS="$(curl --max-time 3 -sS -o "$TMP/response" -w '%{http_code}' 'http://127.0.0.1:19716/' 2>/dev/null || :)"
 if [[ "$STATUS" != "000" && -n "$STATUS" ]]; then break; fi
 sleep 0.1
done
[[ "$STATUS" == "500" ]] || { echo "Expected HTTP 500, got $STATUS" >&2; cat "$TMP/stderr" >&2; exit 1; }
grep -Fq 'Fatal error: Uncaught TypeError:' "$TMP/response"
! grep -Eq 'BEFORE|AFTER|UNREACHABLE' "$TMP/response"
! grep -q 'panicked at' "$TMP/stderr"
echo 'Fatal array error inside comma-separated echo: HTTP 500, no partial output'
