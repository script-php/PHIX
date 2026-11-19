#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$user = ['name' => 'Alex', 'age' => 25];
$user['age'] = 26;
$skills = ['PHP', 'Rust'];
$skills[] = 'JavaScript';
$nested = [[1, 2], [3, 4]];
echo $user['name'], '|', $user['age'], '|', $skills[2], '|', $nested[1][0];
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19572}}
JSON
cd "$ROOT"
cargo build --offline -q
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/out" 2>"$TMP/err" & PID=$!
for i in $(seq 1 40); do
 if curl -fsS 'http://127.0.0.1:19572/' >"$TMP/page"; then break; fi
 sleep 0.1
done
[[ "$(cat "$TMP/page")" == 'Alex|26|JavaScript|3' ]]
kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; PID=""
cat > "$TMP/site/index.php" <<'PHP'
<?php
$a = [5 => 'five', 'name' => 'Alex', '8' => 'eight', '08' => 'zero-eight', 'last'];
echo $a[5], '|', $a['name'], '|', $a[8], '|', $a['08'], '|', $a[9];
PHP
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/mixed-app"
"$TMP/deploy/mixed-app" >"$TMP/mixed.out" 2>"$TMP/mixed.err" & MIXED_PID=$!
for i in $(seq 1 40); do
 if curl -fsS 'http://127.0.0.1:19572/' >"$TMP/mixed.page"; then break; fi
 sleep 0.1
done
kill "$MIXED_PID" 2>/dev/null || :
[[ "$(cat "$TMP/mixed.page")" == 'five|Alex|eight|zero-eight|last' ]]
echo 'Array syntax integration passed'
