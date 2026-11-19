#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site"
cat > "$TMP/site/index.php" <<'PHP'
<?php
function fail(): int { if (true) { throw new Exception("left"); } return 0; }
function later(): int { echo "BAD"; return 3; }
try { echo fail() + later(); } catch (Exception $e) { echo "add;"; }
try { echo fail() * later(); } catch (Exception $e) { echo "multiply;"; }
try { echo fail() < later(); } catch (Exception $e) { echo "compare;"; }
try { echo fail() === later(); } catch (Exception $e) { echo "strict;"; }
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19983}}
JSON
php "$TMP/site/index.php" > "$TMP/expected"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/app" > "$TMP/build.log" 2>&1 || { cat "$TMP/build.log"; exit 1; }
"$TMP/app" > "$TMP/log" 2>&1 & PID=$!
for i in $(seq 1 100); do if curl -fsS http://127.0.0.1:19983/ > "$TMP/actual" 2>/dev/null; then break; fi; sleep 0.1; done
test -f "$TMP/actual" || { cat "$TMP/log"; exit 1; }
diff -u "$TMP/expected" "$TMP/actual"
echo 'integer arithmetic/comparison exception order PHP differential: passed'
