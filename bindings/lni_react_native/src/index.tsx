// Generated by uniffi-bindgen-react-native
import installer from './NativeLniReactNative';

// Register the rust crate with Hermes
// - the boolean flag ensures this loads exactly once, even if the JS
//   code is reloaded (e.g. during development with metro).
let rustInstalled = false;
if (!rustInstalled) {
  installer.installRustCrate();
  rustInstalled = true;
}

// Export the generated bindings to the app.
export * from './generated/lni_uniffi';

// Now import the bindings so we can:
// - intialize them
// - export them as namespaced objects as the default export.
import * as lni_uniffi from './generated/lni_uniffi';

// Initialize the generated bindings: mostly checksums, but also callbacks.
// - the boolean flag ensures this loads exactly once, even if the JS code
//   is reloaded (e.g. during development with metro).
let initialized = false;
if (!initialized) {
  lni_uniffi.default.initialize();
  initialized = true;
}

// Export the crates as individually namespaced objects.
export default {
  lni_uniffi,
};

