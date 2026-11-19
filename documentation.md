# Phix documentation

This document describes **implemented behavior in the current Rust source tree**. Phix is an experimental compiled PHP-like language and web runtime, **not a complete PHP implementation**. Keep this reference updated alongside every new feature or function; distinguish supported behavior from planned work.

## 1. Purpose and architecture

Phix aims to compile PHP-like website projects into native, standalone executables while retaining familiar PHP syntax, URLs, and project structure. The compiler is written in Rust; it generates Rust source and invokes `rustc` to produce the website executable. The generated binary embeds public assets and route handlers and runs a built-in HTTP server. Writable upload storage and runtime configuration remain outside the binary.

The current compiler is an early subset with limited type and expression support. Existing PHP frameworks do **not** run unchanged.

## 2. Prerequisites and build

Install Rust with `cargo` and `rustc` available on PATH. From `phix/`:

```sh
cargo build --release
./target/release/phix build examples/web -o app
./app
```

Open `http://127.0.0.1:9510/` (default bind/port). The output executable is `app`; deployment `config.json` and configured upload storage are created beside it. The CLI also accepts `phix check PROJECT` and `--emit-rs FILE` for retaining generated Rust; `check` is only a limited compiler check, not a comprehensive PHP validator. You may compile a `.php`/`.phx` file directly, or a project directory.

For tests: `cargo test --offline`, followed by the shell scripts under `tests/` (Linux shell and local networking required).

## 3. Source layout and extensions

Projects use `.php` and `.phx` files. `<?php` is supported; PHP-like indentation is not enforced. Whitespace, tabs, CRLF and comments (`//`, `#`, `/* ... */`) can separate tokens; string contents are preserved. `include`, `require`, `include_once`, `require_once` are **compile-time, statically resolved** includes. Dynamic include expressions and full PHP module semantics are not implemented.

Typical project:

```text
website/
  phix.json
  config.json
  public/index.php
  public/assets/style.css
  storage/uploads/
```

## 4. Build configuration: phix.json

`phix.json` belongs in the source project and is processed **at build time**; it is not required beside the deployed binary. Example:

```json
{
  "entry": "public/index.php",
  "routing": {
    "document_root": "public",
    "deny": ["/includes/**", "/classes/**", "/vendor/**", "/storage/**"],
    "rewrites": [{"from": "/user/:id", "to": "/profile.php?id=:id"}],
    "redirects": [{"from": "/old", "to": "/new", "status": 301}],
    "fallback": "/index.php"
  },
  "storage": {"uploads": "storage/uploads", "max_file_size": 10485760}
}
```

- `entry`: source `.php` or `.phx` entrypoint. Without it, Phix looks for `index.php`, `main.php`, `index.phx`, `main.phx`; it can also discover an index/main under the document root.
- `routing.document_root`: relative public directory; defaults to `.`. Only public handlers/assets can be requested directly.
- `routing.deny`: exact URL paths or directory patterns ending `/**`; denied files may still be compile-time includes.
- `routing.rewrites`: internal rewrites to compiled public `.php`/`.phx` handlers. Supports literal segments and `:name` path parameters with query substitution.
- `routing.redirects`: local URL redirects, statuses `301`, `302` (default), `303`, `307`, `308`.
- `routing.fallback`: compiled public `.php`/`.phx` handler for unmatched URLs, useful for a front controller. Without fallback, unmatched paths return 404.
- `storage.uploads`: relative path to persistent writable uploads; **uploads are disabled if omitted**.
- `storage.max_file_size`: per-file upload limit in bytes; default 10 MiB when uploads enabled, maximum configured value 32 MiB.

Invalid schema fields and unsafe paths are rejected. Routing is compiled into the binary; rebuilding is required to change it. Phix blocks access to `phix.json`, `config.json`, dotfiles and certain sensitive extensions regardless of explicit deny rules. Arbitrary Apache `.htaccess` expressions and external redirect targets are not supported.

## 5. Runtime configuration: config.json

`config.json` is read **beside the executable at startup**, and is not embedded as a public asset. If missing during build, a project config is copied, or a default config is generated. Existing deployment config is **not overwritten** on rebuild.

```json
{
  "runtime": {"bind": "127.0.0.1", "port": 9510},
  "website": {"name": "My site"},
  "database": {"host": "localhost"}
}
```

`runtime.bind` currently expects an IPv4 address and `runtime.port` an integer 1–65535. `runtime` is reserved. Application keys can be read using **limited, literal, two-level** access such as `$_PHIX['website']['name']`; this does not imply full PHP array support. Protect credentials and config file permissions. No database driver is implemented by the presence of a `database` configuration object.

## 6. Supported language syntax (subset)

- `<?php` source and PHP-like flexible whitespace/comments; strings with single or double quotes (within implemented lexer rules).
- Integer, boolean, and string expressions; variables with `$` prefix; assignment with static inferred types.
- `echo`, `if (...) { ... }`, `while (...) { ... }`, `return`, and user functions with **required typed parameters and return types**, for example `function add(int $a, int $b): int { return $a + $b; }`.
- Operators: integer `+ - * /`; string concatenation `.`; logical `! && ||`; comparisons `== === != !== < > <= >=` with type restrictions. `===` and `!==` currently compile to native equality/inequality, not complete PHP comparison semantics.
- Static compile-time includes and basic independent PHP/PHX HTTP entrypoints.

This is not full PHP grammar: classes, objects, namespaces, dynamic arrays, `foreach`, exceptions, closures, nullable/union types, full PHP coercion and many standard statements are not implemented. Ordinary PHP functions without explicit types may fail to compile.

## 7. Built-in functions: complete current compiler-recognized list

| Function | Inputs | Return / behavior | Important limitation |
|---|---|---|---|
| `htmlspecialchars(value)` | One scalar value | HTML-escaped string | Limited one-argument implementation; not all PHP flags/encodings |
| `asset_text(path)` | String embedded asset path | String contents | Requires valid UTF-8 |
| `asset_exists(path)` | String embedded asset path | Boolean | Checks compiled resource table |
| `asset_size(path)` | String embedded asset path | Integer byte length | Resource is immutable |
| `asset_bytes(path)` | String embedded asset path | Byte vector | Cannot directly `echo` bytes |
| `asset_write(path)` | String embedded asset path | Writes original bytes into HTTP response | Intended for binary response output |
| `move_uploaded_file(token, destination)` | `$_FILES` upload token and string-compatible destination | Boolean indicating save success | Destination constrained to configured storage; cannot overwrite |

User-defined typed functions are also supported. The compiler does **not** automatically provide all standard PHP functions (for example `strlen`, `json_encode`, `file_put_contents`, `header`, `session_start`). Do not assume unlisted functions are available.

### Examples

```php
<?php
$name = $_GET['name'];
echo htmlspecialchars($name);
```

```php
<?php
if (asset_exists('assets/logo.png')) {
    asset_write('assets/logo.png');
}
```

```php
<?php
if ($_FILES['photo']['tmp_name']) {
    if (move_uploaded_file($_FILES['photo']['tmp_name'], 'photo.png')) {
        echo 'Saved';
    }
}
```

`move_uploaded_file` receives an **opaque in-memory upload identifier** in `tmp_name`, not a conventional PHP temporary filesystem path. Uploads are not saved automatically.

## 8. Superglobals and request data

| Expression | Supported behavior |
|---|---|
| `$_PHIX['section']['key']` | Read-only runtime JSON configuration; limited literal-key indexing |
| `$_GET['key']` | Basic decoded URL query parameter lookup |
| `$_SERVER['REQUEST_URI']` | Original requested URL, including rewrite-aware routing behavior |
| `$_SERVER['SCRIPT_NAME']` | Selected compiled PHP handler path |
| `$_SERVER[...]` | Only a basic subset of server metadata is available |
| `$_FILES['field']['name']` | Uploaded filename |
| `$_FILES['field']['size']` | Uploaded byte count |
| `$_FILES['field']['type']` | Client-reported content type (untrusted) |
| `$_FILES['field']['error']` | Limited error information |
| `$_FILES['field']['tmp_name']` | Opaque upload token for `move_uploaded_file` |

These are limited compiler/runtime accessors, **not general-purpose mutable PHP arrays**. `$_POST`, `$_COOKIE`, `$_SESSION` and complete PHP superglobal semantics are not implemented.

## 9. Uploads, persistence and security

Upload storage is an optional **external** directory. On first deployment, the configured source upload tree is copied next to the executable without overwriting existing files; later rebuilds preserve it. A `.phix-storage-initialized` marker records initialization; do not delete it casually. Uploaded content is not embedded into the executable or served directly as a public static resource.

Incoming multipart files are kept request-local in memory until explicitly saved. Current limits include up to 16 files per request and configured per-file limits. Paths are constrained to the configured upload root: absolute paths, `..`, dotfiles, backslashes and symlink traversal are rejected; existing destination files are not overwritten. Applications must validate authorization, content type and file contents before saving. There is no general file-download API yet. Consult `phix/STORAGE.md` for further implementation details.

## 10. Embedded assets and HTTP

Public CSS/JS/images can be compiled into the executable and served from the configured document root. Runtime `asset_*` helpers can read compiled assets independently of HTTP exposure. The built-in server supports basic HTTP/1.1 routing, GET/HEAD, and a limited multipart upload path; it is **single-threaded and not production hardened**. Standard PHP POST fields, cookies, sessions, `header()`, full MIME/body handling, authentication middleware, and robust production-grade HTTP security are not available yet.

## 11. Examples and verification

- `phix/examples/web/`: website with embedded assets, config and upload form.
- `phix/examples/uploads/`: minimal multipart upload example.
- `phix/examples/routing/`: routing examples.
- `phix/tests/`: integration shell scripts for routing, upload storage, web demo, formatting, assets, config and migration parity.

See `phix/ROUTING.md`, `phix/STORAGE.md`, and `phix/FORMATTING_COMPATIBILITY.md` for additional technical notes.

## 12. Planned, not implemented

General PHP arrays and dynamic values; PHP classes/objects/traits/interfaces; namespaces and autoloading; complete PHP standard library; full request/response APIs; POST fields, cookies and sessions; database drivers; filesystem APIs with explicit sandboxing; robust asynchronous/concurrent HTTP; production hardening; full PHP framework compatibility; native extension/FFI manifest and linking. These are goals, **not current capabilities**.

## Documentation maintenance policy

For each future change: update this file in the same deliverable, add every new public function to the built-in function table with parameters/return types/examples, describe config schema changes and security consequences, update supported/unsupported lists, and ensure examples and tests agree. Keep the root `README.md` focused on what Phix is and its long-term scope, without milestone or version references.

## Security foundations and database API design (in progress)

### Verified HTTP parser hardening

The Rust HTTP runtime rejects headers exceeding 32 KiB while reading (without first buffering beyond the cap), more than 100 headers, invalid header names/control bytes, duplicate or missing HTTP/1.1 Host headers, unsupported HTTP versions, malformed request targets, duplicate Content-Length, and any Transfer-Encoding framing. Unsupported transfer framing is rejected instead of attempting ambiguous parsing. Existing read/write socket timeouts are 5 seconds. These are incremental safeguards, **not** a complete production HTTP security audit.

### Planned security controls — NOT YET IMPLEMENTED

- Configurable maximum concurrent requests, total request memory, aggregate storage quota, request deadlines, per-IP rate limits, and resource budgets.
- Filesystem read/write capability allowlists enforced with OS-level no-follow and race-resistant operations. Current upload path checks do not constitute a general-purpose sandbox.
- Secure session and cookie APIs, CSRF helpers, database query timeouts, structured redacted logging, fuzzing, and native FFI trust policies.
- Production HTTP validation and load testing; the current server is a single-threaded prototype.

### Database interfaces — DESIGN ONLY, NOT IMPLEMENTED

Phix will expose two PHP-compatible APIs sharing a single Rust database core:

- `mysqli` / `new mysqli(...)` for MySQL and MariaDB, with prepared statements, transactions, and PHP-compatible result handling.
- `PDO` / `new PDO(...)` for MySQL, MariaDB, and SQLite, with prepared statements, transactions, fetch modes, and exceptions.

SQLite is intentionally **not** supported through mysqli, matching PHP's separation of drivers.

Proposed build-time `phix.json` storage policy (not yet accepted by the compiler):

```json
{
  "storage": {
    "uploads": "storage/uploads",
    "max_file_size": 10485760,
    "sqlite": "storage/database"
  }
}
```

`storage.sqlite` will authorize only database files inside that directory, with symlink-safe, race-resistant file operations. SQLite `.db` files, journal files, and WAL/SHM sidecars must remain persistent and writable, but never be exposed by the HTTP server or embedded into the executable. Database path permissions are independent of upload permissions. SQLite should not be used over unsupported network filesystems.

Runtime `config.json` may hold MySQL/MariaDB host, port, username, password and database name; it must not be embedded into the executable. Prefer least-privilege credentials and TLS as appropriate. SQLite path configuration will be constrained to the build-time authorized directory; runtime configuration must not broaden the sandbox.

Planned PHP examples (these will NOT run on current Phix):

```php
<?php
$mysqli = new mysqli('127.0.0.1', 'app', 'secret', 'appdb');
$stmt = $mysqli->prepare('SELECT name FROM users WHERE id = ?');
$stmt->bind_param('i', $id);
$stmt->execute();
```

```php
<?php
$pdo = new PDO('sqlite:storage/database/app.db');
$stmt = $pdo->prepare('SELECT name FROM users WHERE id = :id');
$stmt->execute(['id' => $id]);
```

These APIs require future PHP object/class, array, and dynamic-value support and real database drivers. The examples describe the target API, not current behavior.

## HTTP resource limits (implemented)

The embedded HTTP runtime supports these optional settings in the deployment-side `config.json`:

```json
{
  "runtime": {
    "bind": "127.0.0.1",
    "port": 9510,
    "max_request_body": 16777216,
    "max_header_bytes": 16384,
    "max_concurrent_requests": 32,
    "io_timeout_seconds": 5
  }
}
```

- `max_request_body`: 1 to 67,108,864 bytes; default 16 MiB. The existing upload-specific limit can be more restrictive.
- `max_header_bytes`: 1,024 to 32,768 bytes; default 16 KiB.
- `max_concurrent_requests`: 1 to 256 active workers; default 32. Excess connections receive HTTP 503.
- `io_timeout_seconds`: 1 to 30 seconds; default 5 seconds for each blocking socket read/write operation. This is **not** an overall request deadline.

These settings are read at startup. They limit individual connections, but do not yet provide a process-wide memory budget, byte-rate enforcement, per-IP throttling, or a robust request-execution deadline. The HTTP server is still experimental.

## Database API plan (not implemented)

Phix plans to expose `mysqli` and `PDO` for MySQL/MariaDB and `PDO` for SQLite. SQLite files will require an explicitly configured persistent storage directory outside the executable. No database API or SQLite storage configuration is implemented in this milestone.

## Centralized Phix version

The single source of truth for the Phix version is `phix/Cargo.toml`, under `[package]` → `version`. The current version is **v0.0.1**. Change that one value for future releases; Cargo updates `Cargo.lock` as needed when building.

The compiler reads the version at compile time via `env!("CARGO_PKG_VERSION")`, and the generated website executable embeds the same version. Run `phix --version` (or `phix -V` / `phix version`) to display the compiler version; run `./app --version` (or `./app -V`) to display the version embedded at build time. A deployed app retains the version it was built with until recompiled.

`README.md` intentionally contains no version number.

## PHP built-in implementation layout

Built-in PHP-compatible functions are organized under `phix/src/php/`. The `php/mod.rs` dispatcher delegates to feature-specific modules. Currently migrated: `strings/htmlspecialchars.rs`, `filesystem/move_uploaded_file.rs`, and the `assets/mod.rs` asset function family. The compiler calls this dispatcher rather than embedding each built-in's code generation in `compiler.rs`.

This is an organizational refactor, **not** new PHP compatibility. `strlen`, `substr`, `count`, classes, sessions, PDO, MySQLi and SQLite are not implemented by this change. Database work is deferred until later milestones.

When adding a function: create or extend its category module, register it in the category's dispatcher, implement argument checking and generated Rust expression, add a regression test, and document the exact supported semantics here. Runtime implementations can be separated into their own modules as the shared runtime evolves.

## Compiler diagnostics

Phix checks source syntax during `phix build` and `phix check`. Missing semicolons, invalid expressions, unknown functions, and some type mismatches cause a nonzero exit status. Parser errors with line numbers display a source filename and a line excerpt when available, and missing semicolons get a suggestion. The current parser does not provide precise column spans or multiple accumulated diagnostics. Some compiler errors still lack source locations. Included-file location mapping is best effort.

`src/diagnostics.rs` owns the diagnostic formatter, independently of the lexer and parser. The project still uses Rust edition 2021 and version 0.0.1. Database work remains deferred.

## Compiler module organization

The compiler is now organized in `phix/src/compiler/`:

- `types.rs`: internal compiler types (`Ty`, function signatures, parser state).
- `parser.rs`: token parsing, expression checking, statement validation, and PHP built-in dispatch.
- `codegen.rs`: Rust source generation for PHP/Phix functions and request entrypoints.
- `mod.rs`: public compiler entrypoint.

These are **structural refactors** and do not add new PHP language features. Diagnostics remain in `src/diagnostics.rs`; the lexer remains in `src/lexer.rs`. The generated HTTP runtime is still maintained in `runtime/web_runtime.rs` and needs further extraction. The compiler's current type representation is still limited and is not yet the planned dynamic PHP value model.

Database development remains deferred. The project continues to use Rust edition 2021 and version v0.0.1. `README.md` intentionally contains no version numbers.


## Built-in function registry

`phix/src/php/registry.rs` is the authoritative inventory of compiler-recognized PHP-compatible built-ins. Each entry specifies its function name, category, and minimum/maximum argument count. The dispatcher in `php/mod.rs` uses the registry to route calls to the correct implementation module.

The registry currently lists `htmlspecialchars`, `asset_text`, `asset_exists`, `asset_size`, `asset_bytes`, `asset_write`, and `move_uploaded_file`. This inventory describes **compiler-recognized built-ins**, not the complete PHP standard library. The per-function implementation still performs its own argument validation. In particular, registry metadata is not yet a full PHP signature/type system.

To add a function, register it, implement it in its category, add compile-time and runtime tests, and update this reference. The new registry has tests for duplicate names and lookup behavior. No new PHP functions are added in this change. Databases remain deferred.


## PHP value foundation (internal, experimental)

`phix/src/php/value.rs` introduces an internal `Value` enum for `Null`, `Bool`, `Int`, `Float`, `String`, and insertion-ordered `Array` values. `ArrayKey` currently supports integer and string keys. This is an **internal foundation only**: the compiler does **not yet** emit these values, and PHP source code cannot yet declare array literals, floats, or null through this module.

Implemented internal operations: `truthy()`, `type_name()`, `php_string()`, `array()`, `get()`, `set()`, and `len()`. Unit tests cover scalar truthiness, insertion order, key replacement, and basic string conversions. Not yet implemented: PHP array-key coercion, append-index rules, loose comparisons, numeric-string coercion, warnings/notices, float formatting parity, reference semantics, and object values. Array-to-string conversion currently returns `Array` without the warning PHP emits.

The next work is integrating values into compiler expressions and generated runtime with compatibility tests. Databases remain deferred. Phix version stays v0.0.1 and Rust edition stays 2021.

## PHP compatibility differences

See [`PHP_COMPATIBILITY.md`](PHP_COMPATIBILITY.md) for the maintained ledger of known PHP differences, intentional restrictions, unsupported features, and reproduction examples. New PHP-semantic changes must update this ledger when they introduce, resolve, or discover differences.

### Internal value-system improvements

`phix/src/php/value.rs` now includes `ArrayKey::from_php_value` for common PHP-style array-key conversions and `Value::append` for basic indexed insertion. These are **internal-only** helpers and are not yet exposed through PHP array literal/index syntax. The append implementation does not yet preserve PHP's historical next-free-index state; see COMPAT-003.

## Historical array next-index tracking

`phix/src/php/value.rs` now represents an array using `PhpArray` with ordered entries, a historical `next_index`, and a flag indicating whether any numeric key has been inserted. `Value::remove(&ArrayKey)` removes a key without resetting the next append index. `Value::append(Value)` uses the tracked index rather than recalculating from existing keys. Negative-key handling targets PHP 8.3+ in the tested common cases. Overflow currently returns an internal error, which is not a claim of PHP error-message parity.

**Important:** This is an internal implementation, not PHP syntax support. The generated runtime does not yet use `Value::Array`, and PHP `unset()` and array literals remain unsupported. See `PHP_COMPATIBILITY.md`, especially COMPAT-001 and COMPAT-003.

## PHP array source syntax (experimental)

Phix can now compile simple indexed literals, e.g. `$a = [1, "two", true, null]; echo $a[0];`. Nested arrays and empty arrays are accepted. Numeric index reads return `null` for missing positions. This currently uses generated `Json` runtime values, not the full PHP `Value` representation. Associative `=>` literals, offset assignments, append syntax, references, and PHP warning parity are not supported. See `PHP_COMPATIBILITY.md` COMPAT-011. Database APIs remain deferred.

## Source-level arrays (partial implementation)

Phix accepts indexed literals (`[10, 20]`), string-keyed associative literals (`['name' => 'Alex']`), reads (`$a[0]`, `$user['name']`), nested reads (`$a[0][1]`), element assignment (`$a[0] = 42`, `$user['name'] = 'Sam'`), and append to indexed arrays (`$a[] = 42`). Values may be scalars, `null`, or nested arrays. This is generated using `Json::Array`/`Json::Object`, not yet the full PHP `Value` runtime.

Unsupported or different behavior includes mixed-key literals, numeric keys in associative literals, nested assignment, sparse writes, PHP missing-key warnings, and complete key coercion. See `PHP_COMPATIBILITY.md` COMPAT-012. Do not assume full PHP array compatibility.


## PHP-source arrays: dedicated runtime storage (current)

The compiler now emits `Json::PhpArray` backed by `runtime/php_array.rs` for PHP-source array literals. The runtime preserves insertion order, supports mixed integer/string keys, canonical integer-string conversion, sparse integer assignments, and historical append indices. This replaces the earlier generated `Json::Array` / `Json::Object` array-literal strategy; those variants remain in use for strict JSON configuration parsing. Nested reads and simple element assignment/append remain supported. `php/value.rs` still exists separately and has **not** been consolidated with the generated runtime; full PHP compatibility is not claimed. See COMPAT-013 in `PHP_COMPATIBILITY.md` for the remaining gaps.

## Array element deletion (partial PHP compatibility)

Phix supports `unset($array[$key]);` for one key of an array-compatible local variable. Integer and string key expressions are supported. Removing an element preserves the historical next numeric index used by `$array[]` append. Nested deletion, multiple arguments, and variable unsetting are not yet supported. See `PHP_COMPATIBILITY.md` COMPAT-014 for the exact limitations.

### Nested array mutation

PHP-source arrays support nested element assignment (`$a['x']['y'] = 1`), nested append (`$a['x'][] = 2`), and multiple nested element deletions (`unset($a['x']['y'], $a['z'][0])`). Missing intermediate elements are created as empty Phix arrays by the current generated runtime. Nested reads were supported previously. See `PHP_COMPATIBILITY.md` COMPAT-015 for differences from PHP, including invalid intermediate values and references. This feature is verified by `phix/tests/array_nested_integration.sh`.

### Array copies and key coercion (partial)

Array variable assignment (`$b = $a`) copies array values, including nested arrays. Boolean keys are accepted and normalized to integer keys (`true` to 1, `false` to 0), while null keys normalize to the empty string. The compiler now accepts these key expressions in array indexing, assignment, and deletion. See `PHP_COMPATIBILITY.md` COMPAT-016 for limitations, particularly references and warnings. Test: `phix/tests/array_copy_keys_integration.sh`.

### Strict PHP array comparisons

Phix supports strict identity (`===`) and strict nonidentity (`!==`) between PHP array values. Keys and values must match recursively **in insertion order** and scalar types must match. `==`/`!=` array comparisons are not yet implemented. The executable runtime's `Json::PhpArray` comparison is implemented in `phix/runtime/json.rs`; compiler handling is in `phix/src/compiler/parser.rs`. See `PHP_COMPATIBILITY.md` COMPAT-017 and `phix/tests/array_strict_comparison.sh` for differential verification.

## Loose PHP array comparison

PHP-source arrays support `==` and `!=` for recursive, key-order-insensitive comparisons, in addition to existing order-sensitive `===` and `!==`. This is partial PHP compatibility; see `PHP_COMPATIBILITY.md` COMPAT-018 for limitations. Run `phix/tests/array_loose_comparison.sh` to compare compiled website output against PHP.

### PHP array equality correction
PHP loose array comparison (`==`/`!=`) now handles `null` versus string values using PHP 8.x semantics, including nested array entries. See `PHP_COMPATIBILITY.md` COMPAT-019 for the regression and remaining compatibility limitations.

## Experimental PHP reference-slot foundation

The Rust module `phix/runtime/php_reference.rs` contains the internal `PhpSlot` type, using `Rc<RefCell<Json>>` to distinguish a copied PHP array value from a shared variable binding. Operations include `new`, `copy_value`, `bind_reference`, `read`, `replace`, `set_path`, `unset_path`, and `same_binding`. The module is compiled into the **Phix compiler's test/runtime support** but is **not currently included in generated website executables**. PHP source-level `=&` remains unsupported. See `PHP_COMPATIBILITY.md` COMPAT-020. This milestone introduces no new PHP functions and does not alter the project's Rust edition or package version.

## PHP array variable references (partial)

For array-compatible variables, `$b = &$a;` binds `$b` to the same mutable value slot as `$a`. `$b['key'] = $value` mutates both views, while `$c = $a` creates an independent value copy. Assigning another array to `$b` replaces the shared value. Implemented using `runtime/php_reference.rs` and compiler-generated `PhpSlot` locals. See `PHP_COMPATIBILITY.md` COMPAT-021 for unsupported reference forms.

### Array element references (partial)

Phix accepts `$alias = &$array['key'];` and nested paths such as `$alias = &$array['profile']['name'];`. Assigning a scalar or array to the alias updates the referenced element in generated websites. Ordinary array copies are independent in tested cases. The implementation currently uses a root-slot and key-path alias, **not** PHP's full identity-preserving reference cells; see `PHP_COMPATIBILITY.md` (COMPAT-022). Do not rely on alias behavior after `unset()` or replacing a parent array element yet.

## Experimental identity-preserving PHP reference cells

`phix/runtime/php_reference_cell.rs` contains `PhpCell` (shared mutable identity) and `PhpCellEntries` (reference-aware keyed container). This is a tested internal prototype, **not a source-language feature**: generated PHP array operations still use the path-based `PhpSlot` implementation. The new tests cover deletion, reinsertion, and mutation of an aliased element. See COMPAT-023 in `PHP_COMPATIBILITY.md` for remaining differences and integration work.


### Array element reference-cell integration (COMPAT-024)

Generated PHP-source arrays now store elements in identity-preserving `PhpCell` objects backed by `Arc<Mutex<Json>>`. This allows **direct element references** (`$ref = &$array["key"]`) to retain their identity when the key is deleted and subsequently recreated. Ordinary array copies deep-copy element cells; key ordering and next-index tracking remain part of `PhpArray`. Configuration JSON parsing remains separate.

Verified with `tests/array_cell_identity_integration.sh` against a compiled HTTP website, the existing array integration suites, and Rust unit tests. This is **not complete PHP reference compatibility**: nested element aliases still use the earlier path-based mechanism; PHP arrays containing references do not yet preserve reference identity across value copies as PHP does; recursive reference structures and some reference rebinding behavior remain unsupported. The Rust implementation uses mutexes for thread safety; no claim of PHP-style cross-request shared state is implied.

### Nested array reference identity

Nested PHP array element aliases now resolve to identity-preserving `PhpCell` objects, rather than using path-only references. The runtime traverses nested arrays without copying intermediate arrays; aliases survive deletion and parent replacement. The `tests/array_nested_cell_identity.sh` integration test compares compiled HTTP output with PHP CLI. See COMPAT-025 in `PHP_COMPATIBILITY.md` for limitations. Phix remains v0.0.1 with Rust edition 2021.

### Array copy reference semantics

PHP array copies preserve explicitly referenced elements while copying ordinary elements. The generated runtime now tracks reference identity in `PhpCell` and retains marked cells during `PhpArray` cloning. The `tests/array_embedded_reference_copy.sh` differential integration test verifies the behavior against PHP CLI. This does not yet support reference expressions inside array literals or complete PHP reference semantics. See `PHP_COMPATIBILITY.md` (COMPAT-026).

### References inside PHP array literals

Phix supports references to existing array elements in literals, for example `$a = ["value" => 1]; $b = ["ref" => &$a["value"]];`. Writes to `$b["ref"]` update the original element. Copies of arrays preserve reference identity for referenced entries, while non-reference entries are copied. This feature is implemented by `PhpArray::from_cell_parts` and `PhpSlot::reference_cell`. Whole-variable literal references (`[&$a]`) are not yet supported. See `PHP_COMPATIBILITY.md` COMPAT-027 for limitations.

### Whole-array references inside array literals

Phix supports references to entire array variables inside PHP array literals, e.g. `$a = ["x" => 1]; $b = ["ref" => &$a];`. Assignments through `$b["ref"]` modify `$a`; copies of `$b` preserve the reference. Internally, the generated `PhpSlot` uses an identity-preserving root `PhpCell`. See `PHP_COMPATIBILITY.md` COMPAT-028 for known limitations and `phix/tests/array_literal_whole_variable_reference.sh` for the PHP differential test. This does not imply full PHP reference or array compatibility.

### Nested mutation reference identity
Nested set, append and unset operations now traverse existing PhpCell storage instead of cloning intermediate arrays. New Rust regressions verify sibling reference identity, detached aliases after nested unset, and append behavior. See COMPAT-029. This is partial PHP compatibility, not complete array parity.

### Float array key compatibility

PHP-source arrays convert floating-point keys to signed 64-bit integer keys. The runtime handles fractional truncation, finite out-of-range wrapping, and non-finite inputs (key 0). See COMPAT-030 in `PHP_COMPATIBILITY.md` for verification scope and remaining differences. This is not a claim of complete PHP array compatibility.

### Array literal references: null-initialized variables

A null-initialized PHP variable uses Phix's reference-capable `PhpSlot` representation and can be referenced from an array literal, e.g. `$value = null; $a = ['ref' => &$value];`. The array's referenced cell stays shared across normal array copies, while ordinary entries copy by value. Run `phix/tests/array_literal_null_scalar_reference.sh` to compare a compiled HTTP response against PHP CLI. Native scalar variables (integer/string/boolean) are **not yet** accepted as array literal reference targets; see COMPAT-030 in `PHP_COMPATIBILITY.md`.

### Native scalar reference promotion

For `$alias = &$value`, the compiler promotes an existing integer, string, or boolean variable into a shared `PhpSlot` and subsequent operations observe the same cell. This is covered by `tests/array_native_scalar_variable_references.sh`. Scalar references written directly in array literals remain unsupported; see COMPAT-031 in `PHP_COMPATIBILITY.md`.

### Scalar references in array literals

Array literals support references to existing integer, string, boolean, and PHP-compatible variables, e.g. `$number = 10; $a = ['ref' => &$number]; $a['ref'] = 25;`. Scalar variables are promoted to shared reference slots before array literal evaluation. `tests/array_literal_scalar_reference.sh` compares the compiled HTTP response against PHP CLI. See COMPAT-032 for limitations; this does not imply full PHP array compatibility.

### PHP array loose comparison against bool/null

For PHP-source arrays, `==` and `!=` now follow PHP 8.4 behavior when compared to `Json`-represented booleans or null: empty arrays loosely equal `false` and `null`, nonempty arrays loosely equal `true`; arrays are not loosely equal to integer zero or an empty string. Verified against PHP CLI by `phix/tests/array_boolean_null_loose_comparison.sh`. The compiler still restricts comparisons involving native scalar-typed variables versus `Json` arrays; see COMPAT-033 in `PHP_COMPATIBILITY.md`.

### Reference cycle inspection

The runtime exposes an internal `Json::phix_has_reference_cycle()` helper to identify cyclic PHP reference-cell graphs without recursive mutex acquisition. It is not a PHP built-in or a complete implementation of PHP's recursive comparison, serialization, or garbage collection behavior. See COMPAT-035.

### Checked recursive array comparison (experimental)

The internal `Json::phix_checked_array_eq(other, strict)` API detects cyclic array references and returns a `Result<bool, String>`; cyclic inputs currently return `Nesting level too deep - recursive dependency?`. It is verified by two Rust unit tests. This API is not yet used by compiler-generated PHP equality expressions and is not a claim of complete PHP array comparison compatibility. See COMPAT-036.

### Checked array equality in compiled expressions

PHP-value equality operators `==`, `!=`, `===`, and `!==` now call `Json::phix_checked_array_eq` in generated Rust. This prevents a known recursive mutex deadlock for cyclic array values. The cycle preflight is conservative and error propagation is not yet PHP-compatible; see COMPAT-037 in `PHP_COMPATIBILITY.md`.

### Cycle-aware checked array equality
`Json::phix_checked_array_eq` compares PHP array elements lazily and tracks active reference-cell identity pairs. This allows mismatches to short-circuit before traversing cycles and reports a recursion dependency if an active pair is encountered again. Two Rust regression tests cover both cases. The compiler currently maps checked-comparison errors to a Rust panic, **not** a PHP-formatted exception. See `PHP_COMPATIBILITY.md` COMPAT-040. This is not complete PHP array compatibility.

### Mixed native scalar and PHP-array equality

Equality operators `==`, `!=`, `===`, and `!==` accept PHP arrays compared with supported native integer, string, and boolean values in either operand order. The compiler converts native operands into PHP-value representations before invoking the checked comparison. Verified by `tests/array_mixed_native_scalar_equality.sh` against PHP CLI and by a compiler regression test. See COMPAT-041 in `PHP_COMPATIBILITY.md` for limitations. Recursive comparison errors still use Rust panic behavior rather than PHP-compatible errors.

### Cyclic array equality: identical reference cells

The checked PHP array equality implementation now recognizes two operands that reference the same element cell and treats them as equal without traversing their recursive contents. This matches PHP's self-comparison and shared-reference array-copy behavior. Separate recursive graphs continue to report a recursion dependency. Rust regression tests cover both cases. See `PHP_COMPATIBILITY.md` (COMPAT-042) for unresolved error formatting, copying, and other compatibility gaps.

### Recursive comparison HTTP errors

Compiled `==`, `!=`, `===`, and `!==` PHP-value comparisons now use `phix_checked_equality`, which records recursive comparison failures in thread-local request state. The HTTP runtime returns a 500 response instead of a Rust panic and does not expose partial page output. This is an interim fatal-error path, not full PHP `Error` exception semantics. See COMPAT-043.

### Checked nested collection comparisons

The runtime's checked equality traversal now handles nested `Json::Array` and `Json::Object` containers without falling back to unchecked recursive equality. It uses the same reference-pair recursion guard as `PhpArray`, and short-circuits on an earlier mismatch. Verified with two new Rust unit tests and the existing compiled HTTP recursive-comparison test. This does not implement PHP catchable exceptions or complete cyclic-array semantics; see COMPAT-044 in `PHP_COMPATIBILITY.md`.

### Cycle-safe reference-cell diagnostics

`PhpCell` uses a custom, non-recursive Rust `Debug` implementation. It prints the cell identity and reference marker, never the contained `Json`, so formatting self-referential arrays cannot recursively traverse the same cell. Two unit tests cover cyclic cells and arrays. See `PHP_COMPATIBILITY.md` COMPAT-045. This is an internal diagnostic guarantee, not PHP user-facing `var_dump` compatibility.

### Self-referential array copy regression (COMPAT-046)

The runtime test suite now checks that copying an array with an embedded reference to its own original root retains the original reference identity, rather than redirecting that reference to the copy. It also checks ordinary copied-element independence and cycle detection. These are verified Rust runtime behaviors, not a claim of full PHP array compatibility. See `PHP_COMPATIBILITY.md` for remaining gaps.

### PHP array ordering operators

The compiler supports `<`, `>`, `<=`, and `>=` on PHP-value arrays through `phix_checked_order`. The comparator handles array length, matching keys regardless of insertion order, and incomparable key sets. Recursive references use a comparison guard. The differential test is `phix/tests/array_ordering_php_parity.sh`. Mixed scalar ordering and some PHP coercion/error semantics are not yet supported; see COMPAT-047 in `PHP_COMPATIBILITY.md`.

### Array/scalar ordering comparisons

Phix supports PHP-like `<`, `>`, `<=`, `>=` comparisons between arrays and booleans, null, integers, floating-point values, and strings. Arrays versus booleans/null follow boolean conversion; arrays order above numeric/string scalars. Both operand orders are covered. Run `bash phix/tests/array_ordering_scalar_parity.sh` to compare compiled HTTP output against PHP CLI. This is verified for the included tests, not a claim of full PHP 8.x array conformance; see COMPAT-048.

### Mixed scalar ordering inside arrays

Array ordering now supports comparisons of numeric strings with numbers, nonnumeric strings with numbers (PHP 8 string comparison behavior), booleans, and null. The four Rust regression tests are in `runtime/json.rs`; see COMPAT-049 for limitations. This does not establish complete PHP 8.x array compatibility.

### Array reference rebinding regression coverage

Phix supports rebinding an existing reference variable to another variable or array element. Existing aliases remain connected to their former reference cell. The `phix/tests/array_reference_rebinding_parity.sh` integration test compares compiled HTTP output with PHP CLI for whole-array and element reference rebinding. Two Rust unit tests in `runtime/php_reference.rs` protect cell identity. See `PHP_COMPATIBILITY.md` COMPAT-050 for unimplemented edge cases. Async/await remains planned, not implemented; PHP array compatibility is the current priority.

### Numeric-string comparison precision

When an integer is compared with a numeric string consisting of decimal integer digits (optionally signed and surrounded by whitespace), Phix first attempts exact signed 64-bit integer comparison. This avoids loss of precision beyond 2^53 in array ordering and loose equality. Numeric strings that cannot be represented as i64 still use the existing floating-point fallback. See `PHP_COMPATIBILITY.md` COMPAT-051 for limitations. This is implemented and covered by two Rust unit tests; compiled-website differential verification remains pending.

### Exact numeric-string comparisons inside arrays
When comparing two plain decimal integer strings as array values, Phix now compares normalized decimal digits exactly rather than converting both values to floating point. This preserves differences above 2^53 and beyond signed 64-bit range. Decimal/exponent forms still follow the existing numeric conversion implementation. See `PHP_COMPATIBILITY.md` COMPAT-052 for remaining limitations. This feature is covered by three Rust unit regression tests; complete PHP array compatibility has not been established.

### Array numeric-string overflow comparison (COMPAT-053)

String/string loose equality and ordering preserve lexical differences between exponent-form numeric strings that overflow finite floating-point range (for example, `"1e309"` versus `"2e309"`). Verified by `phix/tests/array_overflow_numeric_string_parity.sh` against PHP 8.4.24, plus Rust unit tests. This does not imply complete PHP array compatibility; see `PHP_COMPATIBILITY.md`.

### PHP array differential regression matrix

Run `bash phix/tests/array_differential_matrix.sh` from any directory after building the Phix compiler (`phix/target/debug/phix` must exist). Requires PHP CLI, curl, Rust toolchain, and a free local TCP port 19876. The script builds a temporary standalone Phix HTTP website, runs the identical PHP source under PHP CLI, and checks exact output equality for 14 labeled cases. It cleans up its temporary files and server. Passing this matrix does not imply full PHP 8.x array compatibility; see `PHP_COMPATIBILITY.md` COMPAT-054.

### PHP array differential regression matrix

Run `bash tests/array_differential_matrix.sh` from `phix` after building the Phix compiler. The script uses PHP CLI as the behavioral oracle and checks the compiled website over HTTP. Current matrix: 28 cases (expanded from 14), verified against PHP in the development environment. This is a selected regression matrix, not full PHP array conformance. See `PHP_COMPATIBILITY.md` COMPAT-055.

### Array key boundary differential tests

Run `bash phix/tests/array_differential_matrix.sh` after building `phix/target/debug/phix` to compare PHP CLI output with the compiled Phix HTTP website. The current matrix includes 32 output cases covering comparisons, references, array copies, numeric indexes, and key conversion boundaries. Two Rust unit tests additionally verify numeric-string key conversion at signed 64-bit limits and replacement order. This is regression coverage, not a claim of full PHP compatibility. See COMPAT-056 in `PHP_COMPATIBILITY.md`.

### PHP array union operator

Phix supports `$result = $left + $right` for PHP arrays. Entries from the left take precedence on duplicate keys, while new right-hand keys are appended in order. Reference-aware copying retains explicit PHP reference identity. `phix/tests/array_union_parity.sh` compares four compiled HTTP cases with PHP CLI; `array_differential_matrix.sh` covers 32 other cases. Non-array operands and `+=` are not yet fully PHP-compatible. See COMPAT-057 in `PHP_COMPATIBILITY.md`.

### Array union assignment (`+=`)

Phix supports PHP-style `$array += $otherArray` for PHP array variables. Existing left-hand keys win; new right-hand keys are appended in insertion order. The operation preserves reference cells and updates the left variable's binding. `tests/array_union_assignment_parity.sh` compares four compiled HTTP outputs with PHP CLI. Non-array operands and other PHP lvalue forms are not yet fully compatible; see COMPAT-058 in `PHP_COMPATIBILITY.md`.

### Nested array union assignment
Phix supports PHP array union assignment on existing array-element paths, e.g. `$data["settings"] += ["language" => "en"];` and `$data["outer"]["inner"] += ["new" => 1];`. Existing left-side keys win, and mutations are applied through the target reference cell so aliases see changes. The test `phix/tests/array_nested_union_assignment_parity.sh` compares compiled HTTP output with PHP CLI. Invalid operands and PHP exception semantics remain incomplete; see COMPAT-059.

### Invalid array-union operands
Array `+` and array `+=` now validate operand types at runtime, reporting messages such as `Unsupported operand types: array + int`. In compiled HTTP applications this currently yields HTTP 500 and is not yet a catchable PHP `TypeError`. See COMPAT-060. The `async`/`await` extension remains planned, not implemented; PHP array compatibility remains the immediate priority.

### Typed PHP-style runtime errors (COMPAT-061)
Compiled HTTP applications classify invalid PHP array-union operands as `TypeError` and recursive array-comparison failures as `Error`, returning HTTP 500 without partial output. These are **uncatchable request errors**, not PHP exception instances. PHP `try`/`catch`, full exception propagation, and source-location stack traces remain planned/unimplemented.

### Scalar-left array union assignment (COMPAT-062)
The compiler accepts PHP-style `$value += [1]` even if `$value` is an integer, boolean, or string. As in PHP, these operand combinations are invalid: the compiled HTTP runtime reports a `TypeError`-classified fatal error with HTTP 500. Unlike PHP, the error is not yet catchable with `try`/`catch`. Integer `+=` integer continues to perform arithmetic. Run `bash phix/tests/array_union_scalar_left_parity.sh` to verify the three invalid-operand HTTP cases after building the compiler. The PHP array differential suite remains limited; full array compatibility is not yet achieved.

### Fatal array error execution guards (COMPAT-063)

Compiled request handlers and nested blocks now check the request-local fatal
error state after each statement and stop executing subsequent statements once an
array operation reports an error. The HTTP runtime continues to return a 500
response with the recorded `TypeError` or `Error` classification and discards
partial response output. **Limitations:** this is statement-boundary interruption,
not PHP exception unwinding; `try`/`catch`, exception objects, and interruption
inside a single expression are not yet implemented.

### Fatal errors during comma-separated `echo` (COMPAT-064)
Each `echo` argument is evaluated and checked for a recorded fatal PHP runtime error before appending output. A failed array union in `echo "before", [1] + 2, "after";` aborts the handler and returns the existing HTTP 500 `TypeError` response without partial output. Regression: `bash phix/tests/array_fatal_echo_arguments.sh`. Catchable exceptions remain planned, not implemented.

### Fatal array errors in conditional expressions (COMPAT-065)
Generated `if` and `while` conditions check the runtime error state after evaluating an expression. A recursive array comparison or other checked array operation that records a fatal error stops request execution before the conditional body begins. This is implemented and compiler-unit-tested; PHP-compatible catchable exceptions and comprehensive conditional HTTP parity tests are not yet available.

## Exception handling (partial implementation, COMPAT-066)

The compiler supports `try { ... } catch (TypeError $error) { ... } finally { ... }` in top-level request code and void-returning blocks. Existing runtime array-operation failures can be intercepted by `catch (TypeError $e)`, `catch (Error $e)`, or `catch (Throwable $e)`. A caught error currently exposes a temporary array-like value with `class` and `message` keys, not a PHP Throwable object. `finally` runs after the try/catch path. Typed nonmatching errors propagate to the HTTP runtime.

Verified: Rust compiler tests and one compiled HTTP application catching invalid array union and executing `finally`. Not implemented: `throw`, exception object construction, methods, custom exception classes, full `finally` semantics for return/break/continue, non-void function handling, cross-function exception propagation, multiple catch alternatives, and full PHP exception hierarchy. This is **not** full PHP 8.x exception compatibility.

### Throw statements and rethrow (COMPAT-068)

**Implemented (limited):** In top-level request handlers and void-returning blocks, `throw new Exception("message");`, `throw new Error("message");`, `throw new TypeError("message");`, `throw new ValueError("message");`, and `throw new ParseError("message");` set a typed request-local pending exception. A matching `catch` can consume it; `throw $caught;` can rethrow the temporary caught exception value. Nested handlers and `finally` are exercised by `phix/tests/exception_throw_http.sh`. Invalid thrown values are classified as `Error` by the current runtime.

**Not implemented:** PHP Throwable objects, `new` as a general expression, exception constructors with previous, exception methods and stack traces, custom exception classes, arbitrary `throw` expressions, function propagation, complete `finally` control-flow semantics, or exact PHP error text. Catch bindings still use the temporary `['class' => ..., 'message' => ...]` representation. This is **not** complete PHP exception compatibility.

### Caught exception methods (COMPAT-069, partial)
In a `catch` block, Phix currently compiles `$e->getMessage()`, `$e->getCode()` and `$e->getPrevious()` for the temporary caught-exception representation. With the currently supported `throw new Exception("message")` constructor, these return the message, integer `0`, and `null` respectively. Example:

```php
try { throw new Exception("example"); }
catch (Exception $e) { echo $e->getMessage(), ":", $e->getCode(); }
```

`tests/exception_methods_http.sh` checks compiled HTTP output against PHP CLI. This is **not** a general PHP object implementation: caught values are not real Throwable objects; arbitrary method calls, custom exception codes, previous exception chaining, source locations and stack traces are not supported. Exception compatibility is unfinished.

### Exception constructor codes (COMPAT-070, partial)

**Implemented and tested:** `throw new Exception("message", 42);` and the analogous built-in `Error`, `TypeError`, `ValueError`, and `ParseError` forms accept an optional integer code. The temporary caught-exception representation now retains the code, `$e->getCode()` returns it, and `throw $e;` preserves it when rethrown. `tests/exception_constructor_code_http.sh` compares a compiled HTTP application against PHP CLI, including nested rethrow. The previous no-code constructor remains supported and defaults to `0`.

**Limitations:** These are not real PHP Throwable objects. The optional third `previous` constructor argument, custom exception classes, trace/file/line methods, throw expressions, and full exception propagation are still missing. The compiler currently accepts only integer code expressions, not the full PHP constructor coercion/error behavior.

### Exception chaining (implemented subset)
Supported built-in thrown exception constructors accept `message`, optional integer `code`, and optional `previous` Throwable value (or `null`). For example:

```php
try { throw new Exception("root", 10); }
catch (Exception $root) {
    try { throw new Exception("wrapped", 20, $root); }
    catch (Exception $outer) { echo $outer->getPrevious()->getMessage(); }
}
```

`getPrevious()` preserves nested chains through rethrow. This is a compiler/runtime compatibility subset, not yet genuine PHP `Throwable` object identity or complete exception semantics. Run `phix/tests/exception_previous_http.sh` after building the Phix compiler to compare against PHP CLI.

### Zero-argument exception constructors (implemented subset)
Built-in exception classes currently recognized by Phix (`Exception`, `Error`, `TypeError`, `ValueError`, `ParseError`) may be thrown without arguments: `throw new Exception();`. Their default message is the empty string, their default code is `0`, and their previous exception is `null`. This is verified by `phix/tests/exception_default_constructor_http.sh`, which compares a compiled HTTP response to PHP CLI. General `new` expressions, full PHP object semantics, and user-defined exception subclasses are not yet implemented.

### Union catches and catch without binding (implemented subset)
Phix accepts `catch (Exception|TypeError $e)` and `catch (Exception)` in the existing compiled exception handling subset. Union alternatives are evaluated against the supported built-in class hierarchy, and catch clauses are selected in source order. The binding is optional as in PHP 8. Verify with `phix/tests/exception_union_catch_http.sh` after building the compiler. User-defined exception classes, true Throwable objects, and full exception unwinding are not yet supported.

### Exception object model: current limitation (COMPAT-075)

Exceptions are currently serialized into JSON-shaped values. They are not
native PHP-compatible Throwable objects. `getPrevious()` and rethrowing
cannot yet be assumed to preserve object identity. A structural regression
guard is provided at `phix/tests/exception_object_identity_regression.sh`.
It is intentionally **expected to fail** until the object model is implemented.
Do not include this guard among passing regression tests.

### Native exception objects — COMPAT-075

Compiled PHP exception catches now hold a native identity-bearing `Throwable` runtime value, rather than a JSON object with `class`, `message`, `code`, and `previous` fields. `getMessage()`, `getCode()`, `getPrevious()`, and rethrowing a caught exception use the native object. Previous-exception chains retain the same object identity. PHP arrays or JSON objects cannot impersonate a Throwable. Differential test: `phix/tests/exception_native_identity_http.sh`. This is a limited built-in exception model, **not** complete PHP object support: custom classes, trace methods, typed-function unwinding, and complex finally semantics remain unimplemented.

### COMPAT-076: throwing from typed functions (partial)
Typed functions returning `int`, `bool`, or `string` can throw supported built-in exceptions. The exception propagates through the request-local error channel to a surrounding caller's `try/catch`. Example:

```php
function read_value(): int { throw new Exception('unavailable'); }
try { echo read_value(); } catch (Exception $e) { echo $e->getMessage(); }
```

Verification: `cargo test --offline` (126 passed), `tests/exception_typed_function_http.sh`, `tests/exception_throw_expression_http.sh`, and `tests/exception_native_identity_http.sh` passed. This is **not** full PHP function exception compatibility: `try/catch/finally` within value-returning functions, custom exception classes, and complete control-flow unwinding are still pending. The current compiler uses a default return sentinel on error, not a structured `Result`-based function ABI.

### COMPAT-077 — typed function try/catch/finally (partial)

Typed `int`, `bool`, and `string` functions may now contain `try/catch/finally` blocks that handle an exception and subsequently return normally. The compiler emits scoped closures for each block, preserving the request-local exception channel. Verified by `phix/tests/exception_typed_try_catch_http.sh` against PHP and Rust unit tests.

**Limitations:** This is not structured control-flow unwinding. `return` inside `try`, `catch`, or `finally` is not supported in typed functions; `break`/`continue` through `finally` and exception replacement across all nested paths are not verified. A default-value sentinel is still used for propagation from typed functions. Do not claim complete PHP exception compatibility.

### Compiler structured control flow (COMPAT-079, unfinished)

`phix/src/compiler/flow.rs` introduces a typed `PhpFlow<T, E>` with
`Normal`, `Return`, `Break`, `Continue`, and `Throw` outcomes. Its
`after_finally` method preserves pending control flow when `finally` completes
normally and replaces it when `finally` itself transfers control. The module
has six passing Rust unit tests. **It is not yet connected to code generation**:
compiled typed functions still use default-value sentinels for pending errors.
Do not rely on returns inside typed `try/catch/finally` until integration and
PHP differential tests are complete.

### COMPAT-079 continuation — typed function fallthrough (2026-11-19)

Implemented: generated typed functions no longer append an unconditional `Default::default()` return. A conservative terminal-return check rejects function bodies without a final top-level `return` statement. This deliberately rejects some otherwise valid PHP programs (including throw-only functions) until structured unwinding is integrated. This is **not** complete PHP control-flow analysis. The existing exception-propagation sentinels in `parser.rs` are still present and must be removed through a genuine `PhpFlow` code-generation migration. Rust unit suite: 140 passed. Full HTTP differential suite: not run.

### COMPAT-079 follow-up: typed-function return validation

Phix now conservatively checks that a typed function ends with an unconditional top-level `return` statement. A `return` nested inside an `if` or `while` does not establish that the function always returns. This check intentionally does not implement full PHP control-flow analysis and can reject valid programs. The structured `PhpFlow` module remains a tested foundation; generated PHP execution has **not** migrated to it, and exception propagation still contains legacy default-value sentinels.

### COMPAT-079 — void function nonlocal return safety (2026-11-19)
The compiler now explicitly rejects `return` inside `try`, `catch`, or `finally` even in `void` functions. Previously the closure-based lowering could silently swallow the return. This is a safety/correctness guard, **not support** for PHP nonlocal returns. The required permanent fix remains lowering control flow through `PhpFlow` instead of closures and default-value sentinels. Three Rust regression tests cover this guard.

### COMPAT-079 continued: loop-control diagnostics (2026-11-19)
The compiler now explicitly diagnoses unsupported `break` and `continue` statements rather than producing a misleading generic expression parse error. This is a safety/diagnostics improvement, **not** implementation of loop-control unwinding. Native `PhpFlow` integration, removal of exception sentinels, and `return` through `finally` remain pending. Two compiler regression tests were added.

### Structured control flow (COMPAT-079, work in progress)
The compiler's `flow.rs` contains `PhpFlow<T,E>` variants for normal completion, return, break, continue and throw. Its `and_then` operation sequences only normal outcomes; `catch_throw` handles only thrown outcomes; `after_finally` applies PHP-style overriding of pending transfers. These are currently unit-tested compiler foundations and **are not wired into generated applications**. Returns inside try/catch/finally, full loop-control unwinding, and removal of old exception sentinels remain pending.

### COMPAT-080 — native control-flow analysis IR (partial)

The compiler has a dedicated `compiler/ir.rs` control-flow graph representation with
explicit return, throw, branch, successor, and fallthrough terminators. Typed-function
return validation now uses reachable-exit analysis of the lowered straight-line
function-body suffix instead of the previous token-only terminal-return check.
The analysis conservatively treats nested conditionals/loops as possible fallthrough.
Six IR unit tests passed; the full Rust unit suite passed (162 tests).

**Limitations:** This is a control-flow analysis IR, not yet the full typed expression
IR or native execution lowering. Generated `try/catch/finally` still uses the legacy
mechanism; exception unwinding and sentinel removal are not completed. The new
IR does not establish full PHP compatibility. No PHP differential tests were run
in this milestone.

### COMPAT-080 continuation — CFG structural validation

The compiler now validates the native analysis IR's entry and all successor edges before using it for typed-function fallthrough checks. Invalid control-flow graphs are compiler errors rather than silently ignored edges. Five Rust regression tests cover empty graphs, invalid entries, invalid successors, and valid cycles. This does **not** implement expression lowering, exception edges, or structured unwinding in generated programs; the previous exception propagation mechanism remains.

### COMPAT-080 conditional CFG lowering (2026-11-19)
The native IR's return-path analysis now represents braced `if/else` and nested
conditional branches as explicit graph edges. The parser now emits an `else`
block for supported `if` statements. Conditional typed-function return-path
validation has compiler-level tests. This is not complete PHP conditional
syntax (notably `elseif` and unbraced branches), nor full native control-flow
lowering. Existing runtime-error default-value sentinels are still present and
must be replaced; this milestone does not claim structured unwinding.

### COMPAT-080: explicit throw edges in return-path IR (2026-11-19)

The native function control-flow analysis now lowers standalone `throw` statements to `Terminator::Throw` instead of treating them as opaque statements. The graph distinguishes unconditional throws, conditional throws, and throws inside potentially unexecuted loops. Four IR unit tests cover these cases. This change affects analysis of typed-function fallthrough; it does **not** implement exception-aware generated function results, `finally` unwinding, or removal of the existing default-value sentinels. A function ending in `throw` may still require further code generation changes before the generated Rust builds; no end-to-end PHP compatibility is claimed for that case.

### COMPAT-080: typed integer comparison IR (2026-11-19)

Native IR now lowers integer-to-integer equality, strict equality, inequality, and ordering comparisons into explicit typed expression nodes. Both operands are bound once in left-to-right order before native Rust comparison. This is integrated into the expression compiler, not a standalone helper. Other comparison categories still use the existing compiler paths; this does not establish PHP numeric or general comparison parity. The remaining exception sentinel unwinding and full expression IR migration are outstanding.

### COMPAT-080 — Native logical-expression IR (2026-11-19)

The compiler lowers supported statically typed boolean `&&` and `||` expressions through `TypedExpr::Logical`. Generated Rust evaluates the left operand once and evaluates the right operand lazily, preserving short-circuit side effects. This is a native code-generation path, not a PHP interpreter loop. Mixed-type PHP truthiness and complete exception-aware expression lowering remain unfinished. Rust unit suite: 187 passed (full PHP differential suite not run).

### Native IR conditional-chain validation (COMPAT-080 follow-up)
Braced `else if` branches now compile as nested conditionals, and typed-function fallthrough analysis follows each branch. The IR analyzer also recognizes `elseif`, but the executable parser does not yet support that spelling. This is partial support; no changes to structured exception unwinding have been made.

### COMPAT-080 — `elseif` conditional chains

Braced PHP `elseif` clauses are now accepted by the executable compiler, including mixed `else if` and `elseif` chains. Conditions are evaluated in order and only when preceding conditions were false. Typed-function return-path validation uses the native CFG analysis. Alternative colon-delimited PHP conditional syntax remains unsupported. The Rust test suite passed 196/196 tests; full HTTP/PHP differential testing has not been run for this milestone.

### COMPAT-080: call-effect control-flow analysis (2026-11-19)

The native control-flow IR conservatively models detected call expressions in
statements and returns as `Invoke { normal, unwind }`, with a distinct throwing
successor. A call statement can fall through after normal completion; a returned
call has either a return or exceptional exit. This analysis is wired into typed
function return-path validation. It does **not** yet lower runtime call results
or replace the existing generated-code exception sentinels. Full PHP
compatibility is not claimed.

### COMPAT-080: exception effects in conditional IR (2026-11-19)
The native analysis IR now models function calls in `if` and `elseif` conditions as `Invoke` with separate normal and exceptional successors. This preserves the fact that a condition can throw before selecting a branch. Three new Rust regression tests cover conditional invocation, missing `else`, and nested conditional chains. This is analysis-only: generated function execution still uses the old pending-error mechanism and is not yet structured unwinding.

### COMPAT-080 — Nested throw-expression effects in control-flow IR
The analysis IR now models `throw` tokens inside conditions and return expressions as potentially exceptional execution edges. This is conservative effect analysis, not complete expression lowering or executable structured unwinding. The compiler still contains legacy default-value exception propagation and full PHP parity is not claimed.

### COMPAT-080: generated typed-function body boundary (2026-11-19)

Generated typed Rust functions now terminate the emitted PHP function-body block as a statement rather than allowing it to become a Rust tail expression. This corrects Rust E0317 compilation errors when the last emitted expression is the pending-exception guard. Verified with the typed-function exception, throw-expression, and native Throwable identity compiled HTTP PHP differential scripts. This is a code-generation correctness fix, **not** structured exception unwinding: the existing default-value exception sentinels remain and must be replaced by explicit IR-driven outcomes.

### COMPAT-081 — script termination control-flow foundation
The native compiler flow model now includes `PhpFlow::Exit(PhpExit)` with status and message payload variants. Exit bypasses catch handlers, sequencing, loop boundaries, and `finally` (which must not be executed for an exit transfer). Four Rust unit tests cover these invariants. **This is a model-only foundation:** PHP `exit`/`die` parsing, argument conversion, generated code, shutdown handlers, and CLI/HTTP request lifecycle integration are not yet implemented or PHP-differential-tested. Do not claim executable PHP exit compatibility.

### COMPAT-081: typed-function echo/while guard correctness (2026-11-19)

The compiler now emits return-type-correct pending-error exits in `echo` expression evaluation and `while` condition evaluation. Previously both emitted `return;` unconditionally, producing invalid Rust in functions with non-void return types. Void functions retain their unit-return guards. Three Rust regression tests added; full Rust suite passes 226/226. This is a targeted code-generation fix, **not** structured exception lowering. Existing `Default::default()` propagation is still present, and compiled PHP differential tests were not run for this change.

### COMPAT-082: Function argument evaluation and exceptions

For native typed functions called with two or more arguments, the compiler now emits ordered argument evaluation with a pending-exception check between arguments. If an argument raises a PHP exception, later arguments and the function body are skipped. This applies to `void` and value-returning calls. This does not yet implement full PHP 8.x call semantics, arbitrary dynamically typed arguments, or explicit `Result`-based exception unwinding; value-returning calls still require replacement of default-value propagation placeholders.

### COMPAT-083 — Function-call exception guards

Compiled user-function calls now check pending exceptions consistently for zero, one, and multiple arguments. Calls with arguments evaluate each expression once, in left-to-right order, and skip later arguments and the callee when an earlier argument raises a runtime exception. The single-argument and void-call paths are covered by regression tests. Rust unit suite: 231 passed. This is not a complete structured exception implementation: default-value exception placeholders and `finally` control-flow semantics remain incomplete.

### COMPAT-084 — RHS exception guard before scalar assignment (2026-11-19)

Generated scalar variable initialization, reassignment, and integer `+=` now evaluate the right-hand side once, check the pending PHP runtime exception, and only then commit the new value. This avoids committing a placeholder result after a throwing call. Typed and void functions use the appropriate existing exception-exit guard. Three new code-generation regression tests cover initialization, reassignment, and integer `+=`; the Rust suite passes 234/234. This does **not** replace legacy `Default::default()` exception sentinels or implement structured `finally` unwinding. Compiled PHP differential tests were not run for this milestone.

### COMPAT-085 — Exception-aware logical short-circuit (2026-11-19)

Generated `&&` and `||` expressions now inspect the pending PHP exception immediately after evaluating the left operand, before deciding whether to evaluate the right operand. A thrown left operand cannot accidentally execute the right operand because of its temporary boolean value. Ordinary short-circuit behavior is preserved. Two compiler regression tests were added; 236 Rust tests pass. This is not yet full structured exception unwinding; the compiler still uses default-value exception sentinels elsewhere. No PHP differential suite was run for this milestone.

### COMPAT-086: Binary-expression exception ordering

The Rust source emitter now guards the right-hand side of string concatenation,
array union, and dynamic array comparison when evaluating the left-hand side
raises a pending PHP exception. Both operands are evaluated at most once in
source order. This is a targeted compatibility improvement; structured
exception results and complete PHP 8.x differential verification remain pending.

### COMPAT-088: Array read evaluation order
Generated PHP array-index reads now evaluate their array/base expression before the key expression and check for a pending exception after each evaluation. Nested indexing is guarded at every level. This is verified by Rust code-generation tests only; it does not yet replace default-value exception sentinels or implement structured exception unwinding.

### COMPAT-089 array write exception ordering
Array element writes (`$array[$key] = $value`), append writes (`$array[] = $value`), and array element union assignments (`+=`) evaluate keys before the right-hand side and guard pending exceptions before committing a mutation. This behavior is unit-tested at the generated-code level, not yet comprehensively differential-tested against PHP. Structured exception results are still planned.

### COMPAT-090: `unset()` evaluation order

Array-element `unset()` evaluates each dimension once in source order and checks for a pending exception before evaluating the next dimension or performing the mutation. This behavior is covered by compiler regression tests; full PHP 8.x compatibility is not claimed.

### COMPAT-091 — Throw constructor evaluation order

Supported built-in `throw new Exception(message, code, previous)` expressions and statements now evaluate constructor arguments sequentially, checking for pending exceptions after each expression. If evaluation fails, later constructor arguments and the outer exception construction are skipped. The same applies to supported built-in exception subclasses. This is a partial compatibility improvement; generated functions still use pending-exception guards and default-value propagation, and full `finally` semantics remain unimplemented.

### COMPAT-092: throw operand exception preservation

For `throw $expression`, Phix now evaluates the operand once and checks whether it raised an exception before attempting to throw the resulting value. The guard applies to standalone `throw` statements and `throw` expressions. Two Rust compiler regression tests cover emitted code; compiled PHP differential tests are still needed. The pending-error and default-value propagation mechanism remains in place and is not a substitute for planned structured exception outcomes.

### COMPAT-094: lazy structured finally transfer (foundation)
The compiler's `PhpFlow<T,E>` model now provides `with_finally`, which evaluates
its finally-body closure only for transfers that enter PHP finally. `exit`/`die`
bypass finally without evaluating the closure; a normal finally preserves the
pending return/throw, and a nonlocal transfer from finally replaces it.
Three Rust unit tests verify these cases. This is an IR/control-flow helper,
**not yet executable lowering for generated PHP try/catch/finally**.

### COMPAT-095: Terminal typed-function code generation

For a non-void function, the compiler validates terminal control flow with the native IR before emitting Rust. Its generated Rust now has an explicit unreachable terminal boundary after the function-body statement. This allows functions that only throw (and mixed throw/return branches) to compile without synthesizing a successful return at the function boundary. The unreachable branch indicates a compiler invariant; it is not a PHP return value or an exception transport mechanism. The current generated statements still use pending runtime errors and `Default::default()` for exception propagation; full structured unwinding remains pending. Verified with 258 Rust unit tests and a native build of a throw-only typed function; full PHP differential coverage has not been run.

### COMPAT-096 structured function outcome foundation

The internal `PhpFlow<T,E>::into_function_result()` helper converts a completed
`Return(value)` to `Ok(value)` and `Throw(error)` to `Err(error)`. Other
control-flow transfers are rejected at this boundary; `Exit` must be handled
at the request level. This helper is **not yet wired into executable code
emission**. Current compiled PHP functions still use pending-error guards and
default-value placeholders. Two Rust tests cover the new helper; the complete
Rust suite passed (260 tests). No PHP differential execution was run.

### COMPAT-097 — Composed structured try/catch/finally (internal only)

`PhpFlow::try_catch_finally` now composes catch and lazy finally transfers
in PHP execution order. Catch handles only exceptions from the try body;
finally can override a return or exception, and exit/die bypasses both
callbacks. Three Rust regression tests cover the composition. **Not yet
connected to emitted PHP function code**: generated Rust still uses pending
exception state and default-value sentinels. This does not establish PHP
8.x compatibility; executable lowering and PHP differential tests remain.

### COMPAT-098: Array union assignment evaluation

The compiler now checks for an exception after evaluating the right-hand side
of dynamically handled `$variable += expression` before reading or changing
the left-hand side. This is a targeted compiler correctness fix, not complete
PHP array compatibility or structured exception propagation.

### COMPAT-099 — Built-in SPL exception ancestry (verified subset)

`throw new` and `catch` now accept the built-in exception classes `RuntimeException`, `LogicException`, `InvalidArgumentException`, `DomainException`, `LengthException`, `OutOfRangeException`, `BadFunctionCallException`, `BadMethodCallException`, `OutOfBoundsException`, `OverflowException`, `UnderflowException`, `RangeException`, and `UnexpectedValueException`, in addition to the previously supported core exception classes. Catch matching traverses the built-in PHP exception parent hierarchy, preserving the distinction between `Exception` and `Error`. The differential script `phix/tests/spl_exception_inheritance_parity.sh` compares five cases against installed PHP and a compiled Phix HTTP application. User-defined subclasses, general object construction, and complete exception unwinding remain unsupported.

### COMPAT-100: Exception constructors without parentheses

Phix supports PHP's built-in exception constructor shorthand, for example `throw new RuntimeException;`, as well as `throw new RuntimeException();`. The shorthand works in standalone throw statements and throw expressions. Only the currently supported built-in Throwable classes are accepted; user-defined classes remain outside this verified subset. Run `bash phix/tests/exception_constructor_shorthand_parity.sh` after building Phix to compare compiled HTTP behavior with PHP. This change does not replace pending-exception sentinels or implement structured unwinding.

### COMPAT-101: Partial null-coalescing operator support
`$value ?? $fallback` is supported when `$value` is an already-defined dynamic (`Json`) value. It evaluates `$value` once and evaluates `$fallback` only if the result is `null`. Chained coalescing is right-associative. For now, undefined variables/offsets, properties, mixed-type results, and `??=` are not covered; this is **not** complete PHP `??` compatibility. Compiler tests cover lazy generation, associativity and exception preservation. Structured `Result`-based generated function unwinding is still pending.

### COMPAT-102 — Undefined simple-variable null coalescing

A simple undefined variable used as the left operand of `??` is treated as `null`, allowing the fallback to be evaluated lazily without an undefined-variable diagnostic. Ordinary undefined variable reads remain rejected. This does not implement missing array-offset/property coalescing or `??=`. Two compiler regression tests cover the distinction; Rust unit suite: 274 passing tests. PHP differential tests were not run for this milestone.

### COMPAT-103 — Initial ternary operators

The compiler accepts standard `$condition ? $if_true : $if_false` and shorthand `$value ?: $fallback` ternaries. The condition is evaluated once, branches are lazy, and PHP truthiness is applied for supported scalar and dynamic types. Branches with matching static types preserve that type; mixed scalar/dynamic branches are converted to `Json`. Parenthesized nested ternaries are supported; ambiguous unparenthesized nesting is rejected. This remains a limited typed subset: full PHP precedence interactions, dynamic object semantics, and comprehensive compiled PHP differential parity are not yet established. Generated code still uses the existing pending-exception/sentinel mechanism. Rust regression suite: 277 passing tests.

### COMPAT-104 — Simple-variable null-coalescing assignment

`$variable ??= $fallback` is supported for simple variables. Undefined variables are initialized to a dynamic null slot, while existing integer, boolean, string, or dynamic variables are represented as dynamic slots for subsequent operations. The right-hand side is evaluated only when the existing value is null, and a pending exception prevents committing the assignment. Array-offset/property `??=` and full PHP type/precedence parity are not implemented. Two Rust regression tests added (279 passing total); compiled PHP differential tests not run. Generated exception handling still uses pending-error sentinels.

### COMPAT-105 — Nested array-offset null-coalescing assignment (partial)

Phix now compiles `$array['key'] ??= $fallback` and nested forms such as `$array['parent']['child'] ??= $fallback` for existing dynamic array variables. Keys are evaluated once, in order; the existing value is checked before the fallback is evaluated; and the array is only modified after successful fallback evaluation. An append target (`$array[] ??= ...`) is rejected. The `??` operator also accepts supported statically typed scalar left operands through conversion to a dynamic PHP value. This is a targeted implementation; object properties, general undefined-offset diagnostic parity, PHP references, and comprehensive end-to-end differential validation remain incomplete. Rust suite: 282 tests passed. Existing exception sentinels remain.

### COMPAT-109: Direct typed function outcomes

Non-void generated functions now return `Result<T, Arc<PhixRuntimeError>>` directly, using `Ok(value)` for successful returns and `Err(error)` for pending exceptions detected at statement boundaries. The previous typed-function wrapper closure and post-hoc conversion are removed. This is a partial migration: expression-level placeholders, function-call adapters, and void-function exception handling still use the legacy pending-error mechanism. Complete PHP exception and finally compatibility is **not** claimed.

### Structured exception propagation migration status (COMPAT-110 audit)

Typed functions now have `Result` return boundaries, but expression-level exception propagation still relies on pending-error checks and default-value sentinels. Do not assume `Result` support is complete. A subsequent lowering pass must represent normal and exceptional expression outcomes explicitly across typed functions, top-level handlers, and `try`/`catch`/`finally` without returning from the wrong closure. The supported exception catch-class match list has been deduplicated (no behavioral expansion). All 291 Rust unit tests pass; PHP differential tests were not run for this audit.

### COMPAT-111 — direct typed function outcome forwarding
For a typed function whose return expression is exactly a zero-argument user-function call with the same declared return type, generated Rust directly returns the callee's `Result<T, Arc<PhixRuntimeError>>`. This avoids the pending-error adapter and default-value placeholder on that path. Argument-bearing calls, nested expressions, void functions, and try/finally remain on the existing transitional lowering and are **not** claimed to have full structured propagation. Two compiler tests cover the direct and non-direct paths; the Rust test suite passes (293 tests). PHP differential verification has not been run for this milestone.

### COMPAT-112 — direct typed Result forwarding with arguments

When a non-void typed function directly returns a call to another typed function with the same return type, the compiler now forwards the callee's `Result` without unwrapping it into a placeholder value, including calls with arguments. Argument expressions are evaluated once in source order and checked for a pending exception before invoking the callee. Nested argument expressions still use legacy expression-level adapters. This is partial structured outcome integration, not full PHP exception compatibility.

### COMPAT-113: Structured expression outcome foundation

`phix/src/compiler/flow.rs::expression_outcome` now defines typed `Result<T,E>` evaluation for ordered binary operands, nested expressions, conditional branches, and null-coalescing fallbacks. Failed operands stop subsequent evaluation without producing `Default::default()` values. Three Rust tests verify propagation and lazy behavior. **This API is not yet connected to generated PHP expressions; existing generated adapters remain.**

### COMPAT-114 — Direct nested function outcome forwarding

In a typed function's direct-return call, a complete zero-argument typed function
call used as an argument is now lowered as `fn_inner(ctx)?` instead of using the
pending-exception/default-value adapter. The outer call forwards its `Result`
unchanged. This applies only when the argument consists solely of the inner call;
other nested expressions and other contexts still use transitional lowering.

### COMPAT-115 — Structural expression lowering (in progress)

The typed compiler IR now represents lazy conditional and null-coalescing branches as structural nodes. The `??` parser path delegates emission to the coalescing IR node, with tests for source-order and lazy fallback. This is an incremental migration: most expressions are still directly emitted by the parser, and expression-level pending-error adapters and `Default::default()` exception placeholders remain. Do not treat this milestone as full structured-outcome integration.

### COMPAT-116 — compositional Result expression lowering (internal)
The typed expression IR now exposes `emit_result_rust` for nested arithmetic,
comparisons, logical operations, conditional expressions, and null coalescing.
Each leaf is expected to produce `Result<T, E>`. The emitter composes `and_then`
and `map` so errors short-circuit evaluation without fabricated values or a
pending-error flag; conditional, logical and coalescing branches remain lazy.
This is an internal compiler facility, **not** yet the default parser codegen
path. Existing `emit_rust` and expression adapters remain in use. Complete
PHP compatibility, references, exception/finally precedence, and end-to-end
PHP differential testing remain outstanding.

### COMPAT-117 — Structural expression AST (compiler foundation)

`phix/src/compiler/ast.rs` introduces a source-level expression tree and
Pratt parser for nested calls, operators, PHP 8 ternaries, coalescing, array
literals, indexing, property access, and assignments. Its four unit tests
cover nested call composition, conditional/coalescing syntax, indexed array
assignments, and rejection of ambiguous unparenthesized ternaries.

**Implementation status:** AST construction is tested but is not yet connected
to the legacy parser's executable code-generation path. In particular, this
milestone does not remove `Default::default()` exception placeholders or
establish complete PHP 8.x expression compatibility. The AST is not a full
PHP grammar; additional expression constructs and semantic lowering remain.

### COMPAT-118: structural expression semantic analysis

The expression AST now exposes recursive `validate()` and `effects()` passes. Assignment targets are checked structurally, including nested indexed variables, property assignments and array append assignments. Effect summaries conservatively report potential exception propagation, calls and writes throughout nested expressions. These passes are currently tested AST facilities, **not yet integrated with executable Rust generation**. Existing parser-driven emission and pending-exception adapters remain; no claim of complete PHP expression compatibility is made.
