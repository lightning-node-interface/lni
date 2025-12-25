#!/bin/bash

# Build script for generating Kotlin bindings using UniFFI
# 
# This script:
# 1. Builds the lni library with uniffi feature
# 2. Uses uniffi-bindgen to generate Kotlin bindings from the shared library
#
# Usage: ./build.sh [--release]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

# Parse arguments
BUILD_TYPE="debug"
if [ "$1" == "--release" ]; then
    BUILD_TYPE="release"
fi

echo "Building LNI library with UniFFI feature ($BUILD_TYPE)..."

cd "$ROOT_DIR"

if [ "$BUILD_TYPE" == "release" ]; then
    cargo build --package lni --features uniffi --release
    LIB_PATH="$ROOT_DIR/target/release"
else
    cargo build --package lni --features uniffi
    LIB_PATH="$ROOT_DIR/target/debug"
fi

# Find the shared library (Linux: .so, macOS: .dylib)
if [ -f "$LIB_PATH/liblni.so" ]; then
    LIB_FILE="$LIB_PATH/liblni.so"
elif [ -f "$LIB_PATH/liblni.dylib" ]; then
    LIB_FILE="$LIB_PATH/liblni.dylib"
else
    echo "Error: Could not find liblni.so or liblni.dylib in $LIB_PATH"
    exit 1
fi

echo "Found library: $LIB_FILE"

# Build the uniffi-bindgen tool
echo "Building uniffi-bindgen..."
cargo build --package lni-kotlin-bindgen

# Create output directory
OUTPUT_DIR="$SCRIPT_DIR/src/main/kotlin"
mkdir -p "$OUTPUT_DIR"

echo "Generating Kotlin bindings..."
cargo run --package lni-kotlin-bindgen -- generate --library "$LIB_FILE" --language kotlin --out-dir "$OUTPUT_DIR"

echo ""
echo "Kotlin bindings generated successfully in: $OUTPUT_DIR"
echo ""
echo "Generated files:"
ls -la "$OUTPUT_DIR"
