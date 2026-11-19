#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$a = ["count" => 1];
$b = &$a;
$b["count"] = 5;
echo $a["count"], "|";
$c = $a;
$c["count"] = 9;
echo $a["count"], "|", $c["count"], "|";
$b = ["count" => 7];
echo $a["count"];
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19673}}
JSON
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 60); do
 if curl -fsS 'http://127.0.0.1:19673/' >"$TMP/response" 2>/dev/null; then break; fi
 sleep 0.1
done
[[ "$(cat "$TMP/response")" == '5|5|9|7' ]]
echo 'Array variable reference integration passed'
