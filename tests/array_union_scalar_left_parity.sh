#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ [[ -z "$PID" ]] || { kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; }; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19947}}
JSON
for entry in 'int|1|int' 'bool|true|bool' 'string|"hello"|string'; do
 IFS='|' read -r name initial expected <<< "$entry"
 cat > "$TMP/site/index.php" <<PHP
<?php
\$a = $initial;
\$a += [2];
echo "SHOULD_NOT_RENDER";
PHP
 "$ROOT/target/debug/phix" check "$TMP/site" >/dev/null
 "$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app" >/dev/null
 "$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
 code=""
 for i in $(seq 1 100); do
  code=$(curl -s -o "$TMP/body" -w '%{http_code}' http://127.0.0.1:19947/ || :)
  [[ "$code" == 500 ]] && break
  sleep 0.1
 done
 [[ "$code" == 500 ]] || { cat "$TMP/stderr"; echo "Expected HTTP 500 for $name, got $code"; exit 1; }
 grep -Fq "Fatal error: Uncaught TypeError: Unsupported operand types: $expected + array" "$TMP/body"
 ! grep -q SHOULD_NOT_RENDER "$TMP/body"
 kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; PID=""
done
echo 'Scalar-left array += runtime TypeError: 3/3 cases passed'
