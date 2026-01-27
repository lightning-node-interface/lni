#!/bin/bash
# Patches generated Android files for the example app
# Only runs on Linux due to different generated paths on macOS

if [[ "$OSTYPE" != "linux-gnu"* ]]; then
  echo "Skipping Android patches (not Linux)"
  exit 0
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EXAMPLE_ANDROID="$SCRIPT_DIR/../example/android/app/src/main/java/com/lnireactnative"

# Check if the expected path exists
if [ ! -d "$EXAMPLE_ANDROID" ]; then
  echo "Android source directory not found, skipping patches"
  exit 0
fi

# Fix MainApplication.kt - use OpenSourceMergedSoMapping for SoLoader
if [ -f "$EXAMPLE_ANDROID/MainApplication.kt" ]; then
  # Add import if not present
  if ! grep -q "OpenSourceMergedSoMapping" "$EXAMPLE_ANDROID/MainApplication.kt"; then
    sed -i 's/import com.facebook.soloader.SoLoader/import com.facebook.react.soloader.OpenSourceMergedSoMapping\nimport com.facebook.soloader.SoLoader/' "$EXAMPLE_ANDROID/MainApplication.kt"
  fi

  # Replace SoLoader.init call
  sed -i 's/SoLoader.init(this, false)/SoLoader.init(this, OpenSourceMergedSoMapping)/' "$EXAMPLE_ANDROID/MainApplication.kt"

  echo "Patched MainApplication.kt"
fi

# Fix MainActivity.kt - correct component name
if [ -f "$EXAMPLE_ANDROID/MainActivity.kt" ]; then
  sed -i 's/getMainComponentName(): String = "lni_react_native-example"/getMainComponentName(): String = "LniReactNativeExample"/' "$EXAMPLE_ANDROID/MainActivity.kt"

  echo "Patched MainActivity.kt"
fi

echo "Android patches applied"
