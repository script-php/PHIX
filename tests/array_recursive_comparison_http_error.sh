#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$a = [];
$a = ["value" => 1, "self" => &$a];
$b = [];
$b = ["value" => 1, "self" => &$b];
echo "BEFORE";
echo $a == $b;
echo "AFTER";
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19703}}
JSON
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 80); do
 STATUS="$(curl --max-time 3 -sS -o "$TMP/response" -w '%{http_code}' 'http://127.0.0.1:19703/' 2>/dev/null || :)"
 if [[ "$STATUS" != "000" && -n "$STATUS" ]]; then break; fi
 sleep 0.1
done
[[ "$STATUS" == "500" ]] || { echo "Expected HTTP 500, got $STATUS" >&2; cat "$TMP/stderr" >&2; exit 1; }
grep -Fq 'Fatal error: Uncaught Error: Nesting level too deep - recursive dependency?' "$TMP/response"
! grep -q 'BEFORE\|AFTER' "$TMP/response"
! grep -q 'panicked at' "$TMP/stderr"
echo 'Recursive array comparison returns HTTP 500 without Rust panic or partial output'
