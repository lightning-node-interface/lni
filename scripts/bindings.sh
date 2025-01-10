# bindings
cd lni
cargo build --release

### Kotlin
# Generate Kotlin bindings using uniffi-bindgen
cargo run --bin uniffi-bindgen generate --library ../target/release/liblni.dylib --language kotlin --out-dir ../lni-kotlin/lib/src/main/kotlin/ --no-format
# Copy the binary to the Android resources directory
mkdir -p ../lni-kotlin/lib/src/main/resources/
cp ../target/release/liblni.dylib ../lni-kotlin/lib/src/main/resources/liblni.dylib

### Swift
mkdir -p ../lni-swift/Sources/Lni/include/
cargo run --bin uniffi-bindgen generate --library ../target/release/liblni.dylib --language swift --out-dir ../lni-swift/Sources/Lni/ --no-format
cp ../target/release/liblni.dylib ../lni-swift/Sources/Lni/include/liblni.dylib