#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$data = ["name" => "Alex"];
$ref = &$data["name"];
unset($data["name"]);
$ref = "John";
echo $data["name"], "|", $ref, "|";
$data["name"] = "Jane";
echo $data["name"], "|", $ref;
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19679}}
JSON
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 60); do
 if curl -fsS 'http://127.0.0.1:19679/' >"$TMP/response" 2>/dev/null; then break; fi
 sleep 0.1
done
[[ "$(cat "$TMP/response")" == '|John|Jane|John' ]]
echo 'Array cell identity after deletion integration passed'
