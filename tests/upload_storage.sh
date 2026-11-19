#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
export PATH="${RUST_BIN:-/mnt/data/rust_local/bin}:$PATH"
T="$(mktemp -d)"; PID=''
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$T"; }
trap cleanup EXIT
cd "$ROOT"
cargo build --release --offline --quiet
mkdir -p "$T/site/public" "$T/site/storage/uploads" "$T/deploy"
cat > "$T/site/phix.json" <<'JSON'
{"entry":"public/index.php","routing":{"document_root":".","deny":[],"fallback":"/public/index.php"},"storage":{"uploads":"storage/uploads","max_file_size":1048576}}
JSON
cat > "$T/site/public/index.php" <<'PHP'
<?php
if ($_FILES['picture']['name']) {
  if (move_uploaded_file($_FILES['picture']['tmp_name'], 'saved.bin')) {
    echo 'UPLOADED:' . $_FILES['picture']['name'];
  }
}
echo 'DONE';
PHP
cat > "$T/site/public/escape.php" <<'PHP'
<?php echo move_uploaded_file($_FILES['picture']['tmp_name'], '../escape.bin');
PHP
cat > "$T/site/public/symlink.php" <<'PHP'
<?php echo move_uploaded_file($_FILES['picture']['tmp_name'], 'link/escape.bin');
PHP
printf 'initial-file' > "$T/site/storage/uploads/existing.txt"
printf 'hidden-secret' > "$T/site/storage/uploads/secret.txt"
cat > "$T/deploy/config.json" <<'JSON'
{"runtime":{"port":19578,"bind":"127.0.0.1"}}
JSON
"$ROOT/target/release/phix" build "$T/site" -o "$T/deploy/app"
[[ "$(cat "$T/deploy/storage/uploads/existing.txt")" == 'initial-file' ]]
[[ -f "$T/deploy/storage/uploads/.phix-storage-initialized" ]]
# No build should embed uploads, even if a dev removes all deny patterns.
! grep -aFq 'hidden-secret' "$T/deploy/app"
"$T/deploy/app" > "$T/out" 2> "$T/err" & PID=$!
for i in $(seq 1 45); do if curl -fsS http://127.0.0.1:19578/public/index.php > "$T/page" 2>/dev/null; then break; fi; sleep .1; done
[[ "$(cat "$T/page")" == DONE ]]
[[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:19578/storage/uploads/secret.txt)" == 403 ]]
printf '\x00\x01binary\xff' > "$T/payload.bin"
[[ "$(curl -fsS -F picture=@"$T/payload.bin" http://127.0.0.1:19578/public/index.php)" == 'UPLOADED:payload.binDONE' ]]
cmp "$T/payload.bin" "$T/deploy/storage/uploads/saved.bin"
[[ "$(curl -fsS -F picture=@"$T/payload.bin" http://127.0.0.1:19578/public/escape.php)" == "" ]]
[[ ! -f "$T/deploy/storage/escape.bin" ]]
ln -s "$T" "$T/deploy/storage/uploads/link"
[[ "$(curl -fsS -F picture=@"$T/payload.bin" http://127.0.0.1:19578/public/symlink.php)" == "" ]]
[[ ! -f "$T/escape.bin" ]]
rm "$T/deploy/storage/uploads/link"
# Never replace existing file.
[[ "$(curl -fsS -F picture=@"$T/payload.bin" http://127.0.0.1:19578/public/index.php)" == DONE ]]
# Requests cannot write outside storage even if the application passes a bad path.
kill "$PID";wait "$PID" 2>/dev/null || :;PID=''
rm "$T/deploy/storage/uploads/existing.txt"
"$ROOT/target/release/phix" build "$T/site" -o "$T/deploy/app" > /dev/null
[[ ! -f "$T/deploy/storage/uploads/existing.txt" ]]
cmp "$T/payload.bin" "$T/deploy/storage/uploads/saved.bin"
# Without configuration, uploads must be rejected and no storage folder created.
python3 - "$T/site/phix.json" <<'PY'
import json,sys
p=sys.argv[1];d=json.load(open(p));d.pop('storage');open(p,'w').write(json.dumps(d))
PY
mkdir -p "$T/deploy2"
printf '{"runtime":{"port":19579,"bind":"127.0.0.1"}}' > "$T/deploy2/config.json"
"$ROOT/target/release/phix" build "$T/site" -o "$T/deploy2/app" >/dev/null
[[ ! -d "$T/deploy2/storage/uploads" ]]
"$T/deploy2/app" > "$T/out2" 2> "$T/err2" & PID=$!
for i in $(seq 1 45); do if curl -fsS http://127.0.0.1:19579/public/index.php > /dev/null 2>&1; then break; fi; sleep .1; done
[[ "$(curl -s -o /dev/null -w '%{http_code}' -F picture=@"$T/payload.bin" http://127.0.0.1:19579/public/index.php)" == 403 ]]
[[ ! -d "$T/deploy2/storage/uploads" ]]
# Invalid paths must never be accepted at build time.
cat > "$T/site/phix.json" <<'JSON'
{"entry":"public/index.php","storage":{"uploads":"../outside"}}
JSON
if "$ROOT/target/release/phix" build "$T/site" -o "$T/deploy3/app" > "$T/bad" 2>&1; then echo 'bad upload path accepted';exit 1;fi
printf 'Phix upload storage integration passed\n'
