#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }; trap cleanup EXIT
mkdir -p "$TMP/site"
cat > "$TMP/site/index.php" <<'PHP'
<?php
try { throw new Exception("root", 17); } catch (Exception $root) {
 try { throw new Exception("middle", 29, $root); } catch (Exception $middle) {
  try { throw new Exception("outer", 41, $middle); } catch (Exception $outer) {
   echo $outer->getMessage(), ":", $outer->getCode(), ":", $outer->getPrevious()->getMessage(), ":", $outer->getPrevious()->getPrevious()->getCode(), ":";
   try { throw $outer; } catch (Throwable $again) { echo $again->getPrevious()->getCode(); }
  }
 }
}
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19987}}
JSON
php "$TMP/site/index.php" > "$TMP/expected"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/app" > "$TMP/build.log" 2>&1 || { cat "$TMP/build.log"; exit 1; }
"$TMP/app" > "$TMP/log" 2>&1 & PID=$!
for i in $(seq 1 80); do if curl -fsS http://127.0.0.1:19987/ > "$TMP/actual" 2>/dev/null; then break; fi; sleep 0.1; done
test -f "$TMP/actual" || { cat "$TMP/log"; exit 1; }
diff -u "$TMP/expected" "$TMP/actual"
echo 'exception previous chaining HTTP differential: passed'
