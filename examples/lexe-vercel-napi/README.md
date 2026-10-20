# Lexe napi-rs Vercel probe

This small TypeScript Vercel Node.js app checks that the Linux napi-rs binary loads in a
deployed function, then optionally initializes Lexe and calls the node for an
unused on-chain address. The protected wallet probe returns only stage names;
the public address mode returns a receiving address, never credentials or node
information.

The Vercel project is `nick-theiles-projects/lni-lexe-napi-poc`.

Build `lni_napi_rs` for `x86_64-unknown-linux-gnu` using the locked workspace
dependencies and place the resulting binary at
`native/lni_js.linux-x64-gnu.node`. The binary must be built for the same
Linux ABI as Vercel's Node.js runtime. The deployed probe reports glibc 2.34;
building against a newer glibc will produce a binary that cannot load there.
The existing Spark dependency also needs `protoc` at build time. Do not commit
credentials or binaries.

Use a size-focused release build for the Vercel addon:

```sh
CARGO_PROFILE_RELEASE_OPT_LEVEL=z \
CARGO_PROFILE_RELEASE_LTO=true \
CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1 \
CARGO_PROFILE_RELEASE_PANIC=abort \
CARGO_PROFILE_RELEASE_STRIP=symbols \
cargo build --locked --offline -p lni_napi_rs --release
```

Build this inside a Linux x64 environment with `protoc` available. These
settings reduced the stripped addon from 35.3 MB to 19.1 MB (46%) without
removing any LNI exports. The `release` profile in `crates/lni/Cargo.toml` is
ignored for workspace builds because Cargo profiles must be declared at the
workspace root; the environment overrides above apply to this build only.

Open `/` to see a live Bitcoin receiving address. The public page calls
`/api/probe?mode=address`; only the address is returned, never the credential.
Call `/api/probe` to see the deployed Node platform and architecture, or
`/api/probe?mode=load` to verify that Node loaded the Rust addon. To test
Lexe's network path, set `LEXE_PROBE_TOKEN`, `LEXE_CLIENT_CREDENTIALS`, and
optionally `LEXE_NETWORK` in Vercel. Then call `/api/probe?mode=wallet` with the
token in the `x-probe-token` header. The credential needs permission to read
node info and receive an address. The function returns only `wallet_address`
on success.

Lexe's `get_next_unused_address` may return the same address until a payment
arrives, so this probe does not establish suitability for unique order addresses.
