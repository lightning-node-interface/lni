#!/usr/bin/env zsh

cargo clean
cargo build

set -e
set -u

# NOTE: You MUST run this every time you make changes to the core. Unfortunately, calling this from Xcode directly
# does not work so well.

# In release mode, we create a ZIP archive of the xcframework and update Package.swift with the computed checksum.
# This is only needed when cutting a new release, not for local development.
release=false

for arg in "$@"
do
    case $arg in
        --release)
            release=true
            shift # Remove --release from processing
            ;;
        *)
            shift # Ignore other argument from processing
            ;;
    esac
done



generate_ffi() {
  echo "Generating framework module mapping and FFI bindings"
  # NOTE: Convention requires the modulemap be named module.modulemap
  cargo ndk build --release
  #mkdir -p ../andrpid/Sources/UniFFI/
  #mv target/uniffi-xcframework-staging/*.swift ../apple/Sources/UniFFI/
  #mv target/uniffi-xcframework-staging/module.modulemap ../apple/Sources/UniFFI/module.modulemap
}


basename=lni

cargo build -p $basename --lib --release --target aarch64-apple-darwin

generate_ffi $basename
