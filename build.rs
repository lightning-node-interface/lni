fn main() {
	#[cfg(feature = "uniffi")]
	uniffi::generate_scaffolding("bindings/lni.udl").unwrap();
}