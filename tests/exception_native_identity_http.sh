#!/usr/bin/env bash
# COMPAT-075: native Throwable identity, previous chaining, and rejection of forged arrays.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }; trap cleanup EXIT
mkdir -p "$TMP/site"
cat > "$TMP/site/index.php" <<'PHP'
<?php
try { throw new Exception("first", 7); } catch (Exception $first) {
  try { throw new Exception("second", 9, $first); } catch (Exception $second) {
    echo $second->getPrevious() === $first;
    echo ":", $second->getPrevious()->getCode(), ":", $second->getMessage(), ";";
  }
}
try { throw ["class" => "Exception", "message" => "fake"]; } catch (Error $error) { echo "blocked"; }
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19995}}
JSON
# PHP throws an Error for non-Throwable values, including forged arrays.
php "$TMP/site/index.php" > "$TMP/expected"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/app" > "$TMP/build.log" 2>&1 || { cat "$TMP/build.log"; exit 1; }
"$TMP/app" > "$TMP/log" 2>&1 & PID=$!
for i in $(seq 1 100); do if curl -fsS http://127.0.0.1:19995/ > "$TMP/actual" 2>/dev/null; then break; fi; sleep 0.1; done
test -f "$TMP/actual" || { cat "$TMP/log"; exit 1; }
diff -u "$TMP/expected" "$TMP/actual"
echo 'native Throwable identity and forged-array PHP differential: passed'
