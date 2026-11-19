#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$number = 10;
$text = "before";
$flag = true;
$one = ["ref" => &$number, "plain" => 20];
$two = ["ref" => &$text];
$three = ["ref" => &$flag];
$copy = $one;
$copy["ref"] = 25;
$copy["plain"] = 99;
$two["ref"] = "after";
$three["ref"] = false;
echo $number, "|", $text, "|", $flag, "|", $one["plain"], "|", $copy["plain"];
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19695}}
JSON
EXPECTED="$(php "$TMP/site/index.php")"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 80); do
 if curl -fsS 'http://127.0.0.1:19695/' >"$TMP/response" 2>/dev/null; then break; fi
 sleep 0.1
done
ACTUAL="$(cat "$TMP/response")"
if [[ "$ACTUAL" != "$EXPECTED" ]]; then
 printf 'PHP expected: <%s>\nPhix actual: <%s>\n' "$EXPECTED" "$ACTUAL" >&2
 exit 1
fi
echo "Scalar array literal references match PHP: $ACTUAL"
