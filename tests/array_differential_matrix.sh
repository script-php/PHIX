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
$a = []; $b = [1]; $c = ["x" => 1]; $d = ["x" => 1];
echo "empty_null:", $a == null, "\n";
echo "empty_false:", $a == false, "\n";
echo "nonempty_true:", $b == true, "\n";
echo "strict_same:", $c === $d, "\n";
echo "loose_same:", $c == $d, "\n";
echo "strict_different:", $a !== $b, "\n";
echo "order_length:", $a < $b, "\n";
echo "order_scalar:", $b > 0, "\n";
echo "numeric_string:", ["10"] > [2], "\n";
echo "huge_integer:", [9007199254740993] > ["9007199254740992"], "\n";
echo "overflow:", ["1e309"] < ["2e309"], "\n";
$copy = ["plain" => 10]; $copy2 = $copy; $copy2["plain"] = 20;
echo "copy_plain:", $copy["plain"], "\n";
$refvalue = 10; $refs = ["ref" => &$refvalue, "plain" => 20]; $refs2 = $refs;
$refs2["ref"] = 30; $refs2["plain"] = 40;
echo "copy_reference:", $refvalue, ":", $refs["plain"], "\n";
$nested = ["child" => ["value" => 1]];
$alias = &$nested["child"]["value"];
unset($nested["child"]["value"]);
$alias = 9; $nested["child"]["value"] = 3;
echo "detached_alias:", $alias, ":", $nested["child"]["value"], "\n";
// Expanded PHP array differential coverage (append history, key coercion, references).
$negative = []; $negative[-5] = 7; $negative[] = 8;
echo "negative_append:", $negative[-4], "\n";
$holes = [3 => "three", 8 => "eight"]; unset($holes[8]); $holes[] = "next";
echo "unset_append:", $holes[9], "\n";
$keys = ["08" => 1, "8" => 2, "-0" => 3, "0" => 4];
echo "numeric_key_coercion:", $keys["08"], ":", $keys[8], ":", $keys["-0"], ":", $keys[0], "\n";
$boolkeys = [true => "yes", false => "no", null => "empty"];
echo "bool_null_keys:", $boolkeys[1], ":", $boolkeys[0], ":", $boolkeys[""], "\n";
$duplicate = ["x" => 1, "y" => 2, "x" => 3];
echo "duplicate_key:", $duplicate["x"], ":", $duplicate["y"], "\n";
$deep = ["outer" => ["inner" => 4]]; $deep2 = $deep; $deep2["outer"]["inner"] = 7;
echo "nested_copy:", $deep["outer"]["inner"], ":", $deep2["outer"]["inner"], "\n";
$scalar = 10; $arrayref = ["value" => &$scalar]; $copyref = $arrayref; $copyref["value"] = 22;
echo "scalar_ref_copy:", $scalar, ":", $arrayref["value"], "\n";
$original = ["child" => ["n" => 1]]; $detached = &$original["child"]["n"];
unset($original["child"]); $detached = 6; $original["child"] = ["n" => 9];
echo "detached_parent:", $detached, ":", $original["child"]["n"], "\n";
$compare1 = ["a" => 1, "b" => 2]; $compare2 = ["b" => 2, "a" => 1];
echo "unordered_keys:", $compare1 == $compare2, ":", $compare1 === $compare2, "\n";
echo "numeric_precision:", ["9007199254740993"] > ["9007199254740992"], "\n";
$negative2 = [-3 => "first"]; $negative2[] = "second"; unset($negative2[-3]); $negative2[] = "third";
echo "negative_history:", $negative2[-2], ":", $negative2[-1], "\n";
$aliasRoot = ["left" => 1]; $aliasOther = ["right" => 2];
$alias = &$aliasRoot; $oldAlias = &$alias; $alias = &$aliasOther;
$alias["right"] = 4;
echo "reference_rebind:", $oldAlias["left"], ":", $aliasOther["right"], "\n";
$plain = ["a" => 1]; $plain["a"] = 2; unset($plain["a"]); $plain["a"] = 3;
echo "replace_after_unset:", $plain["a"], "\n";
$huge1 = [9007199254740993]; $huge2 = ["9007199254740992"];
echo "large_int_equality:", $huge1 == $huge2, ":", $huge1 > $huge2, "\n";
// PHP 8.x key coercion boundaries and duplicate-key ordering.
$boundary = ["0" => 1, "00" => 2, "-0" => 3, "+1" => 4, " 1" => 5, "1 " => 6];
echo "key_boundary_zero:", $boundary[0], ":", $boundary["00"], ":", $boundary["-0"], "\n";
echo "key_boundary_sign:", $boundary["+1"], ":", $boundary[" 1"], ":", $boundary["1 "], "\n";
$largekeys = ["9223372036854775807" => 1, "9223372036854775808" => 2];
echo "key_boundary_overflow:", $largekeys[9223372036854775807], ":", $largekeys["9223372036854775808"], "\n";
$dupekeys = ["1" => 1, "01" => 2, true => 3];
echo "key_duplicate_coercion:", $dupekeys[1], ":", $dupekeys["01"], "\n";
PHP
cat > "$TMP/site/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19876}}
JSON
php "$TMP/site/index.php" > "$TMP/expected"
"$ROOT/target/debug/phix" check "$TMP/site"
"$ROOT/target/debug/phix" build "$TMP/site" -o "$TMP/deploy/app"
"$TMP/deploy/app" >"$TMP/stdout" 2>"$TMP/stderr" & PID=$!
for i in $(seq 1 100); do
  if curl -fsS 'http://127.0.0.1:19876/' > "$TMP/actual" 2>/dev/null; then break; fi
  sleep 0.1
done
if [[ ! -f "$TMP/actual" ]]; then cat "$TMP/stderr" >&2; exit 1; fi
if ! diff -u "$TMP/expected" "$TMP/actual"; then echo 'Differential matrix FAILED' >&2; exit 1; fi
echo "PHP array differential matrix: $(wc -l < "$TMP/expected") cases matched"
