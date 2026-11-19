# Phix

**Phix** is an experimental compiled programming language and web application runtime designed to make it possible to build native, standalone web applications using familiar PHP-style syntax.

Phix aims to preserve the approachable development style of PHP—its file structure, syntax, and URL conventions—while compiling an entire website project into an executable with an integrated HTTP server. Its compiler and runtime are written in Rust.

## Project goals

- Make PHP-like code convenient to write, without enforcing a particular indentation or formatting style.
- Compile website source code and public static resources into a standalone native application.
- Support traditional `.php` and `.phx` pages as well as front-controller routing for web frameworks.
- Keep deployment configuration and persistent uploads outside the executable, so rebuilding does not replace application data.
- Provide explicit configuration for public routes, protected directories, URL rewrites and upload permissions.
- Progressively expand PHP language and standard-library compatibility while maintaining a small, maintainable, secure architecture.

Phix is under active development and implements only a subset of PHP. It is **not** a drop-in replacement for PHP or a production-ready web server today.

For installation, build commands, supported syntax, functions, configuration, examples, limitations and future plans, see **[documentation.md](documentation.md)**.
