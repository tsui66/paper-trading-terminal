#!/usr/bin/env bash
# Package release archives for GitHub Releases (matches install / scoop / homebrew names).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="${1:-$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')}"
TAG="v${VERSION}"
PKG="paper-trading-terminal"
FEATURES="${PAPER_FEATURES:-}"

mkdir -p dist

package_tar() {
  local target=$1
  local name=$2
  local bin=target/${target}/release/paper

  if [[ ! -f "$bin" ]]; then
    echo "missing $bin — build first"
    return 1
  fi

  cp "$bin" "dist/paper"
  chmod +x dist/paper
  tar czvf "dist/${name}" -C dist paper
  rm -f dist/paper
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "dist/${name}" | tee "dist/${name}.sha256"
  fi
  echo "==> dist/${name}"
}

package_zip() {
  local target=$1
  local name=$2
  local bin=target/${target}/release/paper.exe

  if [[ ! -f "$bin" ]]; then
    echo "missing $bin — build first"
    return 1
  fi

  mkdir -p dist/win
  cp "$bin" dist/win/paper.exe
  (cd dist/win && zip -q "../${name}" paper.exe)
  rm -rf dist/win
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "dist/${name}" | tee "dist/${name}.sha256"
  fi
  echo "==> dist/${name}"
}

build_target() {
  local target=$1
  if [[ -n "$FEATURES" ]]; then
    cargo build --release --target "$target" --features "$FEATURES"
  else
    cargo build --release --target "$target"
  fi
}

echo "==> packaging paper-trading-terminal ${TAG}"

# Build native host binary when no cross targets specified.
if [[ $# -lt 2 ]]; then
  HOST_OS=$(uname -s | tr '[:upper:]' '[:lower:]')
  HOST_ARCH=$(uname -m)
  case "$HOST_ARCH" in
    x86_64) HOST_ARCH=amd64 ;;
    aarch64 | arm64) HOST_ARCH=arm64 ;;
  esac

  if [[ -n "$FEATURES" ]]; then
    cargo build --release --features "$FEATURES"
  else
    cargo build --release
  fi

  NAME="${PKG}-${HOST_OS}-${HOST_ARCH}.tar.gz"
  cp target/release/paper dist/paper
  chmod +x dist/paper
  tar czvf "dist/${NAME}" -C dist paper
  rm -f dist/paper
  shasum -a 256 "dist/${NAME}" 2>/dev/null | tee "dist/${NAME}.sha256" || true
  exit 0
fi

# Usage: ./scripts/package_release.sh [version] darwin-arm64 linux-amd64 ...
shift || true
for spec in "$@"; do
  case "$spec" in
    darwin-arm64)
      build_target aarch64-apple-darwin
      package_tar aarch64-apple-darwin "${PKG}-darwin-arm64.tar.gz"
      ;;
    darwin-amd64)
      build_target x86_64-apple-darwin
      package_tar x86_64-apple-darwin "${PKG}-darwin-amd64.tar.gz"
      ;;
    linux-amd64)
      build_target x86_64-unknown-linux-gnu
      package_tar x86_64-unknown-linux-gnu "${PKG}-linux-amd64.tar.gz"
      ;;
    linux-arm64)
      build_target aarch64-unknown-linux-gnu
      package_tar aarch64-unknown-linux-gnu "${PKG}-linux-arm64.tar.gz"
      ;;
    linux-musl-amd64)
      build_target x86_64-unknown-linux-musl
      package_tar x86_64-unknown-linux-musl "${PKG}-linux-musl-amd64.tar.gz"
      ;;
    windows-amd64)
      build_target x86_64-pc-windows-msvc
      package_zip x86_64-pc-windows-msvc "${PKG}-windows-amd64.zip"
      ;;
    *)
      echo "unknown target spec: $spec" >&2
      exit 1
      ;;
  esac
done