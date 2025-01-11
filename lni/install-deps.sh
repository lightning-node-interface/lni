# Navigate to the project directory
cd /Users/nick/code/lni/lni

# Install the specified toolchain and targets
rustup show

# Install the specified toolchain and targets
rustup toolchain install stable

# Install the specified targets
rustup target add aarch64-apple-ios x86_64-apple-ios aarch64-apple-ios-sim armv7-linux-androideabi i686-linux-android aarch64-linux-android x86_64-linux-android x86_64-unknown-linux-gnu x86_64-apple-darwin aarch64-apple-darwin x86_64-pc-windows-gnu x86_64-pc-windows-msvc

# Install the specified components
rustup component add clippy rustfmt

# Verify the toolchain and targets
rustup show

cargo install -f wasm-pack
cargo install -f wasm-bindgen-cli
