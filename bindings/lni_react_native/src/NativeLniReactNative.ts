// Generated by uniffi-bindgen-react-native
import type { TurboModule } from 'react-native';
import { TurboModuleRegistry } from 'react-native';

export interface Spec extends TurboModule {
  installRustCrate(): boolean;
  cleanupRustCrate(): boolean;
}

export default TurboModuleRegistry.getEnforcing<Spec>('LniReactNative');