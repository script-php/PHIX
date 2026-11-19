# PHP-style whitespace and formatting compatibility

Phix does not mandate indentation or whitespace layout. The lexer treats ASCII
whitespace between tokens as insignificant and supports PHP `//`, `#`, and
`/*...*/` comments. Source may use tabs, LF or CRLF newlines, newlines around
operators, multiple statements on a single line, and braces on separate lines.

Whitespace *inside* quoted strings is preserved and affects output. Splitting a
variable name or multi-character operator changes the token sequence and must
not be interpreted as equivalent formatting.

## Verification

- `cargo test --offline`: lexer-level token equivalence, comments, strings and
  lexical boundaries, along with existing tests.
- `bash tests/whitespace_compat.sh`: compiles four layout variants of the same
  PHP-style website (formatted, minified, unusual multiline, tabs/CRLF), uses
  a literal `require_once` module, starts the resulting executable, and compares
  the returned HTTP response byte-for-byte against the expected response.
- Existing suites: `parity.sh`, `config_deploy.sh`, `migration_regression.sh`,
  `binary_assets.sh`, `php_web_compat.sh`.

This is **not** a claim that Phix supports the full PHP grammar. It establishes
formatting invariance for the language subset currently implemented. Future
parser changes should add corresponding formatting-equivalence tests.
