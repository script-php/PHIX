#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ [[ -z "$PID" ]] || { kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; }; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
function key1(): string { echo 'K1|'; return 'user'; }
function key2(): string { echo 'K2|'; return 'name'; }
function fallback(): string { echo 'F|'; return 'Guest'; }
echo $missing[key1()][key2()] ?? fallback();
echo '|';
$data = [];
echo $data[key1()][key2()] ?? fallback();
echo '|';
$data['user']['name'] ??= fallback();
echo $data['user']['name'];
echo '|';
$data['user']['name'] ??= fallback();
echo $data['user']['name'];
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19807}}
JSON
EXPECTED="$(php "$TMP/site/index.php")"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app" >"$TMP/build.log" 2>&1 || { cat "$TMP/build.log"; exit 1; }
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 80); do
 if curl -fsS 'http://127.0.0.1:19807/' >"$TMP/response" 2>/dev/null; then break; fi
 sleep 0.1
done
ACTUAL="$(cat "$TMP/response")"
[[ "$ACTUAL" == "$EXPECTED" ]] || { echo "expected <$EXPECTED>, got <$ACTUAL>" >&2; exit 1; }
echo "Coalescing differential matches PHP: $ACTUAL"
