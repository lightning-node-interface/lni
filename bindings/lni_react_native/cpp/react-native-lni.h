#ifndef LNI_H
#define LNI_H
// Generated by uniffi-bindgen-react-native
#include <cstdint>
#include <jsi/jsi.h>
#include <ReactCommon/CallInvoker.h>

namespace lni {
  using namespace facebook;

  uint8_t installRustCrate(jsi::Runtime &runtime, std::shared_ptr<react::CallInvoker> callInvoker);
  uint8_t cleanupRustCrate(jsi::Runtime &runtime);
}

#endif /* LNI_H */