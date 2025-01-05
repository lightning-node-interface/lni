cargo clean

### Wasm
# cargo build --target wasm32-unknown-unknown --release
# Build for bundler target (webpack)
# wasm-pack build --target bundler --out-dir pkg/bundler
# Build for nodejs target
# wasm-pack build --target nodejs --out-dir pkg/nodejs
# Build for web target (script tag)
# wasm-pack build --target web --out-dir pkg/web
