#!/usr/bin/env bash
# Formatting equivalence must be checked against actual compiled websites.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"
PID=""
cleanup() {
  if [[ -n "$PID" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi
  rm -rf "$TMP"
}
trap cleanup EXIT
cd "$ROOT"
cargo build --release --offline
mkdir -p "$TMP/lib"
cat > "$TMP/lib/math.php" <<'PHP'
<?php
function twice(int $x): int { return $x * 2; }
PHP
cat > "$TMP/normal.php" <<'PHP'
<?php
require_once 'lib/math.php';
function result(int $input): int {
    $answer = twice($input);
    if ($answer > 0) {
        return $answer;
    }
    return 0;
}
$name = $_GET['name'];
echo 'Hello ', htmlspecialchars($name), ' | ', result(21), ' | ', $_SERVER['REQUEST_METHOD'];
PHP
cat > "$TMP/compact.php" <<'PHP'
<?php require_once('lib/math.php');function result(int $input):int{$answer=twice($input);if($answer>0){return $answer;}return 0;}$name=$_GET['name'];echo 'Hello ',htmlspecialchars($name),' | ',result(21),' | ',$_SERVER['REQUEST_METHOD'];
PHP
cat > "$TMP/odd.php" <<'PHP'
<?php
# PHP-style comments and arbitrary whitespace between tokens
require_once
(
  'lib/math.php'
)
;
function
result
(
  int
  $input
)
:
int
{
/* block comment
   over multiple lines */
$answer
=
twice
(
$input
)
;
if
(
$answer
>
0
)
{
return
$answer
;
}
return 0;
}
$name = $_GET
[
'name'
];
echo
'Hello '
,
htmlspecialchars
(
$name
)
,
' | '
,
result
(
21
)
,
' | '
,
$_SERVER
[
'REQUEST_METHOD'
]
;
PHP
# Tabs, comments between operators/arguments, CRLF line endings.
printf '<?php\r\n\trequire_once\t"lib/math.php"; // module\r\nfunction result (int $input) : int {\r\n $answer = twice ( $input );\r\n if ($answer > 0) {return $answer;}\r\n return 0;\r\n}\r\n$name=$_GET["name"]; echo "Hello ", htmlspecialchars($name), " | ", result(21), " | ", $_SERVER["REQUEST_METHOD"];\r\n' > "$TMP/tabs.php"

expected='Hello &lt;Jane&amp;Joe&gt; | 42 | GET'
for variant in normal compact odd tabs; do
  site="$TMP/$variant-site"
  mkdir -p "$site/lib" "$TMP/$variant-deploy"
  cp "$TMP/$variant.php" "$site/index.php"
  cp "$TMP/lib/math.php" "$site/lib/math.php"
  # Static port scoped per process; deploy_config should preserve the provided file.
  cat > "$site/config.json" <<'JSON'
{"runtime":{"port":19568,"bind":"127.0.0.1"}}
JSON
  "$ROOT/target/release/phix" build "$site" -o "$TMP/$variant-deploy/app"
  "$TMP/$variant-deploy/app" > "$TMP/$variant.out" 2> "$TMP/$variant.err" & PID=$!
  ready=0
  for i in $(seq 1 60); do
    if curl --silent --fail --max-time 1 'http://127.0.0.1:19568/?name=%3CJane%26Joe%3E' > "$TMP/$variant.response"; then ready=1; break; fi
    sleep .1
  done
  if [[ $ready -ne 1 ]]; then cat "$TMP/$variant.err" >&2; echo "HTTP startup failed: $variant" >&2; exit 1; fi
  response="$(cat "$TMP/$variant.response")"
  if [[ "$response" != "$expected" ]]; then printf 'Mismatch in %s:\nexpected: %s\nactual:   %s\n' "$variant" "$expected" "$response" >&2; exit 1; fi
  kill "$PID" 2>/dev/null || :
  wait "$PID" 2>/dev/null || :
  PID=""
done
printf 'PHP whitespace compatibility passed: 4 layouts, equivalent compiled HTTP responses\n'
