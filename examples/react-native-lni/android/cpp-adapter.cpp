#include <jni.h>
#include "react-native-lni.h"

extern "C"
JNIEXPORT jdouble JNICALL
Java_com_lni_LniModule_nativeMultiply(JNIEnv *env, jclass type, jdouble a, jdouble b) {
    return lni::multiply(a, b);
}
