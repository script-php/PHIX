#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$a = [true => 'yes', false => 'no', null => 'empty', '8' => 'eight', '08' => 'zeroeight'];
$b = $a;
$b[true] = 'changed';
$b['nested'] = ['x' => 1];
$c = $b;
$c['nested']['x'] = 2;
echo $a[1], '|', $b[1], '|', $a[0], '|', $a[''], '|', $a[8], '|', $a['08'], '|', $b['nested']['x'], '|', $c['nested']['x'];
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19581}}
JSON
cd "$ROOT"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/out" 2>"$TMP/err" & PID=$!
for i in $(seq 1 60); do
 if curl -fsS 'http://127.0.0.1:19581/' >"$TMP/page" 2>/dev/null; then break; fi
 sleep 0.1
done
[[ "$(cat "$TMP/page")" == 'yes|changed|no|empty|eight|zeroeight|1|2' ]]
echo 'Array copy and coerced key integration passed'
