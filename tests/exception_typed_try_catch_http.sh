#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }; trap cleanup EXIT
mkdir -p "$TMP/site"
cat > "$TMP/site/index.php" <<'PHP'
<?php
function recover(): int {
  try { throw new Exception('caught'); }
  catch (Exception $e) { echo $e->getMessage(), ';'; }
  finally { echo 'finally;'; }
  return 9;
}
echo recover();
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19992}}
JSON
php "$TMP/site/index.php" > "$TMP/expected"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/app" > "$TMP/build.log" 2>&1 || { cat "$TMP/build.log"; exit 1; }
"$TMP/app" > "$TMP/log" 2>&1 & PID=$!
for i in $(seq 1 100); do if curl -fsS http://127.0.0.1:19992/ > "$TMP/actual" 2>/dev/null; then break; fi; sleep 0.1; done
test -f "$TMP/actual" || { cat "$TMP/log"; exit 1; }
diff -u "$TMP/expected" "$TMP/actual"
echo 'typed try/catch/finally PHP differential: passed'
