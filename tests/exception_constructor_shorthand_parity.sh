#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ [[ -z "$PID" ]] || { kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; }; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
try { throw new InvalidArgumentException; } catch (LogicException $e) { echo 'statement'; }
try { $value = false || throw new RuntimeException; } catch (RuntimeException $e) { echo '|expression'; }
try { throw new Exception('message', 7); } catch (Exception $e) { echo '|arguments'; }
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19801}}
JSON
EXPECTED="$(php "$TMP/site/index.php")"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 80); do
 if curl -fsS 'http://127.0.0.1:19801/' >"$TMP/response" 2>/dev/null; then break; fi
 sleep 0.1
done
ACTUAL="$(cat "$TMP/response")"
[[ "$ACTUAL" == "$EXPECTED" ]] || { echo "expected <$EXPECTED>, got <$ACTUAL>" >&2; exit 1; }
echo "Constructor shorthand matches PHP: $ACTUAL"
