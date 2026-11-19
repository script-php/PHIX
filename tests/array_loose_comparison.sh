#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$a = ["x" => 1, "y" => [true, null]];
$b = ["y" => [true, null], "x" => 1];
$c = ["x" => "1", "y" => [1, false]];
$d = ["x" => 2, "y" => [true, null]];
echo $a == $b, '|', $a === $b, '|', $a == $c, '|', $a != $d;
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19624}}
JSON
cd "$ROOT"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/out" 2>"$TMP/err" & PID=$!
for i in $(seq 1 80); do
 if curl -fsS 'http://127.0.0.1:19624/' >"$TMP/page" 2>/dev/null; then break; fi
 sleep 0.1
done
php "$TMP/site/index.php" > "$TMP/php-output"
diff -u "$TMP/php-output" "$TMP/page"
echo 'Array loose comparison differential test passed'
