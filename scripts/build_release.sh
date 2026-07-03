#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="${1:-$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')}"
TARGET="${CARGO_BUILD_TARGET:-}"

echo "==> paper-trading-terminal release build v${VERSION}"

FEATURES="${PAPER_FEATURES:-${PPT_FEATURES:-}}"
if [[ -n "$FEATURES" ]]; then
  echo "    features: $FEATURES"
  cargo build --release --features "$FEATURES"
else
  cargo build --release
fi

BIN="target/release/paper"
if [[ -n "$TARGET" ]]; then
  BIN="target/${TARGET}/release/paper"
fi

mkdir -p dist
OUT="dist/paper-${VERSION}-$(uname -s | tr '[:upper:]' '[:lower:]')-$(uname -m)"
cp "$BIN" "$OUT"
chmod +x "$OUT"

if command -v shasum >/dev/null 2>&1; then
  shasum -a 256 "$OUT" | tee "${OUT}.sha256"
fi

echo "==> Binary: $OUT"
"$OUT" --version 2>/dev/null || true