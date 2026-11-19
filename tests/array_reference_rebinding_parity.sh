#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$a = ["value" => 1];
$b = ["value" => 2];
$ref = &$a;
$old = &$ref;
$ref = &$b;
$ref["value"] = 3;
echo $a["value"], "|", $b["value"], "|", $old["value"], "|", $ref["value"], "|";
$left = ["key" => 10];
$right = ["key" => 20];
$alias = &$left["key"];
$former = &$alias;
$alias = &$right["key"];
$alias = 30;
echo $left["key"], "|", $right["key"], "|", $former;
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19837}}
JSON
EXPECTED="$(php "$TMP/site/index.php")"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 100); do
 if curl -fsS 'http://127.0.0.1:19837/' >"$TMP/response" 2>/dev/null; then break; fi
 sleep 0.1
done
ACTUAL="$(cat "$TMP/response")"
if [[ "$ACTUAL" != "$EXPECTED" ]]; then
 printf 'PHP expected: <%s>\nPhix actual: <%s>\n' "$EXPECTED" "$ACTUAL" >&2
 exit 1
fi
echo "Array reference rebinding matches PHP: $ACTUAL"
