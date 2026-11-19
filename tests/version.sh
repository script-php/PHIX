#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --quiet
test "$(./target/debug/phix --version)" = "Phix v0.0.1"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
./target/debug/phix build examples/web -o "$work/app"
test "$("$work/app" --version)" = "Phix v0.0.1"
echo "Version consistency: PASS"
