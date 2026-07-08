#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PRETTIER_VERSION="${PRETTIER_VERSION:-3.7.4}"
MODE="--write"

if [[ "${1:-}" == "--check" ]]; then
  MODE="--check"
  shift
elif [[ "${1:-}" == "--write" ]]; then
  shift
fi

if ! command -v npm >/dev/null 2>&1; then
  echo "npm is required to run the TypeScript formatter." >&2
  exit 1
fi

cd "$ROOT"

prettier=(
  npm exec
  --yes
  --package "prettier@$PRETTIER_VERSION"
  --
  prettier
  "$MODE"
  --ignore-unknown
  --ignore-path "$ROOT/.prettierignore"
)

if [[ "$#" -gt 0 ]]; then
  "${prettier[@]}" "$@"
else
  "${prettier[@]}" \
    "bindings/typescript/**/*.{ts,tsx,js,jsx,mjs,cjs,json,md,yml,yaml}" \
    "bindings/typescript-arkade/**/*.{ts,tsx,js,jsx,mjs,cjs,json,md,yml,yaml}" \
    "bindings/typescript-spark/**/*.{ts,tsx,js,jsx,mjs,cjs,json,md,yml,yaml}"
fi
