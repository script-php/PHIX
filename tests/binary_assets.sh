#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release
TMP=$(mktemp -d)
trap 'if [[ -n "${PID:-}" ]]; then kill "$PID" 2>/dev/null || :; wait "$PID" 2>/dev/null || :; fi; rm -rf "$TMP"' EXIT
./target/release/phix build examples/binary_web -o "$TMP/app"
"$TMP/app" & PID=$!
python3 - "$TMP" <<'PYTEST'
import urllib.request, pathlib, time, sys
for _ in range(60):
    try:
        data = urllib.request.urlopen('http://127.0.0.1:19518/', timeout=1).read()
        assert data == bytes([0,255,254,1,128,10,13,0]), data
        break
    except OSError:
        time.sleep(.1)
else: raise RuntimeError('Phix server did not start')
print('Binary-safe embedded output verified')
PYTEST
