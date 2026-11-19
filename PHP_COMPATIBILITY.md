# Phix ↔ PHP compatibility ledger

This file tracks **known differences**, unsupported PHP features, and intentional divergences. It is not a claim of full PHP compatibility. Keep entries until tests demonstrate parity; don't remove a difference merely because an implementation was started.

## Maintenance rules

1. Record each difference with an ID, PHP behavior, Phix behavior, affected versions, status, and a reproducer.
2. Use **Not implemented**, **Different**, **Partial**, **Intentional**, or **Verified parity**. A passing isolated unit test does not prove full parity.
3. Prefer a runnable PHP baseline and an equivalent Phix test. Document the PHP version used.
4. If a difference is fixed, retain its entry and link to regression tests; update its status.
5. Check this file and `documentation.md` in every milestone involving PHP semantics.

## Known differences

### COMPAT-001 — PHP array literals and dynamic values
- **Status:** Not implemented in compiled PHP source
- **PHP:** `$a = ['name' => 'Alex']; echo $a['name'];` works.
- **Phix:** `src/php/value.rs` has internal value/array support, but the parser and generated website runtime do not yet use it.
- **Plan:** Integrate dynamic values, array literals, indexing, and assignments with end-to-end tests.

### COMPAT-002 — PHP array key coercion
- **Status:** Partial (internal only)
- **PHP:** Integer-looking canonical decimal strings become integer keys; `"08"`, `"+8"`, and `"-0"` remain string keys; booleans, null, and finite floats are converted. Some float conversions emit warnings.
- **Phix:** `ArrayKey::from_php_value` implements common conversions, but does not emit PHP warnings, and does not cover objects/resources. Not wired into compiled source.
- **Reproducer:** `php -r '$a=[]; foreach (["8","08","+8","-8","-0",null,true,3.9] as $k) {$a[$k]=1;} var_export(array_keys($a));'`

### COMPAT-003 — Array append next-index tracking
- **Status:** Partial (internal implementation improved; not connected to compiled PHP)
- **PHP:** Append uses a historically tracked next integer index, including after deletions. Negative-key behavior changed in PHP 8.3.
- **Phix:** `PhpArray` now stores historical `next_index` and preserves it across removals. It supports insertion order, replacement, and basic PHP 8.3+ negative-key behavior. Integer overflow handling may differ. `unset` and PHP array literals remain unsupported in compiled source.
- **Regression tests:** `php::value::tests::{deletion_keeps_historical_append_index,insertion_order_and_key_replacement,negative_keys_follow_php_83_plus,array_append_and_overflow}`.
- **Reference:** PHP 8.4.24 CLI: `php -r '$a=[];$a[5]=1;unset($a[5]);$a[]=2;var_export(array_keys($a));'` returns `[6]`.
- **Plan:** Connect this representation to the parser/runtime, then test overflow and PHP-version edge cases.

### COMPAT-004 — Array-to-string conversion warnings
- **Status:** Different (internal only)
- **PHP:** Casting an array to a string produces `Array` and an `Array to string conversion` warning.
- **Phix:** `Value::php_string` returns `Array` without emitting a warning.
- **Plan:** Add structured PHP-compatible diagnostics at runtime.

### COMPAT-005 — PHP scalar conversions and comparisons
- **Status:** Partial
- **PHP:** Implements loose/strict comparisons, numeric strings, float formatting, overflow, and PHP-specific conversions.
- **Phix:** The value module has basic truthiness and scalar string conversion, not complete PHP coercion/comparison semantics. The existing compiler uses its own restricted type model.
- **Plan:** Add reference-based conversion and comparison tests before wiring into generated code.

### COMPAT-006 — PHP classes, exceptions, and namespaces
- **Status:** Not implemented / incomplete
- **PHP:** Supports object-oriented constructs, exceptions, namespaces, and autoloading.
- **Phix:** These are not implemented in the current language subset.

### COMPAT-007 — PHP database APIs
- **Status:** Not implemented (intentionally deferred)
- **PHP:** Supports PDO, MySQLi, and SQLite drivers depending on extensions.
- **Phix:** Database integration is deferred until later milestones, by project decision.

### COMPAT-008 — PHP built-in library coverage
- **Status:** Partial
- **PHP:** Exposes a broad standard library with extension-dependent functions.
- **Phix:** The built-in registry covers only a small subset. Names beginning `asset_` are Phix-specific, not PHP standard functions.
- **Plan:** Expand modular built-ins after value semantics are stable.

### COMPAT-009 — Upload and filesystem capability restrictions
- **Status:** Intentional
- **PHP:** File access depends on server configuration, permissions, and PHP settings.
- **Phix:** Upload destinations and writable storage are explicitly configured, with restrictions intended to reduce accidental exposure. These restrictions can reject PHP programs that assume arbitrary filesystem access.
- **Plan:** Preserve secure defaults and document compatibility impact for each capability.

### COMPAT-010 — Error reporting parity
- **Status:** Partial
- **PHP:** Has PHP-specific parse errors, warnings, notices, exceptions, and runtime diagnostics.
- **Phix:** Compiler diagnostics are being developed; PHP-equivalent warning and exception semantics are not complete.
- **Plan:** Compare parser diagnostics and runtime errors with PHP reference cases.

## Reference environment

Reference comparisons should state the exact PHP version (`php -v`) and the Phix commit or milestone. PHP behavior changes between versions; do not assume all PHP releases are identical.

### COMPAT-011 — Array literal and indexing support (partial)

PHP supports indexed and associative array literals, mutable elements, append, references, and notices/warnings for invalid offsets. Phix now compiles **indexed** array literals (`[]`, `[1, 'x', true, null]`), nested indexed arrays, and read-only numeric indexing (`$items[0]`) using the generated runtime's `Json::Array` representation. Missing/out-of-range numeric indices currently produce `null` silently rather than PHP's undefined-array-key warning. PHP-style associative literals (`=>`), array element assignments (`$a[0] = ...`), `$a[]` append, and references are **not yet implemented in source syntax**. String-key indexing only applies to JSON-style objects already present in the runtime, not associative PHP array literals. The compiler's separate `php/value.rs` model is not yet wired into generated code; `Json` is a temporary bridge, not full PHP-array semantics.

**Status:** partial, needs PHP parity tests and migration to shared value model.

### COMPAT-012 — Source-level associative arrays and mutations (partial)

**PHP:** Arrays are one ordered map of integer and string keys; they permit mixed keys, sparse indices, key coercion, and preserve next-index history. Missing-key reads can emit warnings. Assignments can target nested paths.

**Phix now:** The compiler accepts homogeneous indexed literals (`[1,2]`) and string-keyed associative literals (`['name' => 'Alex']`), reads with integer/string keys including chained reads, replaces existing elements, appends to indexed arrays, and adds/replaces associative string keys. Generated websites still use `Json::Array`/`Json::Object` rather than the internal `php::value::Value` representation. **Differences:** mixed keyed/unkeyed literals rejected; numeric keys in keyed literals rejected; missing keys return `null` silently; out-of-range indexed writes can be ignored; append only operates on dense indexed arrays; nested assignment is not supported. These are compatibility gaps, not PHP-equivalent behavior. Phix must not be used as a drop-in replacement for applications relying on these semantics yet.

**Example:** `$user = ['name' => 'Alex']; $user['name'] = 'Sam'; echo $user['name'];` produces `Sam`.

**Status:** Partially implemented. **Next:** replace generated `Json` values with the PHP array/value runtime and implement PHP-compatible key coercion, index history, and diagnostics.


### COMPAT-013 — Generated PHP-source array storage (current milestone)

**Status:** Partial; supersedes the generated-storage descriptions in COMPAT-011 and COMPAT-012. Those earlier sections are historical milestone records.

**PHP:** A single ordered array map supports mixed integer/string keys, numeric-string key coercion, sparse assignments, and historical append indices.

**Phix now:** PHP array literals generate `Json::PhpArray(PhpArray::from_parts(...))` using `runtime/php_array.rs`. Mixed keyed/unkeyed entries and integer-keyed entries compile. Canonical integer strings such as `"8"` become integer keys, while `"08"` remains a string key. Sparse assignments, insertion order, replacement, and numeric append indices work in the new runtime. JSON configuration parsing still uses `Json::Array` and `Json::Object`, deliberately separate from PHP-source arrays. This is a transitional implementation: `php/value.rs` is **not** yet the source of truth for generated values, and the runtime has a second PHP-array implementation.

**Still different:** `unset()` source syntax, nested writes, PHP warnings for undefined offsets, array references/copy-on-write parity, all invalid-key errors, numeric overflow reporting, array comparisons, PHP-compatible array-to-string warnings, and the full array function library. Invalid offset types and index overflow currently can be silently ignored in the generated runtime. This is not production-compatible PHP array behavior.

**Verification:** 33 Rust unit tests passed; 12 shell integration suites passed including a compiled HTTP website with mixed keys and numeric-string coercion. PHP differential suite is not yet comprehensive.

**Next:** Consolidate the duplicated `PhpArray` models into one reusable runtime type, then implement deletion and nested writes with PHP differential tests.

### COMPAT-014 — Single-element `unset()` for PHP arrays

**PHP:** `unset($array[$key])` removes an array element without resetting its historical next automatic numeric index. Unsetting a missing key is allowed. PHP also supports multiple `unset()` arguments, nested paths, and variables.

**Phix now:** The compiler accepts a single `unset($array[$key]);` statement when `$array` is an array-compatible local variable and `$key` is an integer or string expression. The generated `PhpArray` removes that key while preserving insertion order and next-index history. Example: `$a=[5=>'old']; unset($a[5]); $a[]='new'; echo $a[6];` prints `new`.

**Still different:** `unset($a['nested']['key'])`, multiple arguments, variable unsetting, dynamic keys of other PHP-supported types, and PHP warning/error parity are not implemented. Missing-key reads still return null silently. The compiler's shared `Value` array type and generated `PhpArray` remain separate implementations.

**Verification:** 35/35 Rust unit tests passed, 13/13 shell integration suites passed, including a compiled website HTTP test for `unset()` and subsequent append. This is not a comprehensive PHP differential test suite.

### COMPAT-015 — Nested array mutation and multiple unset operands (partial)

**PHP behavior:** PHP permits nested element assignment, nested append, and `unset()` with multiple operands, including nested elements. PHP also has reference, coercion, and warning semantics that depend on the existing value type.

**Phix behavior:** The generated runtime now supports paths such as `$a['profile']['age'] = 26`, `$a['skills'][] = 'Rust'`, and `unset($a['profile']['name'], $a['skills'][0])`. Intermediate missing/null values are currently converted into empty Phix arrays. This is not equivalent to PHP for every scalar or invalid-key case. The runtime still wraps source arrays in `Json::PhpArray` rather than using the separate internal `Value` representation.

**Status:** Partial; add differential tests for scalar intermediate nodes, missing paths, references, invalid keys, and PHP warnings before claiming parity.

### COMPAT-016 — Boolean/null keys and independent array copies

**PHP behavior:** Boolean keys become integer 0/1; null keys become the empty string; canonical integer strings are converted to integer keys. Assignment by value copies array contents logically, including nested arrays; mutations to the copy do not affect the original. PHP also supports references (`=&`), which intentionally share changes.

**Phix now:** The compiler accepts boolean and null array-key expressions. The generated `PhpKey::from_value` normalizes these key types, and array variable assignment clones the `Json::PhpArray` value (including nested arrays). The new `tests/array_copy_keys_integration.sh` builds and serves a standalone website that verifies these cases, including nested copy independence.

**Still different:** PHP references are unsupported, invalid key errors/warnings are incomplete, the compiler still uses `Json::PhpArray` instead of the internal `Value` system, and general PHP array comparisons and complete copy-on-write performance semantics are not implemented. This is a correctness milestone for value copies, not full PHP array compatibility.

**Verification:** 35/35 Rust unit tests passed and the new compiled HTTP integration test passed with Rust 1.100. A full shell regression run reached all 15 scripts but exceeded the execution timeout before a final aggregate pass count could be reported.

### COMPAT-017 — Strict PHP array identity (partial resolution)

- **PHP:** `===` compares arrays recursively, requiring the same key/value pairs in the same iteration order and identical value types. `!==` is its negation. Loose `==` and `!=` use different rules and ignore entry order.
- **Phix now:** `===` and `!==` work for generated `Json::PhpArray` values and nested PHP arrays, including strict scalar comparisons within arrays. Verified by a compiled website test compared against installed PHP.
- **Still different:** Loose array comparisons are not supported; PHP references, float literals and full warning/error semantics remain incomplete. PHP array comparisons against other PHP value types are not yet supported through source syntax. Internal JSON config arrays/objects are not treated as PHP arrays.
- **Test:** `phix_rust/tests/array_strict_comparison.sh`.
- **Status:** Partially resolved; retain until complete PHP comparison compatibility.

### COMPAT-018 — Loose array equality (partial resolution)

PHP `==` and `!=` compare array entries by key without requiring matching insertion order, and compare corresponding values loosely. Phix now implements this behavior recursively for its PHP-source array representation, including common scalar conversions (`true`, `false`, `null`, numeric strings, integers, and floats). A compiled website differential test is in `phix_rust/tests/array_loose_comparison.sh`.

**Not yet parity-certified:** all PHP 8 numeric-string edge cases, warnings/errors for unusual comparisons, cyclic references, and reference semantics. PHP's broader scalar comparison rules are only approximated; avoid claiming complete array compatibility. The comparison does not change JSON configuration parsing.

### COMPAT-019 — Loose array comparison of null and strings
PHP 8.x compares `null` with strings as an empty string for loose equality: `null == ""` is true, while `null == "0"`, `null == "0.0"`, and `null == " "` are false. Earlier Phix compared these values by truthiness, incorrectly treating `null` and `"0"` as equal. The generated PHP array runtime now handles this case, including recursively nested values. Verified by a Rust unit test and a compiled-website PHP differential test. Other loose comparison cases still require comprehensive verification.

### COMPAT-020 — PHP array references (`=&`) — runtime foundation only

**PHP behavior:** `$b = &$a` makes two variable names share the same variable slot; `$b = $a` instead performs a value assignment (arrays have copy-on-write value semantics). References to array elements, nested references, and rebinding have additional observable rules.

**Current Phix behavior:** `runtime/php_reference.rs` introduces `PhpSlot`, a reference-counted, interior-mutable variable slot with independent `copy_value()` and shared `bind_reference()` operations. Rust tests cover independent copies, shared mutation and deletion, replacing a copied value, and nested-copy isolation. **The compiler does not yet lower `=&` or array-element references to these slots, and generated website code does not yet use this module.** This is an architectural prerequisite, not PHP source-level reference support.

**Follow-up:** Integrate reference-capable slots into generated variables without changing ordinary assignment semantics; implement `=&`, element references and nested references; verify against PHP CLI and compiled website HTTP tests. Handle alias rebinding, reference lifetimes, and array copy-on-write edge cases.

### COMPAT-021 — Top-level array variable references (partial)

PHP `$b = &$a` creates a shared variable binding. Phix now supports this form when `$a` contains an array-compatible `Json` value, and both variable names refer to the same `PhpSlot`. Array element changes, deletion, append, and whole-value replacement propagate through the binding. Ordinary `$b = $a` still copies array values. Verified in a compiled HTTP website with the output `5|5|9|7`.

**Remaining differences:** References to array elements (`$b = &$a['key']`), nested element references, reference parameters/returns, aliases involving scalars and all PHP types, references inside arrays, and complete rebinding/unset semantics are not yet implemented. Generated functions still use their earlier typed parameter model. This is not full PHP reference compatibility.

### COMPAT-022 — References to array elements (partial)

- **PHP behavior:** `$alias = &$array['key']` and nested element aliases refer to the same underlying element. References can survive `unset`, be stored in arrays, and have identity distinct from a simple key path.
- **Phix behavior:** The compiler now accepts reference assignment to an array element or nested element. Generated `PhpSlot` aliases track the root array plus a key path; writes through aliases update the parent, and normal copies remain independent in tested cases.
- **Difference:** Path-based aliases are not equivalent to PHP reference containers: deleting or replacing a referenced parent element, moving referenced values, storing references inside arrays, or reference rebinding may behave differently. These cases are **not yet PHP-compatible**.
- **Reproduction:** `$a=['x'=>1]; $b=&$a['x']; unset($a['x']); $b=2;` must not recreate `$a['x']` in PHP; Phix's path alias can recreate it.
- **Status:** Partial; requires identity-preserving reference cells within array entries.

### COMPAT-023 — Identity-preserving element cells (foundation only)

- **PHP behavior:** References to elements remain valid after `unset($array[$key])`; reinserting the same key creates a distinct element rather than reconnecting the old alias.
- **Phix behavior:** New `runtime/php_reference_cell.rs` provides `PhpCell` and `PhpCellEntries` with stable `Rc<RefCell<Json>>` identity. Tests verify alias survival after deletion, independent reinsertion under the same key, and alias-preserving replacement of an existing element.
- **Difference:** This is an **experimental, internal reference container** and is **not yet connected** to compiler-generated PHP array operations. Source-level references still use path-based `PhpSlot` and therefore remain incompatible with these cases. The prototype does not yet handle nested identity, copy-on-write references within arrays, or PHP cycle semantics.
- **Next:** Migrate generated `PhpArray` entries to reference-aware cells while preserving value-copy semantics and numeric index tracking, then run PHP differential tests for deletion/reinsertion and nested references.


### COMPAT-024 — Direct array-element reference identity after unset (partially resolved)
- **PHP:** A reference to an element remains valid after `unset($array[$key])`. Recreating that key creates a distinct element; aliases to the old cell remain detached. Ordinary copies can retain reference containers in certain cases.
- **Phix:** Generated PHP arrays now hold thread-safe reference cells. Direct element aliases survive deletion and reinsertion; ordinary deep copies are independent. Verified in the compiled-website HTTP integration test.
- **Remaining:** Nested aliases still use path traversal, and arrays containing references do not fully match PHP's copy/reference semantics. Recursive references, rebinding, and error parity need differential testing.
- **Status:** Partial. Do not claim full PHP array compatibility.

### COMPAT-025 — Nested array element reference identity (partially resolved)

- **PHP behavior:** References to nested elements retain identity after deletion or parent replacement; reusing the same key does not reconnect the old alias.
- **Phix behavior:** `PhpSlot::bind_element` now resolves nested `PhpArray` cells without cloning intermediate arrays. `Json::phix_ensure_path_cell` traverses `PhpCell` identities, and `PhpCell::with_mut` permits safe nested mutation. Three Rust unit tests and `tests/array_nested_cell_identity.sh` verify deletion, parent replacement, and alias identity. The compiled HTTP response matched PHP CLI: `John|Jane|Detached|New`.
- **Remaining:** References stored inside arrays, copy-on-write involving embedded references, cycles, reference rebinding, and complete PHP error/warning behavior remain incomplete. Invalid reference paths may still use a path-based fallback. The separate `Value` and `Json` representations remain unmerged.
- **Status:** Partial, not full PHP array compatibility.

### COMPAT-026 — Embedded references preserved across array copies (partially resolved)

- **PHP behavior:** When an array element is a reference, copying the array preserves the reference identity of that element; non-reference elements are copied independently. Nested referenced elements also retain their identity.
- **Phix behavior:** `PhpCell` now tracks whether it has been bound as a PHP reference. `PhpArray::clone()` retains the cell identity for referenced entries and copies ordinary entries. `PhpSlot::bind_element()` marks successfully bound cells. This corrects a previous regression test that incorrectly assumed referenced entries were detached during array copies.
- **Verification:** Rust unit suite, `tests/array_embedded_reference_copy.sh` comparing compiled website output to PHP CLI, and the existing array integration tests.
- **Remaining differences:** Array literal reference expressions (`["x" => &$value]`) are not yet compiled, reference cycles and alias rebinding are incomplete, and complete PHP warning/error behavior and all value conversions remain unverified. The shared `Value` model is not fully integrated.
- **Status:** Partial. Full PHP array compatibility is not established.

### COMPAT-027 — References inside array literals (partially resolved)

- **PHP behavior:** Array literals may contain references such as `["ref" => &$a["value"]`. Copies preserve reference identity for referenced entries while ordinary entries are copied independently.
- **Phix behavior:** The compiler now supports array-literal references to **existing PHP array elements**. It emits `PhpCell` entries and uses `PhpArray::from_cell_parts`, preserving shared element identity. `tests/array_literal_element_reference.sh` compares a compiled website HTTP response against PHP CLI (`30|30|5|99`).
- **Remaining differences:** Array-literal references to whole variables (`[&$variable]`), references to other array entries within the same literal, reference cycles, alias rebinding, reference parameters/returns, and PHP warnings/errors are not yet fully supported.
- **Status:** Partial; do not claim full array compatibility.

### COMPAT-028 — Whole-variable references inside array literals (partially resolved)

- **PHP behavior:** `['ref' => &$array]` embeds an alias to the entire array variable. Mutating that element or copying the containing array retains the alias.
- **Phix behavior:** The generated `PhpSlot` root now uses an identity-preserving `PhpCell`; `reference_cell()` can alias the root variable as well as nested elements. The compiler accepts `&$array` in array literals. The differential HTTP integration `tests/array_literal_whole_variable_reference.sh` matches PHP output `30|30|30`. All 14 array integration suites and 56 Rust unit tests passed in this milestone.
- **Remaining differences:** Scalar variable references in literals, cyclic/self-referential arrays, full reference rebinding, function reference parameters and returns, precise warnings and errors, and comprehensive array API parity remain incomplete. Array semantics are not yet fully PHP-compatible.
- **Status:** Partial.

### COMPAT-029 — Nested mutations preserve PHP reference cells (partial)

- Fixed: nested set, append and unset traverse the original PHP array element cell, avoiding accidental disconnection of references through cloned intermediate arrays.
- Tests: three Rust regression tests; compiled website PHP differential test `array_nested_mutation_identity.sh`.
- Remaining: scalar variable references, cycles, rebinding, function reference parameters and returns, warning/error parity, and comprehensive differential coverage. Full array compatibility is **not** established.

### COMPAT-030 — Floating-point array key conversion (improved)

- **PHP behavior:** Floating-point array keys truncate toward zero and convert to signed platform integers; out-of-range values wrap modulo 2^64 on the tested 64-bit PHP 8.4 runtime. NaN and infinities become integer key zero.
- **Phix behavior:** `PhpKey::from_value` now uses in-range truncation, explicit out-of-range wrapping, and zero for non-finite values rather than Rust's saturating float-to-integer cast. Unit cases cover positive and negative fractions, NaN, infinities, 1e19, -1e19 and 1e20.
- **Remaining differences:** Full PHP 8.x array semantics are not complete; scalar variable references in array literals, reference cycles/rebinding, PHP warnings and errors, and broader differential coverage remain.
- **Status:** Partial; tested against PHP 8.4 on a 64-bit runtime.

### COMPAT-030 — Scalar references stored in array literals (partial)

- PHP supports references to variables of any value type in array literals, including integers, strings, booleans, null, and arrays.
- Phix currently supports this syntax for variables represented by `PhpSlot` (`Ty::Json`), including null-initialized variables. A compiled-website differential test (`tests/array_literal_null_scalar_reference.sh`) verifies reference identity across an array copy and independent copying of ordinary elements (`25|25|10|99`).
- Phix still rejects `&$value` when `$value` is compiled as a native integer, string, or boolean. It must eventually promote all referenceable variables to shared cells without losing PHP scalar semantics. Recursive arrays, cyclic references, rebinding, and full PHP warnings/errors remain incomplete.
- **Status: partial; PHP array compatibility is not complete.**

### COMPAT-031 — Native scalar variable references (partial)

- **PHP behavior:** Integer, string, and boolean variables can become reference targets; assignment through another variable bound by `=&` updates the original.
- **Phix behavior:** A previously native scalar target of `$alias = &$target` is promoted into a shared `PhpSlot` holding `Json::Int`, `Json::Text`, or `Json::Bool`. Later variable reads and assignments use the shared cell. The compiled HTTP differential test `tests/array_native_scalar_variable_references.sh` checks all three types against PHP CLI.
- **Remaining differences:** Scalar targets referenced *directly in array literals* (`[&$scalar]`) are still rejected. Existing native-typed destination variables, reference rebinding semantics, reference cycles, PHP error/warning behavior, and other advanced array-reference cases remain incomplete. This is not full PHP array compatibility.
- **Status:** Partial.

### COMPAT-032 — Native scalar references in array literals

- **Implemented:** `&$variable` inside an array literal now promotes previously native integer, string, and boolean bindings to shared `PhpSlot` cells before the literal is evaluated. A copied array retains referenced-element identity while ordinary elements copy independently.
- **Verified:** Rust 1.100 `cargo test --offline` (65/65), `cargo build --offline`, and `tests/array_literal_scalar_reference.sh` comparing PHP CLI output to a compiled Phix HTTP response (`25|after||20|99`).
- **Not complete:** Reference cycles, reference rebinding, arbitrary reference expressions and PHP warning/error semantics remain incomplete. The promotion scan is currently limited to simple assignment statements; full PHP 8.x array compatibility is not established.

### COMPAT-033 — Array versus boolean/null loose equality (resolved for JSON-backed operands)

- **PHP 8.4 reference:** Empty arrays compare loosely equal to `null` and `false`; nonempty arrays compare loosely equal to `true`. Array comparisons against numeric zero and empty strings remain false. Comparisons are symmetric.
- **Phix implementation:** `Json::phix_loose_eq` now handles array/bool and array/null comparisons before rejecting other non-array operands.
- **Verification:** 66/66 Rust tests passed. `tests/array_boolean_null_loose_comparison.sh` compared PHP CLI output with a compiled Phix HTTP response and passed. Existing loose, strict and embedded-reference-copy integration suites passed.
- **Remaining differences:** PHP scalar variables not represented as `Json` cannot currently be compared directly against `Json` arrays in the compiler; the differential test obtains scalar operands from an array. Cyclic comparisons, warning/error parity, and comprehensive array differential coverage remain incomplete.
- **Status:** Partial; arrays are not fully PHP-compatible.

### COMPAT-034 — Nested mutation through a referenced parent element (resolved for tested operations)

- **PHP behavior:** Assigning, appending, or unsetting through `$parent = &$array['parent']` must mutate the original parent element without breaking aliases to its children. Removing a child detaches its alias; reinserting the same key creates a new cell.
- **Phix change:** `PhpSlot::set_path`, `append_path`, and `unset_path` now call `PhpCell::with_mut` when the slot resolves to an element cell. Previously they read a cloned array and wrote it back, potentially disconnecting nested element aliases.
- **Verified:** Rust 1.100 offline unit suite (69/69); PHP CLI versus compiled HTTP `tests/array_nested_slot_mutation.sh` (`7|2|5|9|3`); three existing reference-array HTTP regression scripts passed. Compiler build passed.
- **Remaining:** Self-referential cycles can deadlock on recursive cell locks; cyclic comparisons and copies, reference rebinding, mixed-representation comparisons, and comprehensive PHP warnings/errors are not complete. Full PHP array compatibility is not established.

### COMPAT-035 — Reference cycle inspection (partial)

Added `Json::phix_has_reference_cycle`, which traverses reference-cell identities with an active-path set and avoids recursive mutex acquisition. Tests cover a self-referential array, repeated noncyclic aliases, and ordinary nested arrays. This is a foundational inspection utility only: PHP-compatible recursive comparison errors, cyclic serialization, cloning, mutation and garbage collection are **not** implemented. Do not claim full array compatibility.

### COMPAT-036 — Checked recursive array comparison (partial)

- Added `Json::phix_checked_array_eq` as a runtime preflight for strict and loose array comparisons. Cyclic references return `Err("Nesting level too deep - recursive dependency?")` instead of entering the existing recursive comparison and potentially deadlocking.
- Two Rust unit tests verify self-reference detection and unchanged acyclic equality behavior.
- **Not complete:** compiler-generated comparison expressions still call the unchecked comparison methods. This API is not yet wired to PHP-compatible runtime exception propagation. The preflight also conservatively rejects any cyclic operand, even if PHP would short-circuit before traversing that cycle. Additional differential tests are required.

### COMPAT-037 — Compiler-generated checked equality (partial)

- **Implemented:** For two `Json` operands, the compiler now emits `phix_checked_array_eq` for `==`, `!=`, `===`, and `!==`. It preserves negation for inequality and performs cycle inspection before recursive comparison.
- **Verified:** Rust 1.100 offline unit tests (74/74) and the compiled HTTP strict-array comparison differential test against PHP.
- **Remaining differences:** The preflight rejects any cyclic operand rather than comparing until a recursive dependency is actually reached; PHP can short-circuit some comparisons. Recursion errors currently cause a Rust panic rather than PHP's exception behavior. Mixed native/Json comparisons remain restricted. This does not establish full array compatibility.

### COMPAT-040 — Pairwise checked recursive comparison
- **Implemented:** `Json::phix_checked_array_eq` compares arrays lazily, tracking active pairs of element-cell identities rather than rejecting every graph containing a cycle. An earlier mismatching element returns `false`; revisiting an active comparison pair reports `Nesting level too deep - recursive dependency?`.
- **Verified:** Two new Rust unit tests (early mismatch and reached recursion); 76/76 Rust tests pass. PHP CLI independently confirms that mismatching first elements return `false` and comparing equivalent cyclic arrays raises `Error`.
- **Remaining:** Generated applications still convert the checked error to a Rust panic rather than a PHP `Error`. Cyclic copying, non-array recursion forms, warning/error parity, and comprehensive PHP differential coverage remain incomplete.
- **Status:** Partial; not 100% PHP array compatible.

### COMPAT-041 — Native scalar operands in array equality (resolved for tested cases)

- **PHP behavior:** PHP arrays can be compared with boolean, integer, and string operands in either operand order using `==`, `!=`, `===`, and `!==`.
- **Phix behavior:** Compiler-generated equality now promotes supported native scalar operands to `Json` before invoking the checked PHP-value comparison. A Rust compiler regression test covers both operand orders and all four operators; `tests/array_mixed_native_scalar_equality.sh` checks actual compiled HTTP output against PHP CLI.
- **Limitations:** Array ordering operators, recursive comparison error formatting, cyclic copying, broader scalar coercions, and PHP warning/error parity remain incomplete. This is not full array compatibility.

### COMPAT-042 — Identical reference-cell equality in cyclic arrays (partial)

- **PHP behavior verified:** `$a == $a` and `$a === $a` return `true` for a self-referential array; a copied array retaining the same embedded reference also compares equal. Comparing two separately constructed equivalent self-referential arrays throws `Error: Nesting level too deep - recursive dependency?`.
- **Phix fix:** Checked equality skips recursive traversal when the two array entries point to the exact same `PhpCell` identity. Distinct reference cells retain pairwise recursion detection. Updated a previously incorrect regression expectation and added two Rust regression tests.
- **Verification:** PHP CLI checks; 79/79 Rust tests pass using Rust 1.100; compiler build passes.
- **Remaining:** PHP-formatted recursion errors in generated applications, cyclic copying and garbage collection, ordering operators, reference rebinding, and comprehensive PHP 8.x differential testing remain incomplete. No claim of full array compatibility.

### COMPAT-043 — Recursive comparison request errors (partial)

- Recursive array comparisons now record a request-local error instead of panicking a Rust worker. The HTTP runtime responds with status 500 and a PHP-style `Fatal error: Uncaught Error: Nesting level too deep - recursive dependency?` message; it discards partial output.
- This is **not** full PHP exception compatibility: catchable `Error`, file/line/stack formatting, display_errors configuration, and exact PHP error handling are still missing.
- Equality comparisons on mixed scalar/array operands continue to use the checked comparison function.

### COMPAT-044 — Checked comparisons through nested JSON-backed collections (partial)

- **Fixed:** `phix_checked_array_eq` now recursively traverses `Json::Array` and `Json::Object` children with the same active reference-pair guard used for PHP arrays. Previously, these containers could delegate to unchecked equality, potentially revisiting recursive cells.
- **Verified:** Two Rust tests cover recursion detection and early mismatch short-circuiting; the existing compiled HTTP recursion-error integration test passes.
- **Remaining:** PHP catchable `Error` objects, exact diagnostic formatting, PHP-specific array ordering, cyclic copying/rebinding, and exhaustive differential testing remain incomplete. The JSON-backed containers are internal representations, not a claim that PHP has distinct JSON array/object semantics.
- **Status:** Partial; not full PHP 8.x array compatibility.

### COMPAT-045 — Cycle-safe Rust debug output (resolved for diagnostic traversal)

- **Problem:** Automatically derived Rust `Debug` for `PhpCell` traversed its contained `Json`. A PHP array holding a reference to itself could trigger unbounded formatting and stack overflow.
- **Change:** `PhpCell` now implements non-recursive `Debug`, displaying only cell identity and whether it is marked as a reference. `PhpArray` and `Json` debug formatting therefore do not traverse cell values.
- **Verification:** Two new Rust tests for cyclic cell/array formatting; 83/83 Rust tests passed; compiler build passed; compiled recursive-comparison HTTP, loose-comparison, and strict-comparison integration suites passed.
- **Remaining:** This is diagnostic safety, not complete PHP array compatibility. PHP catchable Error semantics, cyclic copy/rebinding behavior, array ordering, PHP warnings and exhaustive differential tests remain incomplete.

### COMPAT-046 — Copying self-referential arrays (regression coverage)

PHP copying an array with a `self` element referring to the original array variable does not rebind the embedded reference to the copied variable. Mutating an ordinary key in the copy leaves the original unchanged, while the copied `self` entry continues to reference the original root. Verified the expected semantics using local PHP CLI and added two Rust regression tests for identity and mutation behavior. Both tests pass; this milestone does **not** establish full cyclic array compatibility. Remaining gaps include reference rebinding, recursion-safe serialization and error propagation, and broader differential tests.

### COMPAT-047 — PHP array ordering (partial)

- Implemented `Json::phix_array_order` for PHP arrays and compiler routing of `<`, `>`, `<=`, `>=` to `phix_checked_order`.
- Equal-length arrays with incompatible keys are unordered; insertion order is ignored for matching keys; length differences are compared first. Recursive element comparisons use an active reference-pair guard.
- Verified the compiled HTTP integration suite `tests/array_ordering_php_parity.sh` against the installed PHP CLI and three Rust regression tests.
- **Not complete:** Scalar mixed-type ordering, numeric-string coercion, full nested PHP ordering rules, PHP exceptions and warnings, and full recursive differential coverage remain incomplete. This is not full PHP 8.x array compatibility.

### COMPAT-048 — Array/scalar ordering (partially resolved)

- **PHP behavior:** Arrays compare to booleans and null through boolean conversion, while arrays order above numbers and strings regardless of whether the array is empty. Both operand orders matter.
- **Phix behavior:** `Json::phix_array_order` implements these rules for PHP arrays versus `Bool`, `Null`, `Int`, `Number`, and `Text`. Rust unit tests and `tests/array_ordering_scalar_parity.sh` compare compiled HTTP output against PHP CLI. Existing array ordering differential test also passes.
- **Remaining differences:** Mixed scalar values *inside* arrays, PHP numeric-string comparison subtleties, recursion and exception semantics, and comprehensive PHP 8.x array conformance remain incomplete. This is not full PHP compatibility.
- **Status:** Partially resolved, verified for the stated cases.

### COMPAT-049 — Mixed scalar ordering within arrays (partially resolved)

- **PHP behavior:** Array ordering compares corresponding values using PHP's scalar comparison rules, including numeric strings, boolean coercion, null/string comparisons, and PHP 8 number/non-numeric-string ordering.
- **Phix behavior:** `Json::phix_array_order` now handles these scalar combinations in both operand directions. Four Rust regression tests exercise numeric strings, nonnumeric strings, null, and booleans.
- **Remaining differences:** Full PHP numeric-string grammar, infinities/NaN, recursive array errors, complete reference rebinding, exception propagation, and exhaustive PHP differential testing remain incomplete.
- **Status:** Partial; do not claim full PHP array compatibility.

### COMPAT-050 — Reference rebinding and old-alias lifetime (tested cases)

- **PHP behavior:** Rebinding `$ref = &$other` redirects that variable's binding, without redirecting aliases previously created from `$ref`. The same applies to references bound to array elements.
- **Phix behavior:** Existing `PhpSlot` binding assignment preserves old aliases because each slot retains its original `PhpCell` identity. Two new Rust regression tests cover variable and element rebinding. `tests/array_reference_rebinding_parity.sh` compares the compiled HTTP output against PHP CLI for both cases.
- **Verification:** 95/95 Rust tests, new PHP differential HTTP suite, existing variable-reference and mixed-value ordering HTTP suites passed.
- **Remaining:** Rebinding in complex nested/cyclic graphs, scope teardown, invalid reference targets, PHP warnings/exceptions, and complete PHP 8.x array conformance are not established.
- **Status:** Partial; do not claim full PHP array compatibility.

### COMPAT-051 — Exact integer/numeric-string comparisons (partial)

- **PHP behavior:** Integer-form numeric strings that fit in signed 64-bit integers compare with integers without losing precision beyond 2^53.
- **Phix change:** Array ordering and loose equality now attempt exact i64 comparison for integer-form numeric strings before floating-point fallback. Added two Rust regression tests using 9007199254740992–9007199254740994.
- **Verification:** 97/97 Rust tests passed. This milestone has not undergone a compiled HTTP differential test.
- **Remaining:** Out-of-range numeric strings, floating-point precision, all PHP numeric-string grammar corner cases, cycles, reference rebinding, and PHP error behavior need broader differential testing. **Not fully compatible.**

### COMPAT-052 — Exact numeric-string comparisons within arrays
- **Status:** Partial; targeted Rust regression parity verified against PHP CLI for large decimal integers.
- **PHP behavior:** Numeric strings representing distinct large integers compare distinctly, including values beyond 2^53 and beyond signed 64-bit range. Signed and zero-padded decimal strings are numerically equivalent when appropriate.
- **Phix behavior:** The array scalar comparison path now compares plain signed decimal integer strings using normalized decimal digits rather than converting both to `f64`. This applies to loose equality and array ordering. Three new Rust regression tests cover precision, overflow-range values, and signs/zero padding.
- **Remaining differences:** PHP's complete numeric-string grammar, decimal/exponent precision, infinity/NaN, warnings, and all recursive array paths still need comprehensive differential testing. This is not a declaration of full PHP array compatibility.

### COMPAT-053 — Overflowed exponent-form numeric strings in arrays

- **PHP behavior:** Comparing distinct exponent-form strings whose floating-point parsing overflows (for example, `"1e309"` and `"2e309"`) retains their string comparison behavior instead of treating both as the same infinity.
- **Phix behavior:** String/string loose equality and ordering now use string comparison when floating-point conversion yields a non-finite value. Exact integer-form comparisons continue to use the existing precision-preserving path.
- **Verification:** Two Rust regression tests; compiled website `tests/array_overflow_numeric_string_parity.sh` compared with local PHP CLI (8.4.24). Rust suite: 102/102 passed.
- **Remaining differences:** Full PHP numeric-string grammar, all numeric overflow cases, warnings/errors, reference rebinding, recursive array exception handling, and broad PHP differential conformance remain incomplete.
- **Status:** Partial, not full PHP array compatibility.

### COMPAT-054 — Consolidated PHP array differential matrix

- **Test:** `phix_rust/tests/array_differential_matrix.sh` compiles a PHP website with Phix, executes the same source under PHP CLI, and compares exact HTTP response bytes to PHP output.
- **Verified:** 14 labeled cases covering equality, ordering, numeric strings, plain copies, reference-containing copies, and detached nested aliases. On this milestone all 14 matched PHP; `cargo test --offline` passed 102/102.
- **Scope limitation:** This is a small curated matrix, not a representative measurement of PHP 8.x array compatibility. It does not cover warning/error formatting, recursion exceptions, full reference rebinding, key coercion edge cases, or the official PHP test corpus.
- **Status:** Differential infrastructure added; full array compatibility not established.

### COMPAT-055 — Expanded PHP array differential matrix (verification only)

- Expanded `phix_rust/tests/array_differential_matrix.sh` from 14 to 28 PHP CLI versus compiled-Phix HTTP output cases.
- Added negative-index append history, key coercion, duplicate keys, nested copying, detached aliases, reference rebinding, insertion-order equality, and numeric precision.
- **Verified:** 28/28 cases matched PHP CLI in this environment; `cargo test --offline` passed 102/102.
- **Not resolved:** this matrix does not establish full PHP array compatibility. Cyclic references, catchable PHP errors, warning parity, broader key conversion edge cases, and full PHP conformance remain open.
- **Status:** Expanded regression coverage; no new runtime feature claimed.

### COMPAT-056 — Array key boundary differential coverage

- Added PHP CLI vs compiled Phix HTTP differential cases for numeric-string key boundaries (`"0"`, `"00"`, `"-0"`, `"+1"`, leading/trailing spaces), signed 64-bit key limits, and duplicate key coercion.
- Added two Rust unit tests for numeric-string key normalization and replacement/insertion-order behavior.
- **Verified:** 104/104 Rust unit tests and 32/32 PHP differential output lines matched on the local test environment; compiler build passed.
- **Remaining:** Full PHP array compatibility is not established by these cases. Reference cycles, PHP error/exception semantics, other key coercion edge cases, and broader PHP 8.x conformance remain open.

### COMPAT-057 — PHP array union (`+`) (partially resolved)

- **Implemented and tested:** The `+` operator between two PHP array values now produces an ordered union: left-hand entries win on duplicate keys, right-only keys are appended, and numeric append history is updated. Ordinary entries copy by value while explicitly referenced entries retain their reference cells. Three Rust unit tests and four compiled-website PHP CLI differential cases pass. The existing 32-case array differential matrix also passes.
- **Remaining differences:** Non-array operand errors currently are not PHP-compatible (the generated fallback returns `null`); additional expression typing and PHP error propagation are needed. Array union with cycles, deeply nested reference graphs, and compound `+=` have not been verified. This milestone does not establish complete PHP array compatibility.

### COMPAT-058 — Array union assignment (`+=`)

- **PHP behavior:** `$left += $right` retains all left array entries, adds only missing keys from the right, and updates the left variable binding (including its aliases). Embedded references remain shared where PHP requires them.
- **Phix implementation:** Lexer recognizes `+=`; compiler lowers array union assignment to `PhpArray::union` followed by `PhpSlot::replace`. Runtime non-array operands report an HTTP request error rather than silently replacing the value. The `array_union_assignment_parity.sh` differential test covers four cases.
- **Remaining differences:** Full PHP exception objects, precise PHP error wording/locations, and all non-array operand behavior are not yet compatible. The operator is currently supported for variables holding PHP arrays, not every possible PHP lvalue. Full PHP array compatibility is not established.
- **Status:** Implemented for tested array-variable cases; partial PHP compatibility.

### COMPAT-059 — Nested array union assignment (`+=`)
- **Implemented and tested:** The compiler accepts `+=` on keyed and nested array-element targets, resolves the target via `PhpSlot::bind_element`, and writes the PHP array union through the reference cell. This preserves alias updates for the tested cases.
- **Differential verification:** `tests/array_nested_union_assignment_parity.sh` compares three PHP CLI outputs against compiled HTTP output. The existing union and variable `+=` differential suites also pass (four cases each). Rust unit tests: 109 passed.
- **Not complete:** Invalid operand errors are not yet PHP-compatible catchable `TypeError` objects. Append-form `[] +=` is intentionally rejected. Full array/reference compatibility remains unverified.

### COMPAT-060 — Invalid array-union operands (partial)
- Binary array union (`+`) now accepts scalar operands at compile time and detects unsupported operand combinations in the generated runtime instead of returning `null`.
- Array `+=` on variable and nested-element targets now uses the same operand validation and preserves the left operand on failure.
- The request-local error path returns HTTP 500 with a PHP-like operand-type message. **Not PHP-equivalent:** PHP throws a catchable `TypeError`; Phix currently reports an uncatchable fatal HTTP error, and some scalar-left `+=` targets remain compile-time restricted.
- Verified with Rust unit tests, the existing PHP array-union differential suite, and a new compiled HTTP invalid-operand test.

### COMPAT-061 — Typed fatal array errors (partial)
- **Implemented:** Runtime request errors now carry an explicit PHP error class and message. Invalid `+` / `+=` array operands surface as `TypeError`; recursive comparison errors remain `Error`. This corrects the previous misleading `Uncaught Error` label for invalid array operands.
- **Verified:** The invalid-array-union HTTP regression now checks the `TypeError` class, and the recursive-comparison HTTP regression continues checking `Error`. Rust unit tests remain passing.
- **Still missing:** These are fatal HTTP responses, **not catchable PHP exception objects**. `try`/`catch`, precise PHP stack traces and source locations, and immediate exception-style interruption of execution are not implemented. Do not count this as complete PHP exception compatibility.

### COMPAT-062 — Scalar-left `+=` with array operands (partial)
- **PHP behavior:** `$value += $array` with an integer, boolean, or string left operand throws a `TypeError` with `Unsupported operand types: <left type> + array` (PHP CLI verified for integer).
- **Implemented:** Compiler now promotes a scalar left operand to a PHP slot for runtime operand validation instead of rejecting these expressions at compile time. Ordinary integer `+=` integer retains arithmetic behavior. The runtime reports a typed HTTP 500 error without partial response output.
- **Verified:** 110 Rust unit tests; 3 compiled-website HTTP regression cases (int, bool, string); 32-case PHP array matrix; 4-case `+`, 4-case variable `+=`, 3-case nested `+=` regressions; invalid operand HTTP regression.
- **Still missing:** Catchable `TypeError` objects, PHP `try`/`catch`, precise source positions and stack traces, and full scalar `+=` coercion/arithmetic rules. These tests are not a full PHP 8.x conformance suite.

### COMPAT-063 — Stop executing statements after recorded fatal array errors

The generated request handler and nested statement blocks now check the request-local
runtime error slot after each statement. An invalid array union or recursive array
comparison terminates execution at the next statement boundary instead of continuing
with subsequent statements. This is a partial improvement toward PHP exception
unwinding, not an implementation of PHP `try`/`catch` or throwable objects.

Verification: 112 Rust unit tests passed (including two compiler guard regressions),
and the four-case compiled HTTP array-union PHP differential test passed.
The guard does not interrupt evaluation *inside* an expression or an individual
statement; catchable `TypeError`/`Error` remain unsupported.

### COMPAT-064 — Fatal array errors inside multi-expression `echo`
- **Implemented and tested:** each comma-separated `echo` argument is checked for a pending fatal PHP runtime error before its output is appended. An invalid array-union operand in the middle of `echo` now interrupts that statement, preventing subsequent arguments from being evaluated or echoed.
- **Verified:** two Rust compiler tests and a compiled HTTP regression checking status 500, `TypeError` labeling, no partial output, and no Rust panic.
- **Still missing:** catchable PHP `try`/`catch`, `Throwable` objects, and full PHP error propagation within all expression forms. This milestone does not establish complete array compatibility.

### COMPAT-065 — Fatal array comparison guards in conditional expressions
- Implemented: generated `if` and `while` statements check request-local runtime errors immediately after evaluating their conditions and before entering their bodies. The check is repeated on each `while` iteration.
- Verified: 116 Rust unit tests pass (including two compiler regression tests); existing compiled array-union differential suite 4/4 matches PHP.
- Limitations: PHP `try`/`catch`, catchable `Error`/`TypeError`, and comprehensive runtime differential tests of recursive condition errors remain unimplemented.

### COMPAT-066 — Initial typed catch/finally execution
- Implemented compiler parsing and generated execution for `try` with typed `catch` and optional `finally` in void-returning contexts.
- Added typed pending-error matching, consumption, and restoration; exposed a temporary class/message representation to handlers.
- Verified 119 Rust tests and a compiled HTTP request catching array-union `TypeError`, executing `finally`, and returning HTTP 200.
- Deviations: catch binding is not a PHP Throwable object; no `throw`, custom classes, exception methods, complete unwinding or all control-flow semantics. Exception compatibility remains incomplete.

### COMPAT-068 — Throw statement and typed rethrow foundation
- **Implemented:** `throw new Exception("message");` and analogous `Error`, `TypeError`, `ValueError`, `ParseError` statements in void-returning contexts; `throw $caught;` from the current temporary catch representation. Nested catch/rethrow and `finally` use request-local error propagation.
- **Verified:** 121 Rust tests, compiler build, and `tests/exception_throw_http.sh` compiled HTTP regression returning `first:nested:finally`.
- **Deviations:** No genuine PHP Throwable instances, arbitrary throw expressions, custom classes, complete constructor signatures, stack traces, function unwinding, or PHP-exact diagnostics. The `throw $caught` implementation only accepts the internal class/message object shape, which is not PHP's object model. Full PHP exception compatibility remains incomplete.

### COMPAT-069 — Basic caught-exception method syntax
- **Implemented:** Lexer `->` token and compiler lowering of `$e->getMessage()`, `$e->getCode()`, and `$e->getPrevious()` for JSON-backed caught exception values. Runtime reads the existing temporary exception representation. Default code is `0`; previous is `null`.
- **Verified:** 123 Rust unit tests and `tests/exception_methods_http.sh` compiled HTTP differential against PHP CLI; prior `tests/exception_throw_http.sh` also passes.
- **Deviations:** Catch values remain internal JSON-like objects, not real PHP Throwable objects. Constructors still accept only a message; custom codes, previous exceptions, stack traces, `getFile()`, `getLine()`, `getTrace()`, and `getTraceAsString()` are not implemented. Method syntax is intentionally limited and cannot be considered general object method dispatch.

### COMPAT-070 — Built-in exception constructor code and rethrow preservation
- **Implemented:** Optional integer `code` argument on built-in thrown exception constructors (`Exception`, `Error`, `TypeError`, `ValueError`, `ParseError`). The request-local error and temporary catch representation preserve that code through typed catches and rethrows; `getCode()` returns it. Default code remains `0`.
- **Verified:** 123 Rust unit tests, compiled `exception_methods_http.sh`, and new compiled `exception_constructor_code_http.sh` PHP differential covering nonzero codes and rethrow.
- **Deviations:** No actual Throwable object identity, third-argument previous exception, custom subclasses, general throw expressions, stack traces, or full PHP constructor type validation. Full exception compatibility is incomplete.

### COMPAT-071 — Previous-exception constructor chaining
- **Implemented:** The third argument of supported built-in `throw new Exception/Error/TypeError/ValueError/ParseError` constructors accepts an existing internal Throwable representation or `null`. Chained errors retain their class, message, code, and previous link through catch and rethrow. `getPrevious()` returns the stored chain, allowing nested `getPrevious()->getMessage()` and `getPrevious()->getCode()` calls.
- **Verified:** 123 Rust unit tests and compiled `exception_previous_http.sh` differential against PHP; existing exception constructor, method, throw/rethrow, and catch/finally HTTP regressions passed.
- **Deviations:** Throwable values are still internal JSON-like representations, not PHP objects; object identity, general constructor expressions, custom exception classes, stack traces, full error type validation, and complete control-flow unwinding remain incomplete.

### COMPAT-072 — Zero-argument built-in exception constructors
- **Implemented:** `throw new Exception()`, `throw new Error()`, and other already-supported built-in Throwable classes now accept an omitted message and default to an empty string, code `0`, and no previous exception. Previously the compiler rejected an empty constructor argument list.
- **Verified:** 123 Rust unit tests, `tests/exception_default_constructor_http.sh` compiled PHP CLI differential (Exception, Error, TypeError, catch hierarchy), and existing `tests/exception_previous_http.sh` chaining differential.
- **Deviations:** Exception values are still internal JSON-like representations, not real PHP objects. Custom exception classes, complete stack traces, throw expressions, and general function unwinding remain incomplete.

### COMPAT-073 — Union-type catch and optional catch variable
- **Implemented:** PHP 8-style `catch (Exception|TypeError $e)` and `catch (Exception)` without a bound variable. Multiple catch clauses retain first-match ordering; union alternatives use the existing built-in Throwable hierarchy.
- **Verified:** 123 Rust unit tests and `tests/exception_union_catch_http.sh`, comparing a compiled HTTP response with PHP CLI for union matches, omitted binding, parent-class matches, and ordered catches.
- **Deviations:** Only the existing built-in exception class names are accepted; user-defined Throwable subclasses, genuine PHP objects, complete function unwinding, and complete finally semantics remain unsupported.

### COMPAT-075 — Throwable object identity (open; regression guard added)

The current exception representation is still `Json::Object` with `class`, `message`,
`code`, and `previous` keys. This is not a PHP object and does not preserve
native identity or guarantee that a thrown value is an actual `Throwable`.
The new `phix_rust/tests/exception_object_identity_regression.sh` is an
**expected-failing structural guard**. Do not count it as a passing test or
as implemented compatibility. Completion requires a dedicated runtime object
variant, identity-preserving catch/rethrow/previous, correct inheritance,
and PHP differential tests. Existing COMPAT-074 claims are unchanged.

### COMPAT-075 — Native Throwable identity (2026-11-19)

Implemented and tested: generated binaries now use `Json::Throwable(Arc<PhixRuntimeError>)` instead of JSON field maps for caught exceptions. Cloning and rethrowing preserve object identity; `getPrevious()` returns the same underlying Throwable object; a forged PHP array with `class` and `message` fields cannot be thrown as an exception. The existing throw-expression differential and the new `tests/exception_native_identity_http.sh` differential pass against PHP. The three previously failing static regression guards now pass.

**Not yet compatible:** user-defined exception classes, complete Throwable methods, stack traces and source locations, exception unwinding through value-returning/typed functions, and all `finally` control-flow interactions. This milestone does not establish full PHP 8.x exception compatibility. The previous array compatibility estimate remains provisional.

### COMPAT-076 — Typed function throw propagation (partial, 2026-11-19)
- Compiled `throw new Exception`, `TypeError`, and `ValueError` statements now work in `int`, `bool`, and `string` returning functions, using the request-local exception channel to unwind and allowing outer `try/catch` to handle the exception.
- Added `exception_typed_function_http.sh`, a differential comparison with PHP for three typed return kinds; it passed.
- Existing throw-expression and native Throwable identity HTTP differentials passed; Rust unit tests: 126/126.
- **Not fully compatible:** `try/catch/finally` inside value-returning functions remains unsupported; return/exception unwinding still uses a default-value sentinel and request-local error state rather than a fully structured control-flow result. Missing-return diagnostics and nested expressions need further parity work. Custom classes, stack traces, and complete `finally` semantics remain unfinished.

### COMPAT-077 — typed try/catch/finally (partial; 2026-11-19)

Implemented typed-function `try/catch/finally` for handlers that complete normally and return after the construct. Verified `exception_typed_try_catch_http.sh` against PHP, previous typed throw and throw-expression differentials, and 128 Rust unit tests. Returns from within these constructs and full structured unwinding remain unsupported. Array compatibility estimate remains ~78% provisional and unchanged.

### COMPAT-079 — Structured control-flow foundation (in progress)

The compiler now contains `src/compiler/flow.rs`, defining explicit `Normal`,
`Return`, `Break`, `Continue`, and `Throw` outcomes, including PHP-compatible
`finally` replacement precedence. Six isolated Rust unit tests pass. **This
representation is not yet integrated into generated PHP functions or blocks.**
The existing `Default::default()` exception sentinels and closure-based
`try/catch/finally` remain in code generation; do not claim structured
unwinding or PHP exception compatibility is complete. The next step is to
migrate code generation and run differential PHP tests.

### COMPAT-079 continuation — typed function fallthrough (2026-11-19)

Implemented: generated typed functions no longer append an unconditional `Default::default()` return. A conservative terminal-return check rejects function bodies without a final top-level `return` statement. This deliberately rejects some otherwise valid PHP programs (including throw-only functions) until structured unwinding is integrated. This is **not** complete PHP control-flow analysis. The existing exception-propagation sentinels in `parser.rs` are still present and must be removed through a genuine `PhpFlow` code-generation migration. Rust unit suite: 140 passed. Full HTTP differential suite: not run.

### COMPAT-079 follow-up — terminal-return control-flow validation (2026-11-19)

The typed-function terminal-return check now tracks block and delimiter nesting instead of inferring an unconditional return from a nested `return` token. Conditional-only and loop-only returns are not accepted as proof that a function returns on every path. A final unconditional `return` after a conditional is accepted. This is a conservative compiler validation improvement, **not** PHP-compatible exhaustive control-flow analysis. It may reject valid PHP programs. Typed exception propagation still uses default-value sentinels; `PhpFlow` is not integrated into generated execution. Do not count this milestone as complete structured unwinding.

### COMPAT-079 follow-up: void return through try/catch/finally
- Fixed compiler acceptance of void-function `return` within try/catch/finally closures that would silently return only from a generated closure.
- Now rejected at compile time pending true `PhpFlow`-based lowering. **Not PHP-compatible yet.**
- Verification: 149/149 Rust tests passed (including 3 new regression cases); full PHP differential suite not run.

### COMPAT-079 loop-control diagnostic guard (2026-11-19)
- `break` and `continue`: **not implemented**; explicit compile-time diagnostic now, with regression tests.
- No claim of additional PHP compatibility. Existing closure-based `try/catch/finally` and default-value exception sentinels still need replacement with structured unwinding.
- Rust tests: 151 passed (full PHP differential suite not run).

### COMPAT-079 — explicit flow sequencing and catch semantics (2026-11-19)
- Added `PhpFlow::and_then` for sequencing only normal execution, and `PhpFlow::catch_throw` for handling only thrown transfers. Five new Rust tests cover propagation, catch bypass, and `finally` precedence.
- Verified: 156/156 Rust tests pass using Rust 1.100.0.
- **Not integrated into generated PHP execution yet.** The parser still emits closure-based try/catch/finally and default-value exception sentinels. This milestone is a foundation only, not full PHP compatibility. Do not count this as an implemented language feature.

### COMPAT-080 (partial): compiler control-flow IR

Implemented an explicit control-flow graph with reachable-fallthrough analysis,
used by typed-function return validation. Verified 162 Rust unit tests, including
six new graph tests. **Not yet implemented:** full typed IR, expression lowering,
structured exception edges, `finally` transfers, or removal of old sentinel paths.
PHP differential tests were not run for this change. No compatibility percentage
increase is claimed.

### COMPAT-080 continuation: IR structural validation (2026-11-19)

- Implemented: CFG entry and successor validation in the compiler analysis path; five new Rust tests.
- Verified: 167/167 Rust unit tests (`cargo test --offline`).
- Not verified: PHP differential suite, production behavior, complete IR lowering.
- Not implemented: removal of exception sentinels, native `finally` unwinding, full PHP 8.x parity.
- Compatibility percentage unchanged; IR validation is a correctness prerequisite, not new PHP syntax support.

### COMPAT-080 conditional CFG follow-up (2026-11-19)
- Implemented: conservative nested braced `if/else` control-flow graph analysis,
  and parser generation for braced `else` branches.
- Verified: 174 Rust unit tests, including new IR and codegen tests.
- Not verified: full PHP differential suite or generated native executable
  equivalence for these new branches.
- Still incomplete: `elseif`, unbraced conditionals, `finally` unwinding,
  expression-level exception edges, and removal of default-value sentinels.

### COMPAT-080: throw terminators in compiler CFG (2026-11-19)

- Implemented: standalone `throw` statements are explicit terminal IR edges for conservative return-path analysis; tested unconditional, conditional, mixed return/throw branches, and loop-contained throws.
- Verified: Rust unit suite 178/178 passed.
- Not verified: generated application build and PHP differential behavior for this change.
- Still missing: expression-level IR, structured exception propagation in generated code, `finally` control-flow semantics, removal of default-value exception sentinels.

### COMPAT-080: typed integer comparison IR (2026-11-19)

Native IR now lowers integer-to-integer equality, strict equality, inequality, and ordering comparisons into explicit typed expression nodes. Both operands are bound once in left-to-right order before native Rust comparison. This is integrated into the expression compiler, not a standalone helper. Other comparison categories still use the existing compiler paths; this does not establish PHP numeric or general comparison parity. The remaining exception sentinel unwinding and full expression IR migration are outstanding.

### COMPAT-080 — Boolean logical IR

Implemented for already-typed boolean operands: `&&` and `||` lower through native typed IR with short-circuit RHS evaluation. Two new Rust IR tests pass; complete PHP differential verification has not been run. General PHP truthiness and structured exception propagation remain incomplete. No change to provisional array compatibility estimate.

### COMPAT-080 follow-up — conditional-chain control-flow lowering
- The function CFG now analyzes braced `if / else if / elseif / else` chains, including implicit fallthrough when a final `else` is absent.
- The compiler's executable code generator accepts braced `else if` chains using nested conditional lowering; regression tests verify accepted and rejected typed return paths.
- `elseif` is recognized by CFG analysis but is **not yet supported by executable code generation**. This distinction is intentional; do not count CFG-only analysis as PHP syntax compatibility.
- This milestone does not integrate exception edges, remove default-return sentinels, or establish full PHP compatibility.

### COMPAT-080 — executable `elseif` chains (2026-11-19)

- The parser now lowers braced PHP `elseif (...) { ... }` chains through the same conditional execution path as `if` / `else if` / `else`.
- Mixed `else if` and `elseif` spellings are accepted for the tested braced form.
- Typed-function control-flow validation continues to reject conditional chains without guaranteed exits.
- Verification: 196 Rust unit tests passed (3 new tests). Full compiled HTTP differential suite not run.
- This does **not** complete native structured exception unwinding or PHP conditional syntax generally (e.g. alternative colon syntax).

### COMPAT-080 — Logical truthiness IR extension (2026-11-19)

**Implemented and Rust-unit-tested:** `&&` / `||` with PHP truthiness conversion for known `int`, `bool`, `string` and dynamic `Json` values; lazy RHS evaluation retained by native Rust conditional operators. **Not fully verified:** compiled HTTP differential against PHP, `and`/`or`/`xor`, and full PHP expression semantics. Structured exception propagation remains incomplete; no compatibility percentage increase is claimed.

### COMPAT-080 — Conditional/unary truthiness (2026-11-19)

- Implemented: `if` condition and unary `!` compilation now use the shared PHP truthiness lowering for supported booleans, integers, strings, and dynamic values.
- Verified: 204 Rust unit tests passed, including four new code-generation regression tests.
- Not verified: full PHP differential suite, all coercion cases, and structured exception unwinding. No overall compatibility percentage change.

### COMPAT-080: Exceptional control-flow edges (2026-11-19)

Implemented and unit-tested in analysis IR: `Invoke { normal, unwind }`, target validation, fallthrough reachability and cyclic exceptional edges (4 new Rust tests; 208 total passing). **Pending:** lowering actual calls/expressions into invoke terminators, typed exception payloads, `finally` unwinding, removal of `Default::default()` exception sentinels, and PHP differential verification. No PHP compatibility percentage increase is justified.

### COMPAT-080 call-effect IR (partial)
- Implemented: conservative `Invoke` normal/unwind edges for call-bearing statements and returned calls in the function-body analysis IR.
- Verified: 213 Rust tests, including three new IR regression tests.
- Missing: executable exception-edge lowering, structured `finally` unwinding, removal of default-value propagation sentinels, PHP differential verification.

### COMPAT-080 conditional-call effects (2026-11-19)
- Implemented: explicit normal/unwind CFG edges for detected function calls in `if` and `elseif` conditions; three regression tests.
- Verified: 216 Rust unit tests passed.
- Not implemented: executable IR-based exception propagation, sentinel removal, and `finally` control transfers. No PHP differential test was run for this change.

### COMPAT-080 — Nested throw-expression CFG effects
- Implemented: conditional and return expression IR analysis recognizes nested `throw` as an exceptional effect.
- Verified: two new Rust regression tests; 218 total unit tests pass.
- Not implemented: generated-code structured exception unwinding, removal of default-value sentinels, full PHP differential validation.

### COMPAT-080 generated typed-function boundary regression (2026-11-19)

- Fixed generated Rust typed-function block termination (Rust E0317 in compiled HTTP differential cases).
- Verified targeted PHP differential scripts: `exception_typed_function_http.sh`, `exception_throw_expression_http.sh`, `exception_native_identity_http.sh`.
- Added Rust code-generation regression test. Native structured exception unwinding and elimination of default-value sentinels remain **incomplete**. No PHP compatibility percentage increase is claimed.

### COMPAT-081 — exit/die (foundation only)
- Implemented: distinct native `Exit` control-flow outcome, with status/message payload, non-catchability, no `finally` execution, and propagation across loops/sequence.
- Verified: four new Rust unit tests; 223 total Rust unit tests passed.
- Not implemented: PHP parser/code generation for `exit`/`die`, PHP-version-specific argument coercion, shutdown functions, destructors, output buffers, and script-boundary handling. No PHP differential test was run. PHP `exit`/`die` compatibility remains **incomplete**.

### COMPAT-081 typed-function `echo` and `while` error guards (2026-11-19)
- Fixed: non-void generated Rust functions no longer emit an invalid bare `return;` when an error is detected during `echo` expression evaluation or `while` condition evaluation.
- Verified: 226/226 Rust unit tests, including three targeted code-generation tests.
- Not verified: executable PHP differential tests for these paths.
- Still incomplete: structured error results, sentinel removal, full PHP 8.x semantics. No compatibility percentage change is claimed.

### COMPAT-082 — Ordered evaluation of typed function arguments (2026-11-19)

Implemented compiler-side nested evaluation for calls with multiple arguments. Each argument is evaluated in source order and a pending PHP runtime exception prevents evaluation of subsequent arguments and prevents invoking the callee. Both typed-return and void-return callees use this lowering. Two Rust regression tests were added, and a generated Rust executable containing both forms was successfully compiled. This is a narrow correctness improvement, **not** complete exception propagation: typed calls still use a default-value placeholder after an exception, and structured `Result`/`PhpFlow` unwinding and PHP `finally` return semantics remain incomplete. No complete PHP differential suite was run for this change.

### COMPAT-083 — Guarded function invocation (partial)

User-defined function calls now use consistent exception guards for zero, one, and multiple arguments, with source-order argument evaluation. Three additional compiler regression tests passed (231 Rust tests total). PHP differential testing was not performed for this milestone. Explicit structured exception outcomes, removal of `Default::default()` exception placeholders, and PHP-compatible `finally` precedence are still pending; do not consider full PHP 8.x compatibility achieved.

### COMPAT-084 — Scalar assignment RHS evaluation

Native code generation now checks for pending exceptions between evaluating the right-hand side and committing scalar variable initialization, reassignment, or integer `+=`. Verified with Rust code-generation regression tests (234 passing overall). Full PHP differential validation and explicit structured exception propagation remain outstanding.

### COMPAT-085 — Logical expression exception ordering

Implemented: compiler-generated boolean `&&` and `||` expressions check pending exceptions after the left operand and suppress right-hand evaluation on an exception. Two new regression tests pass, with 236 Rust tests passing in total. Not yet verified with PHP differential execution; full PHP 8.x compatibility and structured exception outcomes remain incomplete.

- COMPAT-086: Guarded binary evaluation for concatenation, array union and
  dynamic comparisons; unit-tested. Explicit exception-result lowering and
  comprehensive PHP differential tests are still outstanding.

### COMPAT-088 — Array index read exception ordering (partial)
- Implemented: generated array reads evaluate the base before the key, guard after each, and skip indexing or subsequent nested keys when an exception is pending.
- Verified: two new Rust code-generation regression tests; 242/242 Rust unit tests passed. Existing associative-array code-generation assertion updated for the guarded structure.
- Not verified: PHP differential tests for this change. Structured error outcomes, removal of `Default::default()` sentinels, and full PHP 8.x compatibility remain incomplete.

### COMPAT-089 — Guarded array element writes (partial)
- Implemented: array element assignment evaluates each index key in source order, checks pending exceptions after every key and after the right-hand side, and commits only if evaluation succeeds. Append writes and element `+=` are also guarded.
- Verified: 244/244 Rust unit tests, including two new code-generation regressions.
- Not verified: PHP differential execution for this milestone. Explicit exception outcomes, `finally` precedence, and full PHP 8.x compatibility remain incomplete.

### COMPAT-090: exception-ordered unset dimensions

The compiler now evaluates nested `unset($array[key1()][key2()])` keys sequentially, checking pending exceptions before evaluating later keys or mutating the array. Multiple `unset` operands retain source order. Rust regression tests verify generated guards. Full PHP differential verification and structured exception-result lowering remain pending.

### COMPAT-091 — Exception constructor argument ordering (partial)

- Implemented: `throw new Exception(...)` and supported built-in exception subclasses evaluate message, code, and previous arguments in source order, checking pending exceptions before later arguments and before constructing the outer exception. This applies to both throw statements and throw expressions.
- Verified: 248/248 Rust unit tests, including two new code-generation regression tests.
- Not verified: compiled PHP differential execution for this milestone. Full structured exception outcomes, `finally` precedence, and PHP 8.x compatibility remain incomplete.

### COMPAT-092 — Preserve exceptions from throw operands (partial)

- Implemented: `throw $expression` in statement and expression contexts evaluates the operand once, then invokes `phix_throw_value` only if operand evaluation did not already raise a pending exception. This prevents replacing the original exception with a secondary error when the operand throws.
- Verified: 250/250 Rust tests, including two generated-code regression tests.
- Not verified: compiled PHP differential execution for this milestone. Structured exception outcomes, complete `finally` precedence, and full PHP 8.x compatibility remain incomplete.

### COMPAT-094 structured finally semantics (2026-11-19)
- Implemented/tested in Rust control-flow model: lazy `with_finally` execution;
  `exit` bypasses finally; normal finally preserves a pending return;
  a return from finally replaces a pending exception.
- Verification: 255/255 Rust unit tests passed (3 new tests).
- **Not yet PHP-compatible executable behavior**: the parser/code generator
  still emits closure-based try/catch/finally and pending-error sentinels;
  no compiled PHP differential tests were run for this change.

### COMPAT-095: Typed functions that only throw

Implemented: native code generation emits a verified terminal boundary for non-void typed functions, allowing `function fail(): int { throw new Exception('failed'); }` to compile. Rust unit tests: 258 passed, including three new terminal-boundary tests. A generated native binary with this function compiled. Limitations: pending-error propagation still uses default-value sentinels; no complete PHP differential suite was executed, and return/throw/finally structured lowering is incomplete.

### COMPAT-096: Explicit function outcome conversion (partial)

- Added `PhpFlow::into_function_result` to distinguish `Return(T)` and `Throw(E)` as `Result<T, E>` at a completed function boundary, without a fabricated default value.
- Rejects fallthrough, unresolved loop transfers, and request-level exit rather than silently converting them into normal function returns.
- Verified: 260/260 Rust unit tests passed, including two new tests.
- **Not yet integrated into generated functions**: parser/codegen still uses request-local pending exceptions and `Default::default()` guards. No compiled PHP differential tests for this change; PHP 8.x compatibility remains incomplete.

### COMPAT-097 — Composed structured try/catch/finally (internal only)

`PhpFlow::try_catch_finally` now composes catch and lazy finally transfers
in PHP execution order. Catch handles only exceptions from the try body;
finally can override a return or exception, and exit/die bypasses both
callbacks. Three Rust regression tests cover the composition. **Not yet
connected to emitted PHP function code**: generated Rust still uses pending
exception state and default-value sentinels. This does not establish PHP
8.x compatibility; executable lowering and PHP differential tests remain.

### COMPAT-098 — Dynamic array-union assignment exception guard

Generated `+=` code for dynamically handled array union now checks for a pending
exception immediately after evaluating its right-hand side and before reading
or modifying the left-hand side. A Rust regression test verifies the generated
ordering; 264 Rust unit tests pass. This retains the existing pending-exception
and default-value propagation architecture, which must still be replaced with
explicit executable outcomes. No PHP differential suite was run for this change.

### COMPAT-099 — Built-in SPL exception subclass matching
- **Status:** Partial (verified subset)
- **PHP:** Catch clauses match subclasses through the exception inheritance hierarchy; `Error` is not an `Exception`.
- **Phix:** Built-in SPL exception classes listed in `documentation.md` can be thrown and matched through their built-in parent classes. User-defined exception classes, other Throwable implementations, and complete control-flow unwinding remain unsupported.
- **Verification:** `cargo test --offline` (266 passing); `phix_rust/tests/spl_exception_inheritance_parity.sh` matches installed PHP output for five compiled HTTP cases.

### COMPAT-100 — Exception constructor shorthand
- **Status:** Verified subset.
- `throw new RuntimeException;` and `throw new Exception()` are accepted both as statements and inside throw expressions, with built-in exception class restrictions unchanged.
- Three new Rust regression tests pass (269 total). `phix_rust/tests/exception_constructor_shorthand_parity.sh` compares a compiled HTTP application's response with PHP for statement shorthand, expression shorthand, and parenthesized constructor arguments.
- This does not implement user-defined exception subclasses, structured exception propagation, or return-in-finally semantics.

### COMPAT-101 — null coalescing (partial, 2026-11-19)
- Implemented lexer/parser/code-generation support for `??` when the left operand is a defined dynamic (`Json`) value. The right operand is evaluated lazily only when the left value is `null`; chained operators parse right-associatively.
- The left operand is evaluated exactly once. Pending exceptions are not converted into successful fallback evaluation.
- **Not complete PHP `??` compatibility:** undefined-variable and undefined-offset suppression, property access, general mixed static/dynamic type inference, and `??=` are not implemented. Generated-code regression tests passed; no PHP differential run for this milestone.
- This is not the planned migration to explicit `Result`-based function outcomes; existing exception sentinels remain.

### COMPAT-102 — Null coalescing for undefined simple variables

Implemented: `$missing ?? $fallback` treats an undefined simple left-hand variable as null; ordinary undefined reads remain errors. Verified by two Rust regression tests (274 total passed). Not implemented: missing offsets, properties, null-coalescing assignment (`??=`), and comprehensive PHP differential coverage. Overall PHP 8.x compatibility remains incomplete.

### COMPAT-103 — Ternary expressions (partial)

Implemented parser/codegen for `a ? b : c` and `a ?: c`, lazy branch selection, single evaluation of the condition, PHP truthiness for supported types, and parenthesized nesting. Ambiguous unparenthesized nested ternaries are rejected. Verified by 3 new compiler regression tests (277 Rust tests passed). Not yet verified by compiled PHP differential tests. Full PHP 8.x ternary precedence, type conversion, and exception-unwinding compatibility are not claimed.

### COMPAT-104 — `??=` for simple variables (partial)

Implemented lexer/parser/code generation for simple-variable null-coalescing assignment, including undefined variables and existing null/non-null values. The fallback is lazy and commits only on successful evaluation. Verified with 2 new Rust tests (279 passing). Not yet supported: array offsets, object properties, complete PHP precedence and differential validation. Structured exception results remain pending; full compatibility is not claimed.

### COMPAT-105 — Array-offset `??=` (partial)

Implemented compiler generation for existing dynamic-array offset targets, including nested keys. Reads precede lazy RHS evaluation, and writes occur only after successful RHS evaluation. Added typed-scalar `??` left operands. Three Rust regression tests added (282 passing). No comprehensive PHP differential run. Property targets, all reference/offset edge cases, and structured exception unwinding remain incomplete.

### COMPAT-109 — typed function exception outcomes (partial)

Generated non-void function statement guards now propagate explicit `Err` outcomes, and successful returns use `Ok`. Expression-level `Default::default()` placeholders and legacy call adapters remain. The entire PHP 8.x exception model is not yet compatible; differential tests remain necessary.

### COMPAT-110 audit: explicit exception outcomes (in progress)

The COMPAT-109 generated typed-function boundaries use `Result<T, Arc<PhixRuntimeError>>`, but expression generation still contains `Default::default()` exception sentinels for function calls, ordered binary expressions, and ternaries. These cannot be safely replaced with `return Err(...)` without context-aware expression lowering: top-level request handlers return `()`, and `try`/`catch`/`finally` currently use closures. Direct returns in these closures would have different control-flow semantics. The catch-class matcher was deduplicated without changing its accepted classes. Rust unit tests: 291 passed. Executable structured expression unwinding and comprehensive PHP differential testing remain pending.

### COMPAT-111 — structured outcome forwarding (partial)
Implemented: a direct return of a zero-argument typed user function with matching return type forwards `Result` directly. This is an architectural migration step, not a claim of new PHP language compatibility. Pending: general expression and argument lowering, void function outcomes, try/catch/finally semantics, and comprehensive differential tests.

### COMPAT-112 — argument-bearing direct Result forwarding (partial)

Implemented: direct `return callee(args...)` forwarding for matching non-void return types, with sequential argument evaluation and exceptional short-circuiting. Verified by Rust code-generation regression tests. Not yet verified against PHP differential execution. Remaining: replace expression-level pending-exception adapters and default-value placeholders, migrate void functions, and implement full `try`/`catch`/`finally` control flow.

### COMPAT-113 — structured expression outcomes (internal, not executable PHP lowering)

Implemented and tested Rust `Result` expression combinators for binary operations, conditionals and coalescing (298 Rust tests passing). Generated PHP expressions still use legacy pending-error/default-value adapters. No PHP differential tests were run; no new PHP compatibility claim is made.

- COMPAT-114: Direct typed return calls now propagate errors from zero-argument
  typed calls used as complete arguments using Rust `?` (unit-tested). General
  expression-level structured propagation, `void` outcomes and full `finally`
  unwinding remain incomplete; no new broad PHP compatibility claim.

### COMPAT-115 — Structural expression lowering (partial)

The `??` emission path now uses a structural typed-IR node; lazy ternary and coalescing nodes have IR regression coverage. No new PHP language feature is claimed. The general AST-to-typed-IR migration, explicit exception outcomes in all expression contexts, and PHP differential validation remain incomplete.

### COMPAT-116 status
Implemented and Rust-unit-tested compositional `Result`-based IR emission for
nested arithmetic, comparisons, boolean short-circuiting, ternaries and
null-coalescing. **Not yet wired to the PHP parser's executable expression
lowering**. The old sentinel-based paths remain; no additional PHP language
feature is claimed compatible on the basis of this internal change.
