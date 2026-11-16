fn main() {
    // The napi CLI collects declarations from this shared crate as well as the
    // Node binding. Re-emit them when a new CLI type-generation context starts.
    println!("cargo:rerun-if-env-changed=TYPE_DEF_TMP_PATH");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_NAPI_RS_CLI_VERSION");
}
