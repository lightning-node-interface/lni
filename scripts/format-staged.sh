#!/usr/bin/env bash
set -euo pipefail

ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

rust_changed=0
rust_files=()
typescript_files=()

while IFS= read -r -d '' file; do
  case "$file" in
    *.rs)
      rust_changed=1
      rust_files+=("$file")
      ;;
    bindings/typescript/* | bindings/typescript-arkade/* | bindings/typescript-spark/*)
      case "$file" in
        *.ts | *.tsx | *.js | *.jsx | *.mjs | *.cjs | *.json | *.md | *.yml | *.yaml)
          typescript_files+=("$file")
          ;;
      esac
      ;;
  esac
done < <(git diff --cached --name-only --diff-filter=ACMR -z)

if [[ "$rust_changed" -eq 0 && "${#typescript_files[@]}" -eq 0 ]]; then
  exit 0
fi

partial_files=()
for file in "${rust_files[@]}" "${typescript_files[@]}"; do
  if ! git diff --quiet -- "$file"; then
    partial_files+=("$file")
  fi
done

if [[ "${#partial_files[@]}" -gt 0 ]]; then
  echo "Refusing to autoformat files with both staged and unstaged changes:" >&2
  printf '  %s\n' "${partial_files[@]}" >&2
  echo "Stage or stash the unstaged hunks, then commit again." >&2
  exit 1
fi

if [[ "$rust_changed" -eq 1 ]]; then
  if ! command -v cargo >/dev/null 2>&1; then
    echo "cargo is required to format staged Rust files." >&2
    exit 1
  fi

  cargo fmt --all --manifest-path "$ROOT/Cargo.toml"
fi

if [[ "${#typescript_files[@]}" -gt 0 ]]; then
  "$ROOT/scripts/format-typescript.sh" --write "${typescript_files[@]}"
fi

for file in "${rust_files[@]}" "${typescript_files[@]}"; do
  if [[ -e "$file" ]]; then
    git add -- "$file"
  fi
done
