#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$empty = [];
$nonempty = [1];
$value0 = null;
echo $empty < $value0, $empty > $value0, $empty <= $value0, $empty >= $value0, "|";
echo $nonempty < $value0, $nonempty > $value0, $nonempty <= $value0, $nonempty >= $value0, "|";
$value1 = false;
echo $empty < $value1, $empty > $value1, $empty <= $value1, $empty >= $value1, "|";
echo $nonempty < $value1, $nonempty > $value1, $nonempty <= $value1, $nonempty >= $value1, "|";
$value2 = true;
echo $empty < $value2, $empty > $value2, $empty <= $value2, $empty >= $value2, "|";
echo $nonempty < $value2, $nonempty > $value2, $nonempty <= $value2, $nonempty >= $value2, "|";
$value3 = 0;
echo $empty < $value3, $empty > $value3, $empty <= $value3, $empty >= $value3, "|";
echo $nonempty < $value3, $nonempty > $value3, $nonempty <= $value3, $nonempty >= $value3, "|";
$value4 = "";
echo $empty < $value4, $empty > $value4, $empty <= $value4, $empty >= $value4, "|";
echo $nonempty < $value4, $nonempty > $value4, $nonempty <= $value4, $nonempty >= $value4, "|";
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19786}}
JSON
EXPECTED="$(php "$TMP/site/index.php")"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 100); do
 if curl -fsS 'http://127.0.0.1:19786/' >"$TMP/response" 2>/dev/null; then break; fi
 sleep 0.1
done
ACTUAL="$(cat "$TMP/response")"
if [[ "$ACTUAL" != "$EXPECTED" ]]; then
 printf 'PHP expected: <%s>\nPhix actual: <%s>\n' "$EXPECTED" "$ACTUAL" >&2
 exit 1
fi
echo "Array/scalar ordering matches PHP: $ACTUAL"
