#!/usr/bin/env bash
# Differential regression: PHP CLI is the oracle, Phix standalone HTTP is the subject.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; PID=""
cleanup(){ [[ -z "$PID" ]] || { kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; }; rm -rf "$TMP"; }
trap cleanup EXIT
mkdir -p "$TMP/site" "$TMP/deploy"
cat > "$TMP/site/index.php" <<'PHP'
<?php
$a = [2 => "left", "x" => 10];
$b = [2 => "right", "y" => 20];
$c = $a + $b;
echo "left_wins:", $c[2], ":", $c["x"], ":", $c["y"], "\n";
$first = [5 => "five"] + [9 => "nine"];
$first[] = "next";
echo "append:", $first[10], "\n";
$ref = 3;
$left = ["ref" => &$ref];
$combined = $left + ["extra" => 4];
$combined["ref"] = 7;
echo "reference:", $ref, ":", $combined["extra"], "\n";
$original = ["value" => 10];
$copy = $original + ["extra" => 2];
$copy["value"] = 30;
echo "copy:", $original["value"], ":", $copy["value"], "\n";
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19888}}
JSON
php "$TMP/site/index.php" > "$TMP/expected"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 100); do
  if curl -fsS 'http://127.0.0.1:19888/' > "$TMP/actual" 2>/dev/null; then break; fi
  sleep 0.1
done
if [[ ! -f "$TMP/actual" ]]; then cat "$TMP/stderr" >&2; exit 1; fi
if ! diff -u "$TMP/expected" "$TMP/actual"; then echo 'Differential matrix FAILED' >&2; exit 1; fi
echo "PHP array union differential: $(wc -l < "$TMP/expected") cases matched"
