#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
cd "$ROOT"
cargo build --release
PHIX="$ROOT/target/release/phix"
mkdir -p "$TMP/project/src" "$TMP/deploy"
printf '<?php\necho "Hello from custom entry";\n' > "$TMP/project/src/start.php"
printf '{"entry":"src/start.php"}\n' > "$TMP/project/phix.json"
printf '{"runtime":{"port":19511,"bind":"127.0.0.1"},"database":{"host":"localhost"}}\n' > "$TMP/project/config.json"
"$PHIX" build "$TMP/project" -o "$TMP/deploy/app"
cmp "$TMP/project/config.json" "$TMP/deploy/config.json"
"$PHIX" build "$TMP/project" -o "$TMP/deploy/app"
cmp "$TMP/project/config.json" "$TMP/deploy/config.json"
printf '{"runtime":{"port":19513,"bind":"127.0.0.1"},"database":{"password":"production-secret"}}\n' > "$TMP/deploy/config.json"
"$PHIX" build "$TMP/project" -o "$TMP/deploy/app"
grep -q 'production-secret' "$TMP/deploy/config.json"
rm "$TMP/deploy/config.json"
rm "$TMP/project/config.json"
"$PHIX" build "$TMP/project" -o "$TMP/deploy/app"
grep -q '"port": 9510' "$TMP/deploy/config.json"
grep -q '"bind": "127.0.0.1"' "$TMP/deploy/config.json"
# Invalid project configs must not be deployed.
rm "$TMP/deploy/config.json"
printf '{"runtime":{"port":"bad"}}\n' > "$TMP/project/config.json"
if "$PHIX" build "$TMP/project" -o "$TMP/deploy/app"; then echo 'Expected invalid port rejection' >&2; exit 1; fi
[[ ! -e "$TMP/deploy/config.json" ]]
# Reject invalid runtime objects and bindings.
for invalid in '{"runtime":12}' '{"runtime":{"bind":"not-an-ip"}}' '{"runtime":{"port":65536}}'; do
  printf '%s\n' "$invalid" > "$TMP/project/config.json"
  if "$PHIX" build "$TMP/project" -o "$TMP/deploy/app"; then echo "Expected invalid runtime rejection: $invalid" >&2; exit 1; fi
  [[ ! -e "$TMP/deploy/config.json" ]]
done
# Root-level port and bind are application settings, not reserved server fields.
printf '{"port":"application-owned","bind":"application-owned"}\n' > "$TMP/project/config.json"
"$PHIX" build "$TMP/project" -o "$TMP/deploy/app"
cmp "$TMP/project/config.json" "$TMP/deploy/config.json"
echo 'Config deployment tests passed' 
