# LNI Kotlin Example

This is an example Kotlin/Android project demonstrating how to use the LNI Kotlin bindings.

## Project Structure

```
example/
├── app/
│   ├── src/main/
│   │   ├── kotlin/com/lni/example/
│   │   │   └── Main.kt          # Example usage
│   │   └── jniLibs/              # Native libraries go here
│   │       ├── arm64-v8a/liblni.so
│   │       ├── armeabi-v7a/liblni.so
│   │       ├── x86/liblni.so
│   │       └── x86_64/liblni.so
│   └── build.gradle.kts
├── settings.gradle.kts
└── build.gradle.kts
```

## Setup

1. First, build the LNI Kotlin bindings:
   ```bash
   cd ../
   ./build.sh --release
   ```

2. Build native libraries for your target architectures (e.g., for Android):
   ```bash
   # Example for arm64-v8a
   cargo build --package lni --features uniffi --release --target aarch64-linux-android
   # Example Mac Apple Silicon
   cargo build --package lni --features uniffi --release --target aarch64-linux-android
   cp ../../target/aarch64-linux-android/release/liblni.so app/src/main/jniLibs/arm64-v8a/
   ```

3. Build the example:
   ```bash
   ./gradlew build
   ```

## Running

For JVM-based execution (testing on desktop):
```bash
./gradlew run
```

For Android:
1. `cd .. && ./build.sh --release --android 2>&1`
2. Import the project into Android Studio and Open `lni/bindings/kotlin/example`
3. File → Sync Project with Gradle Files
4. Build → Clean Project
5. Build and run on an emulator or device

## Usage Examples

See `app/src/main/kotlin/com/lni/example/Main.kt` for examples of:
- Creating nodes (Strike, Blink, NWC, etc.)
- Getting node info
- Creating invoices
- Paying invoices
- Listing transactions
