# bindings
cargo build --release
cargo run --release --features=uniffi/cli --bin uniffi-bindgen generate --library target/release/liblni.so --language kotlin --out-dir out
