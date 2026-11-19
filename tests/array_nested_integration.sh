#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$user = ['profile' => ['name' => 'Alex', 'age' => 25], 'skills' => ['PHP']];
$user['profile']['age'] = 26;
$user['skills'][] = 'Rust';
unset($user['profile']['name'], $user['skills'][0]);
$user['skills'][] = 'Go';
echo $user['profile']['age'], '|', $user['skills'][1], '|', $user['skills'][2];
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19580}}
JSON
cd "$ROOT"
cargo build --offline -q
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/out" 2>"$TMP/err" & PID=$!
for i in $(seq 1 50); do
 if curl -fsS 'http://127.0.0.1:19580/' >"$TMP/page" 2>/dev/null; then break; fi
 sleep 0.1
done
[[ "$(cat "$TMP/page")" == '26|Rust|Go' ]]
echo 'Nested array mutation integration passed'
