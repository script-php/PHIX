#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMP="$(mktemp -d)"; trap 'rm -rf "$TMP"' EXIT
"$ROOT/target/debug/phix" build "$ROOT/examples/web" -o "$TMP/app" >/dev/null
cat > "$TMP/config.json" <<'JSON'
{"runtime":{"bind":"127.0.0.1","port":19577,"max_request_body":1024,"max_header_bytes":1024,"max_concurrent_requests":2,"io_timeout_seconds":2}}
JSON
"$TMP/app" >"$TMP/server.log" 2>&1 & PID=$!
trap 'kill "$PID" 2>/dev/null || true; rm -rf "$TMP"' EXIT
python3 - <<'PY'
import socket,time
for _ in range(60):
 try:
  s=socket.create_connection(('127.0.0.1',19577),timeout=.2);s.close();break
 except OSError:time.sleep(.05)
def req(data):
 s=socket.create_connection(('127.0.0.1',19577),timeout=3);s.sendall(data);r=s.recv(256);s.close();return r
assert b'413' in req(b'POST / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1025\r\n\r\n'), 'body limit'
assert b'431' in req(b'GET / HTTP/1.1\r\nHost: localhost\r\nX-Long: '+b'a'*1100+b'\r\n\r\n'), 'header limit'
print('runtime limits passed')
PY
