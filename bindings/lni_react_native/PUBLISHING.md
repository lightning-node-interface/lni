# Publishing react-native-lni to npm

This guide explains how to build and publish the `react-native-lni` package to npm.

## Prerequisites

- Node.js 18+ and Yarn
- Rust toolchain (rustc, cargo)
- uniffi-bindgen-react-native CLI (`npm install -g uniffi-bindgen-react-native`)
- iOS development tools (Xcode) for building the iOS framework
- Android NDK for building Android libraries
- npm account with publish permissions

## Build Process

The package requires a multi-step build process because it combines Rust native code with React Native bindings.

> **Note**: The package directory is located at `bindings/lni_react_native` in the repository, but the published npm package name is `react-native-lni`.

### 1. Clean Previous Builds (Optional)

From the repository root:
```bash
cd bindings/lni_react_native
yarn ubrn:clean
```

### 2. Generate Native Bindings

Generate the React Native bindings from the Rust code using uniffi-bindgen-react-native:

**For iOS:**
```bash
yarn ubrn:ios
cd example/ios && pod install
```

**For Android:**
```bash
yarn ubrn:android
```

This generates:
- `src/generated/` - TypeScript/JavaScript bindings
- `ios/generated/` - iOS native code
- `android/generated/` - Android native code
- `cpp/` - C++ bridge code
- `LniReactNativeFramework.xcframework` - iOS framework

### 3. Build JavaScript/TypeScript

Once the generated files exist, build the JS/TS output:

```bash
yarn build
```

This creates:
- `lib/commonjs/` - CommonJS modules
- `lib/module/` - ES modules
- `lib/typescript/` - TypeScript definitions

### 4. Create Package

Create a tarball for testing:

```bash
yarn pack
```

This creates `react-native-lni-v0.1.1.tgz` (or similar based on version).

## Testing the Package Locally

Before publishing, test the package in a React Native app:

1. Copy the `.tgz` file to your test project
2. Install it: `yarn add ./react-native-lni-v0.1.1.tgz`
3. Follow the installation instructions in the README
4. Test all major functionality

## Publishing to npm

### First Time Setup

1. Create an npm account if you don't have one: https://www.npmjs.com/signup
2. Login to npm: `npm login`
3. Ensure you have publish permissions for the `react-native-lni` package

### Version Management

Update the version in `package.json` following semantic versioning:
- Patch (0.1.1 → 0.1.2): Bug fixes
- Minor (0.1.1 → 0.2.0): New features, backward compatible
- Major (0.1.1 → 1.0.0): Breaking changes

Or use the release-it tool:
```bash
yarn release
```

### Publish

```bash
npm publish
```

Or with the `public` flag if it's the first publish:
```bash
npm publish --access public
```

## Automated Publishing

For automated publishing via CI/CD:

1. Set up npm authentication token in your CI environment
2. Ensure the build steps run in order:
   - Install dependencies
   - Run ubrn build (iOS and Android)
   - Build JS/TS
   - Publish

## Files Included in Package

The package includes (as specified in `package.json` files array):
- `src/` - Source TypeScript files including generated bindings
- `lib/` - Compiled JavaScript and TypeScript definitions
- `android/` - Android native code
- `ios/` - iOS native code
- `cpp/` - C++ bridge code
- `*.podspec` - CocoaPods specification
- `LniReactNativeFramework.xcframework` - Prebuilt iOS framework
- `react-native.config.js` - React Native configuration

Excluded files:
- Build artifacts (`android/build`, `ios/build`)
- Development files (`__tests__`, `example/`)
- Configuration files (`.gitignore`, etc.)

## Troubleshooting

### TypeScript Build Fails

If TypeScript compilation fails but the generated files exist, the build script has fallback logic. Check that `src/generated/` directory exists and contains the `lni.ts` file.

### Missing Native Frameworks

Ensure you've run the ubrn build steps (`yarn ubrn:ios` and/or `yarn ubrn:android`) before building the JS package.

### Version Conflicts

If you get "version already published" error, increment the version number in package.json.

## Verification

After publishing, verify the package:

1. Check on npm: https://www.npmjs.com/package/react-native-lni
2. Install in a test project: `npm install react-native-lni`
3. Verify all files are present: `ls node_modules/react-native-lni/`
4. Test that imports work: `import { LndNode } from 'react-native-lni'`
