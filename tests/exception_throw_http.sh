#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"; TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }; trap cleanup EXIT
mkdir -p "$TMP/site"
cat > "$TMP/site/index.php" <<'PHP'
<?php
try { throw new Exception("first"); } catch (Exception $e) { echo $e["message"]; }
echo ":";
try { try { throw new TypeError("nested"); } catch (TypeError $e) { throw $e; } } catch (Error $e) { echo $e["message"]; } finally { echo ":finally"; }
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19983}}
JSON
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/app" > "$TMP/build.log" 2>&1 || { cat "$TMP/build.log"; exit 1; }
"$TMP/app" > "$TMP/log" 2>&1 & PID=$!
for i in $(seq 1 50); do if curl -fsS http://127.0.0.1:19983/ > "$TMP/actual" 2>/dev/null; then break; fi; sleep 0.1; done
printf 'first:nested:finally' > "$TMP/expected"; diff -u "$TMP/expected" "$TMP/actual"; echo 'throw/catch/rethrow HTTP regression: passed'
