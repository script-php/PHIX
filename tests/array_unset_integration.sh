#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$items = [5 => 'old', 'name' => 'Alex'];
unset($items[5]);
$items[] = 'new';
echo $items[6], '|', $items['name'];
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19579}}
JSON
cd "$ROOT"
cargo build --offline -q
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/out" 2>"$TMP/err" & PID=$!
for i in $(seq 1 40); do
 if curl -fsS 'http://127.0.0.1:19579/' >"$TMP/page"; then break; fi
 sleep 0.1
done
[[ "$(cat "$TMP/page")" == 'new|Alex' ]]
echo 'Array unset integration passed'
