#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$integer = 10;
$alias = &$integer;
$alias = 25;
$string = "old";
$textAlias = &$string;
$textAlias = "new";
$boolean = true;
$flagAlias = &$boolean;
$flagAlias = false;
echo $integer, "|", $string, "|", $boolean, "|", $alias, "|", $textAlias;
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19704}}
JSON
EXPECTED="$(php "$TMP/site/index.php")"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 80); do
 if curl -fsS 'http://127.0.0.1:19704/' >"$TMP/response" 2>/dev/null; then break; fi
 sleep 0.1
done
ACTUAL="$(cat "$TMP/response")"
if [[ "$ACTUAL" != "$EXPECTED" ]]; then
 printf 'PHP expected: <%s>\nPhix actual: <%s>\n' "$EXPECTED" "$ACTUAL" >&2
 exit 1
fi
echo "Native scalar variable references match PHP: $ACTUAL"
