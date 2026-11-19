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
$a = ["settings" => ["theme" => "dark"]];
$a["settings"] += ["theme" => "light", "language" => "en"];
echo $a["settings"]["theme"], "|", $a["settings"]["language"], "\n";
$alias = &$a["settings"];
$a["settings"] += ["extra" => 12];
echo $alias["extra"], "\n";
$nested = ["outer" => ["inner" => ["a" => 1]]];
$nested["outer"]["inner"] += ["a" => 9, "b" => 2];
echo $nested["outer"]["inner"]["a"], "|", $nested["outer"]["inner"]["b"], "\n";
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19891}}
JSON
php "$TMP/site/index.php" > "$TMP/expected"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 100); do
  if curl -fsS 'http://127.0.0.1:19891/' > "$TMP/actual" 2>/dev/null; then break; fi
  sleep 0.1
done
if [[ ! -f "$TMP/actual" ]]; then cat "$TMP/stderr" >&2; exit 1; fi
if ! diff -u "$TMP/expected" "$TMP/actual"; then echo 'Differential matrix FAILED' >&2; exit 1; fi
echo "Nested array += differential: $(wc -l < "$TMP/expected") cases matched"
