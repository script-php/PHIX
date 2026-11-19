# Phix project routing — Milestone 17

The project root `phix.json` controls **compile-time** routing. It is embedded in the generated executable. `config.json` next to the executable still controls the HTTP port and bind address and is preserved on rebuild.

```json
{
  "entry": "public/index.php",
  "routing": {
    "document_root": "public",
    "deny": ["/includes/**", "/classes/**", "/vendor/**", "/storage/**"],
    "rewrites": [{"from":"/user/:id", "to":"/profile.php?id=:id"}],
    "redirects": [{"from":"/old", "to":"/new", "status":301}],
    "fallback": "/index.php"
  }
}
```

## Behavior

- Without `routing`, `document_root` defaults to `.`. Files such as `/login.php` and `/login.phx` in the public root compile as independent HTTP handlers. `/` selects the available index/main PHP/Phix handler.
- With `routing.document_root: "public"`, only files under `public/` can be requested directly. Other files can still be included at compile time, but are not HTTP routes.
- The compiler recursively discovers PHP and PHX entrypoints beneath the document root (max 256). Modules in denied folders are excluded from public discovery.
- Denied paths use exact matches or a directory prefix with `/**`. A single `:parameter` segment is supported by `rewrites`/`redirects`. Arbitrary Apache rewrite directives or regular-expression patterns are not implemented.
- A rewrite maps a public URL to a compiled `.php` or `.phx` entrypoint. Query substitutions like `?id=:id` are supported. `$_SERVER['REQUEST_URI']` keeps the browser URL; `$_SERVER['SCRIPT_NAME']` contains the selected PHP entrypoint.
- Redirects support statuses 301, 302 (default), 303, 307, 308. Targets are local absolute URL paths; external redirect destinations are intentionally disallowed.
- An optional `fallback` routes unmatched URLs to a compiled PHP/Phix entrypoint. Without it, unmatched paths return 404. Fallback is appropriate for framework front controllers.
- `config.json`, `phix.json`, dotfile paths and common key/certificate file extensions are always forbidden even when `deny` is empty. URL paths are decoded conservatively; dangerous encoded separators, path traversal, and malformed escapes produce 400.
- Compile-time validation rejects missing or denied rewrite/fallback entrypoints, invalid routing schemas and document roots outside the project. Source symlinks are not exposed as public entrypoints.
- Static assets still embed into the executable; only assets within `document_root` and not denied can be directly served. Runtime helpers such as `asset_text()` remain separate from HTTP public access.

## Known limitations

The HTTP server is still an early, single-threaded GET/HEAD-only implementation. It is not production-hardened and does not yet support PHP framework requirements such as POST, cookies, sessions, classes, Composer autoloading, dynamic includes, or complete PHP standard-library behavior. Routing does not imply those frameworks already run. Rebuild the executable to change routing rules.

Test with `bash tests/routing_integration.sh` and `cargo test`.
