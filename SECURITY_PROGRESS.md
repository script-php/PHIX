# Security progress

Implemented and tested this iteration: bounded HTTP header reads (32 KiB), header count cap (100), strict HTTP/1.0/1.1 version validation, header-name/control-byte validation, HTTP/1.1 Host requirement, duplicate Host rejection, and Transfer-Encoding rejection. The existing 5-second read/write socket timeouts, upload opt-in, and existing upload path checks remain.

Not yet implemented: general filesystem capability model, upload quotas, concurrency controls, rate limiting, session/cookie security, SQL drivers, PDO/mysqli, SQLite storage provisioning, and production audit.

The planned mysqli, PDO and SQLite API is documented in documentation.md; no database support is claimed in this iteration.

## Subsequent resource-limits milestone

Implemented bounded per-connection worker concurrency (default 32), HTTP 503 overload rejection, configurable request body and header byte limits, and configurable socket I/O timeout. Config values are validated on deployment and at startup. This is not a complete denial-of-service defense: no global memory budget, total wall-clock request deadline, or per-IP limits yet. MySQLi/PDO/SQLite remain planned, not implemented.
