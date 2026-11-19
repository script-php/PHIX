#!/usr/bin/env bash
# COMPAT-075: security/semantic regression guard for the planned native Throwable model.
# This is a RED test until Throwable is represented by a dedicated runtime object.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
python3 - "$ROOT" <<'PY'
from pathlib import Path
import sys
root=Path(sys.argv[1]); runtime=(root/'runtime/web_runtime.rs').read_text(); json=(root/'runtime/json.rs').read_text()
checks={
 'Throwable has a dedicated runtime variant': 'Throwable(' in json.split('pub(crate) enum Json',1)[1].split('}',1)[0],
 'Throw conversion does not trust ordinary JSON class fields': 'value.get("class")?' not in runtime.split('fn phix_error_from_json',1)[1].split('fn phix_throw_builtin_with_previous',1)[0],
 'Caught exception does not serialize into an ordinary JSON object': 'Json::Object(vec![' not in runtime.split('fn phix_exception_json',1)[1].split('fn phix_exception_method_string',1)[0],
}
for name,ok in checks.items():print(('PASS' if ok else 'FAIL')+' '+name)
if not all(checks.values()):sys.exit(1)
PY
