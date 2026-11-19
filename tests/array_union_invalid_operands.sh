#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ [[ -z "$PID" ]] || { kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; }; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$a = ["x" => 1];
$b = $a + 5;
echo "SHOULD_NOT_RENDER";
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19942}}
JSON
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 100); do
 code=$(curl -s -o "$TMP/body" -w '%{http_code}' http://127.0.0.1:19942/ || :)
 if [[ "$code" == 500 ]]; then break; fi
 sleep 0.1
done
[[ "${code:-}" == 500 ]] || { cat "$TMP/stderr"; echo "Expected HTTP 500, got $code"; exit 1; }
grep -Fq 'Fatal error: Uncaught TypeError: Unsupported operand types: array + int' "$TMP/body"
! grep -q 'SHOULD_NOT_RENDER' "$TMP/body"
echo 'Invalid array union operand HTTP test passed'
