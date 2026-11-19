#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }; trap cleanup EXIT
mkdir -p "$TMP/site"
cat > "$TMP/site/index.php" <<'PHP'
<?php
try { throw new Exception("hello"); } catch (Exception $e) { echo $e->getMessage(), ":", $e->getCode(), ":", $e->getPrevious(); }
echo ":";
try { throw new TypeError("wrong"); } catch (Error $e) { echo $e->getMessage(), ":", $e->getCode(); }
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19984}}
JSON
php "$TMP/site/index.php" > "$TMP/expected"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/app" > "$TMP/build.log" 2>&1 || { cat "$TMP/build.log"; exit 1; }
"$TMP/app" > "$TMP/log" 2>&1 & PID=$!
for i in $(seq 1 60); do if curl -fsS http://127.0.0.1:19984/ > "$TMP/actual" 2>/dev/null; then break; fi; sleep 0.1; done
test -f "$TMP/actual" || { cat "$TMP/log"; exit 1; }
diff -u "$TMP/expected" "$TMP/actual"
echo 'exception method HTTP differential: passed'
