#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$user = ["profile" => ["name" => "Alex"], "count" => 1];
$name = &$user["profile"]["name"];
$name = "John";
echo $user["profile"]["name"], "|";
$other = &$user["count"];
$other = 7;
echo $user["count"], "|";
$copy = $user;
$name = "Jane";
echo $copy["profile"]["name"], "|", $user["profile"]["name"];
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19674}}
JSON
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 60); do
 if curl -fsS 'http://127.0.0.1:19674/' >"$TMP/response" 2>/dev/null; then break; fi
 sleep 0.1
done
EXPECTED="$(php "$TMP/site/index.php")"
[[ "$(cat "$TMP/response")" == "$EXPECTED" ]]
echo 'Array element reference integration passed'
