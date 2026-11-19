#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
cd "$ROOT"
cargo build --release
"$ROOT/target/release/phix" check examples/web
"$ROOT/target/release/phix" build examples/web -o "$TMP/app"
printf '{"runtime":{"port":19512,"bind":"127.0.0.1"},"website":{"name":"Rust Parity"}}\n' > "$TMP/config.json"
"$TMP/app" >"$TMP/out" 2>"$TMP/err" & PID=$!
sleep 0.5
curl --fail --silent http://127.0.0.1:19512/ >"$TMP/page"
grep -q '<h1>Rust Parity</h1>' "$TMP/page"
grep -q '127.0.0.1:19512' "$TMP/page"
grep -q 'multipart/form-data' "$TMP/page"
curl --fail --silent http://127.0.0.1:19512/assets/site.css | grep -q 'color-scheme'
[[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:19512/missing.css)" == "404" ]]
echo 'Rust frontend HTTP smoke test passed'
