#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
T="$(mktemp -d)"; PID=''
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$T"; }
trap cleanup EXIT
export PATH="${RUST_BIN:-/mnt/data/rust_local/bin}:$PATH"
cd "$ROOT"
cargo build --release --offline --quiet
mkdir -p "$T/site/public/assets" "$T/site/public/includes" "$T/site/vendor" "$T/deploy"
cat > "$T/site/phix.json" <<'JSON'
{
  "entry":"public/index.php",
  "routing": {
    "document_root":"public",
    "deny":["/includes/**","/private/**"],
    "rewrites":[{"from":"/user/:id","to":"/profile.php?id=:id"}],
    "redirects":[{"from":"/old","to":"/new","status":301}],
    "fallback":"/index.php"
  }
}
JSON
cat > "$T/site/public/index.php" <<'PHP'
<?php echo 'FRONT:' . $_SERVER['REQUEST_URI'] . '|' . $_SERVER['SCRIPT_NAME'];
PHP
cat > "$T/site/public/profile.php" <<'PHP'
<?php echo 'PROFILE:' . $_GET['id'] . '|' . $_SERVER['REQUEST_URI'] . '|' . $_SERVER['SCRIPT_NAME'];
PHP
cat > "$T/site/public/login.php" <<'PHP'
<?php echo 'LOGIN';
PHP
cat > "$T/site/public/includes/secret.php" <<'PHP'
<?php echo 'SHOULD NOT BE SERVED';
PHP
cat > "$T/site/public/private.php" <<'PHP'
<?php echo 'PUBLIC TOP LEVEL';
PHP
printf 'css-test' > "$T/site/public/assets/site.css"
printf 'internal-secret' > "$T/site/vendor/secret.txt"
printf 'dont-serve' > "$T/site/public/config.json"
cat > "$T/deploy/config.json" <<'JSON'
{"runtime":{"port":19576,"bind":"127.0.0.1"}}
JSON
"$ROOT/target/release/phix" build "$T/site" -o "$T/deploy/app"
"$T/deploy/app" > "$T/out" 2> "$T/err" & PID=$!
for i in $(seq 1 40); do if curl -fsS "http://127.0.0.1:19576/" > "$T/page"; then break; fi; sleep .1; done
[[ "$(cat "$T/page")" == 'FRONT:/|/index.php' ]]
[[ "$(curl -fsS http://127.0.0.1:19576/login.php)" == 'LOGIN' ]]
[[ "$(curl -fsS 'http://127.0.0.1:19576/user/42?origin=test')" == 'PROFILE:42|/user/42?origin=test|/profile.php' ]]
[[ "$(curl -fsS 'http://127.0.0.1:19576/some/framework/route')" == 'FRONT:/some/framework/route|/index.php' ]]
[[ "$(curl -fsS 'http://127.0.0.1:19576/assets/site.css')" == 'css-test' ]]
[[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:19576/includes/secret.php)" == 403 ]]
[[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:19576/config.json)" == 403 ]]
[[ "$(curl -fsS http://127.0.0.1:19576/vendor/secret.txt)" == "FRONT:/vendor/secret.txt|/index.php" ]]
[[ "$(curl --path-as-is -s -o /dev/null -w '%{http_code}' http://127.0.0.1:19576/%2e%2e/config.json)" == 400 ]]
[[ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:19576/%2Fvendor%2Fsecret.txt)" == 400 ]]
[[ "$(curl -s -D "$T/headers" -o /dev/null http://127.0.0.1:19576/old -w '%{http_code}')" == 301 ]]
grep -qi '^Location: /new' "$T/headers"
kill "$PID"; wait "$PID" 2>/dev/null || :; PID=''
# No fallback => unknown should be 404. Denied rewrite targets must fail at build time.
python3 - "$T/site/phix.json" <<'PY'
import json,sys
p=sys.argv[1];d=json.load(open(p));d['routing'].pop('fallback');open(p,'w').write(json.dumps(d))
PY
"$ROOT/target/release/phix" build "$T/site" -o "$T/deploy/app" >/dev/null
python3 - "$T/site/phix.json" <<'PY'
import json,sys
p=sys.argv[1];d=json.load(open(p));d['routing']['rewrites']=[{'from':'/oops','to':'/includes/secret.php'}];open(p,'w').write(json.dumps(d))
PY
if "$ROOT/target/release/phix" build "$T/site" -o "$T/deploy/app" >"$T/badlog" 2>&1; then echo 'SECURITY ERROR: denied rewrite target accepted'; exit 1; fi
grep -q 'rewrite target' "$T/badlog"
echo 'Phix routing integration passed'
