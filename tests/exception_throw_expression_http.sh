#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }; trap cleanup EXIT
mkdir -p "$TMP/site"
cat > "$TMP/site/index.php" <<'PHP'
<?php
try { echo (throw new Exception("expr", 42)); } catch (Exception $e) { echo "caught:", $e->getMessage(), ":", $e->getCode(), ";"; }
try { echo (throw new TypeError("typed")); } catch (Error $e) { echo "parent:", $e->getMessage(), ";"; }
try { throw new Exception("inner"); } catch (Exception $e) {
  try { echo (throw $e); } catch (Exception $again) { echo "rethrow:", $again->getMessage(); }
}
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19994}}
JSON
php "$TMP/site/index.php" > "$TMP/expected"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/app" > "$TMP/build.log" 2>&1 || { cat "$TMP/build.log"; exit 1; }
"$TMP/app" > "$TMP/log" 2>&1 & PID=$!
for i in $(seq 1 100); do if curl -fsS http://127.0.0.1:19994/ > "$TMP/actual" 2>/dev/null; then break; fi; sleep 0.1; done
test -f "$TMP/actual" || { cat "$TMP/log"; exit 1; }
diff -u "$TMP/expected" "$TMP/actual"
echo 'exception throw expressions PHP differential: passed'
