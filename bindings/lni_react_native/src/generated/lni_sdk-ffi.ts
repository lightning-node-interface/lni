// This file was autogenerated by some hot garbage in the `uniffi-bindgen-react-native` crate.
// Trust me, you don't want to mess with it!

import {
  type StructuralEquality as UniffiStructuralEquality,
  type UniffiReferenceHolder,
  type UniffiRustArcPtr,
  type UniffiRustCallStatus,
  type UniffiRustFutureContinuationCallback as RuntimeUniffiRustFutureContinuationCallback,
} from 'uniffi-bindgen-react-native';

interface NativeModuleInterface {
  uniffi_internal_fn_func_ffi__string_to_byte_length(
    string: string,
    uniffi_out_err: UniffiRustCallStatus
  ): number;
  uniffi_internal_fn_func_ffi__string_to_arraybuffer(
    string: string,
    uniffi_out_err: UniffiRustCallStatus
  ): ArrayBuffer;
  uniffi_internal_fn_func_ffi__arraybuffer_to_string(
    buffer: ArrayBuffer,
    uniffi_out_err: UniffiRustCallStatus
  ): string;
  ffi_lni_uniffi_uniffi_contract_version(): number;
}

// Casting globalThis to any allows us to look for `NativeLniSdk`
// if it was added via JSI.
//
// We use a getter here rather than simply `globalThis.NativeLniSdk` so that
// if/when the startup sequence isn't just so, an empty value isn't inadvertantly cached.
const getter: () => NativeModuleInterface = () =>
  (globalThis as any).NativeLniSdk;
export default getter;

// Structs and function types for calling back into Typescript from Rust.
export type UniffiRustFutureContinuationCallback = (
  data: bigint,
  pollResult: number
) => void;
export type UniffiForeignFutureFree = (handle: bigint) => void;
export type UniffiCallbackInterfaceFree = (handle: bigint) => void;
export type UniffiForeignFuture = {
  handle: bigint;
  free: UniffiForeignFutureFree;
};
export type UniffiForeignFutureStructU8 = {
  returnValue: number;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteU8 = (
  callbackData: bigint,
  result: UniffiForeignFutureStructU8
) => void;
export type UniffiForeignFutureStructI8 = {
  returnValue: number;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteI8 = (
  callbackData: bigint,
  result: UniffiForeignFutureStructI8
) => void;
export type UniffiForeignFutureStructU16 = {
  returnValue: number;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteU16 = (
  callbackData: bigint,
  result: UniffiForeignFutureStructU16
) => void;
export type UniffiForeignFutureStructI16 = {
  returnValue: number;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteI16 = (
  callbackData: bigint,
  result: UniffiForeignFutureStructI16
) => void;
export type UniffiForeignFutureStructU32 = {
  returnValue: number;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteU32 = (
  callbackData: bigint,
  result: UniffiForeignFutureStructU32
) => void;
export type UniffiForeignFutureStructI32 = {
  returnValue: number;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteI32 = (
  callbackData: bigint,
  result: UniffiForeignFutureStructI32
) => void;
export type UniffiForeignFutureStructU64 = {
  returnValue: bigint;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteU64 = (
  callbackData: bigint,
  result: UniffiForeignFutureStructU64
) => void;
export type UniffiForeignFutureStructI64 = {
  returnValue: bigint;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteI64 = (
  callbackData: bigint,
  result: UniffiForeignFutureStructI64
) => void;
export type UniffiForeignFutureStructF32 = {
  returnValue: number;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteF32 = (
  callbackData: bigint,
  result: UniffiForeignFutureStructF32
) => void;
export type UniffiForeignFutureStructF64 = {
  returnValue: number;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteF64 = (
  callbackData: bigint,
  result: UniffiForeignFutureStructF64
) => void;
export type UniffiForeignFutureStructPointer = {
  returnValue: bigint;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompletePointer = (
  callbackData: bigint,
  result: UniffiForeignFutureStructPointer
) => void;
export type UniffiForeignFutureStructRustBuffer = {
  returnValue: ArrayBuffer;
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteRustBuffer = (
  callbackData: bigint,
  result: UniffiForeignFutureStructRustBuffer
) => void;
export type UniffiForeignFutureStructVoid = {
  callStatus: UniffiRustCallStatus;
};
export type UniffiForeignFutureCompleteVoid = (
  callbackData: bigint,
  result: UniffiForeignFutureStructVoid
) => void;

// UniffiRustFutureContinuationCallback is generated as part of the component interface's
// ffi_definitions. However, we need it in the runtime.
// We could:
// (a) do some complicated template logic to ensure the declaration is not generated here (possible)
// (b) import the generated declaration into the runtime (m a y b e) or…
// (c) generate the declaration anyway, and use a different declaration in the runtime.
//
// We chose (c) here as the simplest. In addition, we perform a compile time check that
// the two versions of `UniffiRustFutureContinuationCallback` are structurally equivalent.
//
// If you see the error:
// ```
// Type 'true' is not assignable to type 'false'.(2322)
// ```
// Then a new version of uniffi has changed the signature of the callback. Most likely, code in
// `typescript/src/async-rust-call.ts` will need to be changed.
//
// If you see the error:
// ```
// Cannot find name 'UniffiRustFutureContinuationCallback'. Did you mean 'RuntimeUniffiRustFutureContinuationCallback'?(2552)
// ```
// then you may not be using callbacks or promises, and uniffi is now not generating Futures and callbacks.
// You should not generate this if that is the case.
//
// ('You' being the bindings generator maintainer).
const isRustFutureContinuationCallbackTypeCompatible: UniffiStructuralEquality<
  RuntimeUniffiRustFutureContinuationCallback,
  UniffiRustFutureContinuationCallback
> = true;
