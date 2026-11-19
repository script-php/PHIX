#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ [[ -z "$PID" ]] || { kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; }; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
try { throw new InvalidArgumentException('bad'); } catch (LogicException $e) { echo 'logic'; }
try { throw new BadMethodCallException('bad'); } catch (BadFunctionCallException $e) { echo '|bad-function'; }
try { throw new OutOfBoundsException('bad'); } catch (RuntimeException $e) { echo '|runtime'; }
try { throw new TypeError('bad'); } catch (Exception $e) { echo '|wrong'; } catch (Error $e) { echo '|error'; }
try { throw new DomainException('bad'); } catch (RuntimeException $e) { echo '|wrong'; } catch (Exception $e) { echo '|exception'; }
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19799}}
JSON
EXPECTED="$(php "$TMP/site/index.php")"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 80); do
 if curl -fsS 'http://127.0.0.1:19799/' >"$TMP/response" 2>/dev/null; then break; fi
 sleep 0.1
done
ACTUAL="$(cat "$TMP/response")"
[[ "$ACTUAL" == "$EXPECTED" ]] || { echo "expected <$EXPECTED>, got <$ACTUAL>" >&2; exit 1; }
echo "SPL exception inheritance matches PHP: $ACTUAL"
