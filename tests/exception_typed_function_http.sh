#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }; trap cleanup EXIT
mkdir -p "$TMP/site"
cat > "$TMP/site/index.php" <<'PHP'
<?php
function fail_int(): int { throw new Exception('int failure'); return 7; }
function fail_bool(): bool { throw new TypeError('bool failure'); return true; }
function fail_string(): string { throw new ValueError('string failure'); return 'ok'; }
try { echo fail_int(); echo 'unreachable'; } catch (Exception $e) { echo $e->getMessage(), ';'; }
try { echo fail_bool(); echo 'unreachable'; } catch (Error $e) { echo $e->getMessage(), ';'; }
try { echo fail_string(); echo 'unreachable'; } catch (Error $e) { echo $e->getMessage(), ';'; }
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19993}}
JSON
php "$TMP/site/index.php" > "$TMP/expected"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/app" > "$TMP/build.log" 2>&1 || { cat "$TMP/build.log"; exit 1; }
"$TMP/app" > "$TMP/log" 2>&1 & PID=$!
for i in $(seq 1 100); do if curl -fsS http://127.0.0.1:19993/ > "$TMP/actual" 2>/dev/null; then break; fi; sleep 0.1; done
test -f "$TMP/actual" || { cat "$TMP/log"; exit 1; }
diff -u "$TMP/expected" "$TMP/actual"
echo 'typed function exception PHP differential: passed'
