#!/bin/bash

# Build script for generating Kotlin bindings using UniFFI
# 
# This script:
# 1. Builds the lni library with uniffi feature
# 2. Uses uniffi-bindgen to generate Kotlin bindings from the shared library
# 3. Builds for Android targets using cargo-ndk (default, use --no-android to skip)
#
# Prerequisites:
# - cargo-ndk: cargo install cargo-ndk
# - Android NDK: Set ANDROID_NDK_HOME environment variable
# - Rust targets: rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android
#
# Usage: ./build.sh [--release] [--no-android]
#        ./build.sh --release             # Build for Android in release mode
#        ./build.sh --release --no-android # Skip Android builds

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
EXAMPLE_DIR="$SCRIPT_DIR/example"

# Parse arguments
BUILD_TYPE="debug"
BUILD_ANDROID=true

for arg in "$@"; do
    case $arg in
        --release)
            BUILD_TYPE="release"
            ;;
        --no-android)
            BUILD_ANDROID=false
            ;;
    esac
done

# Check for cargo-ndk if building for Android
if [ "$BUILD_ANDROID" = true ]; then
    if ! command -v cargo-ndk &> /dev/null; then
        echo "Error: cargo-ndk is required for Android builds."
        echo "Install it with: cargo install cargo-ndk"
        echo "Or skip Android builds with: ./build.sh --no-android"
        exit 1
    fi
    
    if [ -z "$ANDROID_NDK_HOME" ]; then
        echo "Warning: ANDROID_NDK_HOME is not set."
        echo "Attempting to find NDK automatically..."
        
        # Try common NDK locations
        if [ -d "$HOME/Library/Android/sdk/ndk" ]; then
            # Find the latest NDK version
            ANDROID_NDK_HOME=$(find "$HOME/Library/Android/sdk/ndk" -maxdepth 1 -type d | sort -V | tail -1)
        elif [ -d "$HOME/Android/Sdk/ndk" ]; then
            ANDROID_NDK_HOME=$(find "$HOME/Android/Sdk/ndk" -maxdepth 1 -type d | sort -V | tail -1)
        fi
        
        if [ -n "$ANDROID_NDK_HOME" ] && [ -d "$ANDROID_NDK_HOME" ]; then
            echo "Found NDK at: $ANDROID_NDK_HOME"
            export ANDROID_NDK_HOME
        else
            echo "Error: Could not find Android NDK. Please set ANDROID_NDK_HOME."
            exit 1
        fi
    fi
fi

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
    echo "Building for Android targets using cargo-ndk..."
    
    # Ensure Android targets are installed
    echo "Ensuring Rust Android targets are installed..."
    rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android 2>/dev/null || true
    
    # Create jniLibs directory
    JNILIBS_DIR="$EXAMPLE_DIR/app/src/main/jniLibs"
    mkdir -p "$JNILIBS_DIR"
    
    cd "$ROOT_DIR"
    
    # Use cargo-ndk to build for all Android targets
    # cargo-ndk automatically handles:
    # - Setting up correct linker
    # - Linking libc++_shared.so
    # - Avoiding OpenSSL cross-compilation issues (when using rustls-tls)
    if [ "$BUILD_TYPE" == "release" ]; then
        echo "Building release for Android targets..."
        cargo ndk \
            -t aarch64-linux-android \
            -t armv7-linux-androideabi \
            -t x86_64-linux-android \
            -t i686-linux-android \
            -o "$JNILIBS_DIR" \
            build --package lni --features uniffi --release
    else
        echo "Building debug for Android targets..."
        cargo ndk \
            -t aarch64-linux-android \
            -t armv7-linux-androideabi \
            -t x86_64-linux-android \
            -t i686-linux-android \
            -o "$JNILIBS_DIR" \
            build --package lni --features uniffi
    fi
    
    echo "Android builds complete!"
    echo "Libraries copied to: $JNILIBS_DIR"
    ls -la "$JNILIBS_DIR"
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

if [ "$BUILD_ANDROID" = true ]; then
    echo ""
    echo "============================================================"
    echo "IMPORTANT: After updating native libraries, you may need to"
    echo "invalidate Android Studio caches:"
    echo ""
    echo "  File → Invalidate Caches → Invalidate and Restart"
    echo "============================================================"
fi
