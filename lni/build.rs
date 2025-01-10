fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    uniffi::generate_scaffolding("./src/lni.udl").unwrap();
}
