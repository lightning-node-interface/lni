#!/bin/bash

# Build script for generating Kotlin bindings using UniFFI
# 
# This script:
# 1. Builds the lni library with uniffi feature
# 2. Uses uniffi-bindgen to generate Kotlin bindings from the shared library
# 3. Optionally builds for Android targets
#
# Usage: ./build.sh [--release] [--android]
#        ./build.sh --release --android   # Build for Android in release mode

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
EXAMPLE_DIR="$SCRIPT_DIR/example"

# Parse arguments
BUILD_TYPE="debug"
BUILD_ANDROID=false

for arg in "$@"; do
    case $arg in
        --release)
            BUILD_TYPE="release"
            ;;
        --android)
            BUILD_ANDROID=true
            ;;
    esac
done

echo "Building LNI library with UniFFI feature ($BUILD_TYPE)..."

cd "$ROOT_DIR"

# Build for host platform (needed for uniffi-bindgen)
if [ "$BUILD_TYPE" == "release" ]; then
    cargo build --package lni --features uniffi --release
    LIB_PATH="$ROOT_DIR/target/release"
else
    cargo build --package lni --features uniffi
    LIB_PATH="$ROOT_DIR/target/debug"
fi

# Build for Android targets if requested
if [ "$BUILD_ANDROID" = true ]; then
    echo ""
    echo "Building for Android targets..."
    
    # Android target configurations: (rust_target, jni_dir)
    ANDROID_TARGETS=(
        "aarch64-linux-android:arm64-v8a"
        "armv7-linux-androideabi:armeabi-v7a"
        "x86_64-linux-android:x86_64"
        "i686-linux-android:x86"
    )
    
    # Create jniLibs directories
    JNILIBS_DIR="$EXAMPLE_DIR/app/src/main/jniLibs"
    
    for target_pair in "${ANDROID_TARGETS[@]}"; do
        RUST_TARGET="${target_pair%%:*}"
        JNI_DIR="${target_pair##*:}"
        
        echo "  Building for $RUST_TARGET..."
        
        if [ "$BUILD_TYPE" == "release" ]; then
            cargo build --package lni --features uniffi --release --target "$RUST_TARGET"
            ANDROID_LIB_PATH="$ROOT_DIR/target/$RUST_TARGET/release/liblni.so"
        else
            cargo build --package lni --features uniffi --target "$RUST_TARGET"
            ANDROID_LIB_PATH="$ROOT_DIR/target/$RUST_TARGET/debug/liblni.so"
        fi
        
        # Copy to jniLibs
        mkdir -p "$JNILIBS_DIR/$JNI_DIR"
        cp "$ANDROID_LIB_PATH" "$JNILIBS_DIR/$JNI_DIR/"
        echo "    Copied to $JNILIBS_DIR/$JNI_DIR/liblni.so"
    done
    
    echo "Android builds complete!"
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
