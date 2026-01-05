#!/bin/bash

# Build script for generating Swift bindings using UniFFI
# 
# This script:
# 1. Builds the lni library with uniffi feature
# 2. Uses uniffi-bindgen to generate Swift bindings from the shared library
# 3. Optionally builds for iOS simulator targets
#
# Usage: ./build.sh [--release] [--ios]
#        ./build.sh --release --ios   # Build for iOS in release mode

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
EXAMPLE_DIR="$SCRIPT_DIR/example"

# Parse arguments
BUILD_TYPE="debug"
BUILD_IOS=false

for arg in "$@"; do
    case $arg in
        --release)
            BUILD_TYPE="release"
            ;;
        --ios)
            BUILD_IOS=true
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

# Build for iOS simulator targets if requested
if [ "$BUILD_IOS" = true ]; then
    echo ""
    echo "Building for iOS simulator targets..."
    
    # iOS Simulator targets (x86_64 for Intel Macs, aarch64 for Apple Silicon Macs)
    IOS_SIM_TARGETS=(
        "aarch64-apple-ios-sim"
        "x86_64-apple-ios"
    )
    
    # iOS device target (for real devices)
    IOS_DEVICE_TARGET="aarch64-apple-ios"
    
    # Create libs directory
    LIBS_DIR="$SCRIPT_DIR/libs"
    mkdir -p "$LIBS_DIR"
    
    # Build for iOS Simulator
    for target in "${IOS_SIM_TARGETS[@]}"; do
        echo "  Building for $target..."
        
        # Add target if not already installed
        rustup target add "$target" 2>/dev/null || true
        
        if [ "$BUILD_TYPE" == "release" ]; then
            cargo build --package lni --features uniffi --release --target "$target"
            cp "$ROOT_DIR/target/$target/release/liblni.a" "$LIBS_DIR/liblni-$target.a"
        else
            cargo build --package lni --features uniffi --target "$target"
            cp "$ROOT_DIR/target/$target/debug/liblni.a" "$LIBS_DIR/liblni-$target.a"
        fi
        echo "    Copied to $LIBS_DIR/liblni-$target.a"
    done
    
    # Create universal library for iOS Simulator (combining x86_64 and arm64)
    echo ""
    echo "Creating universal library for iOS Simulator..."
    lipo -create \
        "$LIBS_DIR/liblni-aarch64-apple-ios-sim.a" \
        "$LIBS_DIR/liblni-x86_64-apple-ios.a" \
        -output "$LIBS_DIR/liblni-ios-sim.a"
    echo "  Created $LIBS_DIR/liblni-ios-sim.a"
    
    # Build for iOS device
    echo ""
    echo "Building for iOS device ($IOS_DEVICE_TARGET)..."
    rustup target add "$IOS_DEVICE_TARGET" 2>/dev/null || true
    
    if [ "$BUILD_TYPE" == "release" ]; then
        cargo build --package lni --features uniffi --release --target "$IOS_DEVICE_TARGET"
        cp "$ROOT_DIR/target/$IOS_DEVICE_TARGET/release/liblni.a" "$LIBS_DIR/liblni-ios-device.a"
    else
        cargo build --package lni --features uniffi --target "$IOS_DEVICE_TARGET"
        cp "$ROOT_DIR/target/$IOS_DEVICE_TARGET/debug/liblni.a" "$LIBS_DIR/liblni-ios-device.a"
    fi
    echo "  Created $LIBS_DIR/liblni-ios-device.a"
    
    # Create headers directory for XCFramework
    HEADERS_DIR="$SCRIPT_DIR/include"
    mkdir -p "$HEADERS_DIR"
    
    # Note: Headers will be generated after running uniffi-bindgen below
    # The XCFramework creation needs to happen after bindings are generated
    echo ""
    echo "iOS builds complete!"
    echo "Note: Run the full build to generate Swift bindings and create XCFramework."
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
cargo build --package lni-swift-bindgen

# Create output directory
OUTPUT_DIR="$SCRIPT_DIR/Sources/LNI"
mkdir -p "$OUTPUT_DIR"

echo "Generating Swift bindings..."
cargo run --package lni-swift-bindgen -- generate --library "$LIB_FILE" --language swift --out-dir "$OUTPUT_DIR"

echo ""
echo "Swift bindings generated successfully in: $OUTPUT_DIR"
echo ""
echo "Generated files:"
ls -la "$OUTPUT_DIR"

# Create XCFramework if iOS builds were requested
if [ "$BUILD_IOS" = true ]; then
    LIBS_DIR="$SCRIPT_DIR/libs"
    HEADERS_DIR="$SCRIPT_DIR/include"
    
    # Copy headers for XCFramework
    mkdir -p "$HEADERS_DIR"
    cp "$OUTPUT_DIR/lniFFI.h" "$HEADERS_DIR/"
    cp "$OUTPUT_DIR/lniFFI.modulemap" "$HEADERS_DIR/module.modulemap"
    
    echo ""
    echo "Creating XCFramework..."
    XCFRAMEWORK_DIR="$SCRIPT_DIR/LNI.xcframework"
    rm -rf "$XCFRAMEWORK_DIR"
    
    xcodebuild -create-xcframework \
        -library "$LIBS_DIR/liblni-ios-sim.a" \
        -headers "$HEADERS_DIR" \
        -library "$LIBS_DIR/liblni-ios-device.a" \
        -headers "$HEADERS_DIR" \
        -output "$XCFRAMEWORK_DIR"
    
    echo "  Created $XCFRAMEWORK_DIR"
    echo ""
    echo "XCFramework created successfully!"
    echo "To use in your iOS project:"
    echo "  1. Drag LNI.xcframework into your Xcode project"
    echo "  2. Add Sources/LNI/lni.swift to your project"
    echo "  3. Import and use the LNI types"
fi
