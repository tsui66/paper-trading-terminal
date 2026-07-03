#!/usr/bin/env bash
# Resolve cargo-built paper binary path (Unix + Windows Git Bash).
resolve_paper_bin() {
  local profile="${1:-debug}"
  local base="./target/${profile}/paper"
  if [[ -f "${base}.exe" ]]; then
    echo "${base}.exe"
  elif [[ -f "$base" ]]; then
    echo "$base"
  else
    echo "error: paper binary not found under target/${profile}/" >&2
    return 1
  fi
}