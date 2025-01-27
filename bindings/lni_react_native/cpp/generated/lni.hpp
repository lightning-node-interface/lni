// This file was autogenerated by some hot garbage in the
// `uniffi-bindgen-react-native` crate. Trust me, you don't want to mess with
// it!
#pragma once
#include "UniffiCallInvoker.h"
#include <ReactCommon/CallInvoker.h>
#include <iostream>
#include <jsi/jsi.h>
#include <map>
#include <memory>

namespace react = facebook::react;
namespace jsi = facebook::jsi;

class NativeLni : public jsi::HostObject {
private:
  // For calling back into JS from Rust.
  std::shared_ptr<uniffi_runtime::UniffiCallInvoker> callInvoker;

protected:
  std::map<std::string, jsi::Value> props;
  jsi::Value cpp_uniffi_internal_fn_func_ffi__string_to_byte_length(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_internal_fn_func_ffi__string_to_arraybuffer(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_internal_fn_func_ffi__arraybuffer_to_string(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_fn_clone_fetcher(jsi::Runtime &rt,
                                                    const jsi::Value &thisVal,
                                                    const jsi::Value *args,
                                                    size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_fn_free_fetcher(jsi::Runtime &rt,
                                                   const jsi::Value &thisVal,
                                                   const jsi::Value *args,
                                                   size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_fn_constructor_fetcher_new(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_fn_method_fetcher_get_config(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_fn_method_fetcher_get_ip_address(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_fn_clone_phoenixservice(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_fn_free_phoenixservice(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_fn_constructor_phoenixservice_new(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_fn_method_phoenixservice_get_info(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_u8(jsi::Runtime &rt,
                                                    const jsi::Value &thisVal,
                                                    const jsi::Value *args,
                                                    size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_u8(jsi::Runtime &rt,
                                                      const jsi::Value &thisVal,
                                                      const jsi::Value *args,
                                                      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_u8(jsi::Runtime &rt,
                                                    const jsi::Value &thisVal,
                                                    const jsi::Value *args,
                                                    size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_u8(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_i8(jsi::Runtime &rt,
                                                    const jsi::Value &thisVal,
                                                    const jsi::Value *args,
                                                    size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_i8(jsi::Runtime &rt,
                                                      const jsi::Value &thisVal,
                                                      const jsi::Value *args,
                                                      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_i8(jsi::Runtime &rt,
                                                    const jsi::Value &thisVal,
                                                    const jsi::Value *args,
                                                    size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_i8(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_u16(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_u16(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_u16(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_u16(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_i16(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_i16(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_i16(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_i16(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_u32(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_u32(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_u32(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_u32(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_i32(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_i32(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_i32(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_i32(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_u64(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_u64(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_u64(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_u64(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_i64(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_i64(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_i64(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_i64(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_f32(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_f32(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_f32(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_f32(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_f64(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_f64(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_f64(jsi::Runtime &rt,
                                                     const jsi::Value &thisVal,
                                                     const jsi::Value *args,
                                                     size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_f64(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_pointer(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_pointer(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_pointer(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_pointer(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_rust_buffer(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_rust_buffer(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_rust_buffer(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_rust_buffer(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_poll_void(jsi::Runtime &rt,
                                                      const jsi::Value &thisVal,
                                                      const jsi::Value *args,
                                                      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_cancel_void(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_free_void(jsi::Runtime &rt,
                                                      const jsi::Value &thisVal,
                                                      const jsi::Value *args,
                                                      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_rust_future_complete_void(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_checksum_method_fetcher_get_config(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_checksum_method_fetcher_get_ip_address(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_checksum_method_phoenixservice_get_info(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_checksum_constructor_fetcher_new(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_lni_uniffi_checksum_constructor_phoenixservice_new(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_ffi_lni_uniffi_uniffi_contract_version(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_internal_fn_method_fetcher_ffi__bless_pointer(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);
  jsi::Value cpp_uniffi_internal_fn_method_phoenixservice_ffi__bless_pointer(
      jsi::Runtime &rt, const jsi::Value &thisVal, const jsi::Value *args,
      size_t count);

public:
  NativeLni(jsi::Runtime &rt,
            std::shared_ptr<uniffi_runtime::UniffiCallInvoker> callInvoker);
  virtual ~NativeLni();

  /**
   * The entry point into the crate.
   *
   * React Native must call `NativeLni.registerModule(rt, callInvoker)` before
   * using the Javascript interface.
   */
  static void registerModule(jsi::Runtime &rt,
                             std::shared_ptr<react::CallInvoker> callInvoker);

  /**
   * Some cleanup into the crate goes here.
   *
   * Current implementation is empty, however, this is not guaranteed to always
   * be the case.
   *
   * Clients should call `NativeLni.unregisterModule(rt)` after final use where
   * possible.
   */
  static void unregisterModule(jsi::Runtime &rt);

  virtual jsi::Value get(jsi::Runtime &rt, const jsi::PropNameID &name);
  virtual void set(jsi::Runtime &rt, const jsi::PropNameID &name,
                   const jsi::Value &value);
  virtual std::vector<jsi::PropNameID> getPropertyNames(jsi::Runtime &rt);
};