#!/usr/bin/env bash
# Run after installing Rust toolchain: cargo test and live generated-binary checks.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"
PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
cd "$ROOT"
cargo test
cargo build --release
PHIX="$ROOT/target/release/phix"
mkdir -p "$TMP/project/lib" "$TMP/output"
cat > "$TMP/project/index.php" <<'PHP'
<?php
require_once("lib/a.php"); require_once "lib/a.php";
echo "Value: ", add(2), "\n";
echo "Port: ", $_PHIX["runtime"]["port"], "\n";
echo "Third: ", $_PHIX["application"]["nested"]["third"], "\n";
if ($_PHIX["application"]["enabled"]) { echo "Enabled\n"; }
PHP
cat > "$TMP/project/lib/a.php" <<'PHP'
<?php
function add(int $a): int { return $a + 40; }
PHP
cat > "$TMP/project/config.json" <<'JSON'
{"runtime":{"port":19519,"bind":"127.0.0.1"},"application":{"enabled":true,"nested":{"third":"arbitrary"}}}
JSON
printf 'body { color: teal; }\n' > "$TMP/project/site.css"
"$PHIX" check "$TMP/project"
"$PHIX" build "$TMP/project" -o "$TMP/output/app"
cmp "$TMP/project/config.json" "$TMP/output/config.json"
"$TMP/output/app" >"$TMP/out" 2>"$TMP/err" & PID=$!
for i in $(seq 1 40); do if curl --silent --fail http://127.0.0.1:19519/ >"$TMP/page"; then break; fi; sleep .1; done
grep -q 'Value: 42' "$TMP/page"
grep -q 'Port: 19519' "$TMP/page"
grep -q 'Third: arbitrary' "$TMP/page"
grep -q 'Enabled' "$TMP/page"
curl --fail --silent http://127.0.0.1:19519/site.css | grep -q 'color: teal'
kill "$PID"; wait "$PID" || :; PID=""
rm -rf "$TMP/project"
"$TMP/output/app" >"$TMP/out2" 2>"$TMP/err2" & PID=$!
for i in $(seq 1 40); do if curl --silent --fail http://127.0.0.1:19519/ >"$TMP/page2"; then break; fi; sleep .1; done
grep -q 'Value: 42' "$TMP/page2"
grep -q 'Third: arbitrary' "$TMP/page2"
cat > "$TMP/invalid.php" <<'PHP'
<?php
$_PHIX = 123;
PHP
if "$PHIX" check "$TMP/invalid.php"; then echo 'Expected read-only config rejection' >&2; exit 1; fi
printf 'Migration regression tests passed\n'
