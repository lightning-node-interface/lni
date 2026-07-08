#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required to run cargo fmt." >&2
  exit 1
fi

cargo fmt --all --manifest-path "$ROOT/Cargo.toml"
"$ROOT/scripts/format-typescript.sh" --write
