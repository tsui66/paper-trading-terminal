#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "== cargo fmt =="
cargo fmt --check

echo "== cargo clippy =="
cargo clippy -- -D warnings

echo "== cargo test =="
cargo test

echo "== CLI integration =="
./scripts/test/test_cli.sh

echo "CI passed on $(uname -s 2>/dev/null || echo unknown)."