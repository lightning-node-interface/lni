import { statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const packageDirectory = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);

const requiredArtifacts = [
  {
    path: 'ReactNativeLexeFramework.xcframework/Info.plist',
    minimumSize: 1,
  },
  {
    path: 'ReactNativeLexeFramework.xcframework/ios-arm64/libreact_native_lexe.a',
    minimumSize: 1_000_000,
  },
  {
    path: 'ReactNativeLexeFramework.xcframework/ios-arm64-simulator/libreact_native_lexe.a',
    minimumSize: 1_000_000,
  },
  {
    path: 'android/src/main/jniLibs/arm64-v8a/libreact_native_lexe.so',
    minimumSize: 1_000_000,
  },
  {
    path: 'android/src/main/jniLibs/armeabi-v7a/libreact_native_lexe.so',
    minimumSize: 1_000_000,
  },
  {
    path: 'android/src/main/jniLibs/x86/libreact_native_lexe.so',
    minimumSize: 1_000_000,
  },
  {
    path: 'android/src/main/jniLibs/x86_64/libreact_native_lexe.so',
    minimumSize: 1_000_000,
  },
];

if (process.argv.includes('--list')) {
  for (const artifact of requiredArtifacts) {
    console.log(artifact.path);
  }
  process.exit(0);
}

const missingArtifacts = requiredArtifacts.filter((artifact) => {
  try {
    // A real native library is much larger than a Git LFS pointer or an empty
    // placeholder. This also catches incomplete CI artifact downloads.
    return (
      statSync(path.join(packageDirectory, artifact.path)).size <
      artifact.minimumSize
    );
  } catch {
    return true;
  }
});

if (missingArtifacts.length > 0) {
  console.error('Missing native artifacts required by the npm package:');
  for (const artifact of missingArtifacts) {
    console.error(`- ${artifact.path}`);
  }
  console.error(
    'Run `yarn ubrn:ios` and `yarn ubrn:android` before packing or publishing.'
  );
  process.exit(1);
}

console.log('Verified iOS and Android native artifacts.');
