#!/usr/bin/env bash
# Verify checked-in Swift bindings against the current Rust UniFFI metadata.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
GENERATED_DIR="$(mktemp -d)"
trap 'rm -rf "$GENERATED_DIR"' EXIT

cd "$ROOT_DIR"
cargo build --locked --package lni --features uniffi
TARGET_DIR="$(cd "${CARGO_TARGET_DIR:-$ROOT_DIR/target}" && pwd)"
case "$(uname -s)" in
    Darwin) LIBRARY="$TARGET_DIR/debug/liblni.dylib" ;;
    Linux) LIBRARY="$TARGET_DIR/debug/liblni.so" ;;
    *) echo "Unsupported host for Swift binding drift check" >&2; exit 1 ;;
esac

cargo run --locked --package lni-swift-bindgen -- generate \
    --library "$LIBRARY" --language swift --out-dir "$GENERATED_DIR"

awk '{ sub(/[[:blank:]]+$/, ""); print }' "$GENERATED_DIR/lni.swift" > "$GENERATED_DIR/normalized.swift"
diff -u "$SCRIPT_DIR/Sources/LNI/lni.swift" "$GENERATED_DIR/normalized.swift"
diff -u "$SCRIPT_DIR/example/LNIExample/lni.swift" "$GENERATED_DIR/normalized.swift"
echo "Both checked-in Swift bindings match the current Rust library."


if command -v swiftc >/dev/null 2>&1; then
    swiftc -parse-as-library \
        -Xcc "-fmodule-map-file=$GENERATED_DIR/lniFFI.modulemap" \
        "$SCRIPT_DIR/Sources/LNI/lni.swift" "$SCRIPT_DIR/tests/OnchainRecordSmoke.swift" \
        -L "$TARGET_DIR/debug" -llni \
        -Xlinker -rpath -Xlinker "$TARGET_DIR/debug" \
        -o "$GENERATED_DIR/onchain-smoke"
    "$GENERATED_DIR/onchain-smoke"
else
    echo "Swift compiler unavailable; native serialization smoke test skipped."
fi
