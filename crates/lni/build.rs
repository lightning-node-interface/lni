fn main() {
    // The shared crate also emits a cdylib; its Node-API symbols need the same
    // platform linker setup as the final Node addon.
    #[cfg(feature = "napi_rs")]
    napi_build::setup();

    // The napi CLI collects declarations from this shared crate as well as the
    // Node binding. Re-emit them when a new CLI type-generation context starts.
    println!("cargo:rerun-if-env-changed=TYPE_DEF_TMP_PATH");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_NAPI_RS_CLI_VERSION");
}
