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
$values = [null, false, true, 0, ""];
$nil = $values[0]; $no = $values[1]; $yes = $values[2]; $zero = $values[3]; $blank = $values[4];
echo $empty == $nil, '|', $empty == $no, '|', $empty == $yes, '|',
     $nonempty == $yes, '|', $nonempty == $no, '|', $nonempty == $nil, '|',
     $empty == $zero, '|', $empty == $blank, '|',  $no == $empty, '|',  $yes == $nonempty;
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19706}}
JSON
EXPECTED="$(php "$TMP/site/index.php")"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 100); do
 if curl -fsS 'http://127.0.0.1:19706/' >"$TMP/response" 2>/dev/null; then break; fi
 sleep 0.1
done
ACTUAL="$(cat "$TMP/response")"
if [[ "$ACTUAL" != "$EXPECTED" ]]; then
 printf 'PHP expected: <%s>\nPhix actual: <%s>\n' "$EXPECTED" "$ACTUAL" >&2
 exit 1
fi
echo "Array bool/null comparisons match PHP: $ACTUAL"
