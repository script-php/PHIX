#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="${RUST_BIN:-/mnt/data/rust_local/bin}:$PATH"
TMP="$(mktemp -d)"; PID=''
cleanup(){ if [[ -n "$PID" ]];then kill "$PID" 2>/dev/null || :;wait "$PID" 2>/dev/null || :;fi;rm -rf "$TMP"; }
trap cleanup EXIT
cd "$ROOT"
cargo build --release --offline --quiet
cp -a examples/web "$TMP/site"
mkdir "$TMP/deploy"
cat > "$TMP/deploy/config.json" <<'JSON'
{"runtime":{"port":19619,"bind":"127.0.0.1"},"website":{"name":"Demo <Phix>"}}
JSON
"$ROOT/target/release/phix" build "$TMP/site" -o "$TMP/deploy/app"
[[ -f "$TMP/deploy/storage/uploads/README.txt" ]]
! grep -aFq 'Example persistent upload directory' "$TMP/deploy/app"
"$TMP/deploy/app" > "$TMP/server.log" 2>&1 & PID=$!
for i in $(seq 1 50);do if curl -fsS http://127.0.0.1:19619/index.php -o "$TMP/page" 2>/dev/null;then break;fi;sleep .1;done
grep -q 'Demo &lt;Phix&gt;' "$TMP/page"
grep -q 'multipart/form-data' "$TMP/page"
curl -fsS http://127.0.0.1:19619/assets/site.css | grep -q 'color-scheme'
[[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:19619/storage/uploads/README.txt)" == 403 ]]
printf 'uploaded-file-bytes\x00\xff' > "$TMP/payload.bin"
curl -fsS -F "photo=@$TMP/payload.bin" http://127.0.0.1:19619/index.php | grep -q 'Upload saved to persistent storage'
cmp "$TMP/payload.bin" "$TMP/deploy/storage/uploads/photo-upload.bin"
[[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:19619/storage/uploads/photo-upload.bin)" == 403 ]]
curl -fsS -F "photo=@$TMP/payload.bin" http://127.0.0.1:19619/index.php | grep -q 'Upload not saved'
kill "$PID"; wait "$PID" 2>/dev/null || :; PID=''
"$ROOT/target/release/phix" build "$TMP/site" -o "$TMP/deploy/app" > /dev/null
cmp "$TMP/payload.bin" "$TMP/deploy/storage/uploads/photo-upload.bin"
printf 'Web example upload integration passed\n'
