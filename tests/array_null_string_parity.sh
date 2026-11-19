#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$a = [null];
$b = [""];
$c = ["0"];
$d = ["0.0"];
$e = [" "];
echo $a == $b, '|', $a == $c, '|', $a == $d, '|', $a == $e;
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19627}}
JSON
cd "$ROOT"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/out" 2>"$TMP/err" & PID=$!
for i in $(seq 1 100); do
 if curl -fsS 'http://127.0.0.1:19627/' >"$TMP/page" 2>/dev/null; then break; fi
 sleep 0.1
done
php "$TMP/site/index.php" > "$TMP/php-output"
diff -u "$TMP/php-output" "$TMP/page"
echo 'PHP null/string array differential test passed'
