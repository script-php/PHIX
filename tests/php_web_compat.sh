#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"
PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
cd "$ROOT"
cargo build --release --offline
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$name = $_GET['name'];
echo 'Hello ' . htmlspecialchars($name) . '!';
echo '|', $_SERVER['REQUEST_METHOD'], '|', $_SERVER['QUERY_STRING'];
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19541}}
JSON
"$ROOT/target/release/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/out" 2>"$TMP/err" & PID=$!
for i in $(seq 1 50); do
  if curl -fsS 'http://127.0.0.1:19541/?name=%3CJane%26Joe%3E' > "$TMP/page"; then break; fi
  sleep .1
done
grep -Fq 'Hello &lt;Jane&amp;Joe&gt;!|GET|name=%3CJane%26Joe%3E' "$TMP/page"
[[ "$(curl -s -o /dev/null -w '%{http_code}' 'http://127.0.0.1:19541/?name=%GG')" == 400 ]]
[[ "$(curl -s -o /dev/null -w '%{http_code}' 'http://127.0.0.1:19541/?name=%FF')" == 400 ]]
[[ "$(curl -s -o /dev/null -w '%{http_code}' -I 'http://127.0.0.1:19541/?name=Hello')" == 200 ]]
echo 'PHP web compatibility integration passed'
