import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const packageDirectory = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  '..'
);

const failures = [];

function read(relativePath) {
  const absolutePath = path.join(packageDirectory, relativePath);
  if (!existsSync(absolutePath)) {
    failures.push(`${relativePath}: missing`);
    return '';
  }
  return readFileSync(absolutePath, 'utf8');
}

function requireText(relativePath, requiredText) {
  const contents = read(relativePath);
  for (const text of requiredText) {
    if (!contents.includes(text)) {
      failures.push(`${relativePath}: missing ${JSON.stringify(text)}`);
    }
  }
}

const expectedIdentity = {
  'ubrn.config.yaml': ['name: react-native-lni-lexe', 'spec: NativeLniLexe'],
  'android/CMakeLists.txt': [
    'project(LniLexe)',
    'add_library(react-native-lni-lexe',
    '../cpp/react-native-lni-lexe.cpp',
    'target_link_libraries(react-native-lni-lexe',
  ],
  'android/build.gradle': [
    'project.properties["LniLexe_" + name]',
    'libraryName = "LniLexe"',
  ],
  'android/cpp-adapter.cpp': [
    '#include "react-native-lni-lexe.h"',
    'Java_com_reactnativelnilexe_LniLexeModule_nativeInstallRustCrate',
    'Java_com_reactnativelnilexe_LniLexeModule_nativeCleanupRustCrate',
    'lnilexe::installRustCrate',
    'lnilexe::cleanupRustCrate',
  ],
  'android/src/main/java/com/reactnativelnilexe/LniLexeModule.kt': [
    'class LniLexeModule',
    'NativeLniLexeSpec(reactContext)',
    'const val NAME = "LniLexe"',
    'System.loadLibrary("react-native-lni-lexe")',
  ],
  'android/src/main/java/com/reactnativelnilexe/LniLexePackage.kt': [
    'class LniLexePackage',
    'LniLexeModule(reactContext)',
  ],
  'cpp/react-native-lni-lexe.cpp': [
    '#include "react-native-lni-lexe.h"',
    'namespace lnilexe',
  ],
  'cpp/react-native-lni-lexe.h': ['namespace lnilexe'],
  'ios/LniLexe.h': [
    '#import "react-native-lni-lexe.h"',
    '@interface LniLexe : NSObject <NativeLniLexeSpec>',
  ],
  'ios/LniLexe.mm': [
    '#import "LniLexe.h"',
    'NativeLniLexeSpecJSI',
    'lnilexe::installRustCrate',
    'lnilexe::cleanupRustCrate',
  ],
  'LniLexe.podspec': ['s.name         = "LniLexe"'],
  'src/NativeLniLexe.ts': [
    "TurboModuleRegistry.getEnforcing<Spec>('LniLexe')",
  ],
  'src/index.tsx': ["import installer from './NativeLniLexe'"],
};

for (const [relativePath, requiredText] of Object.entries(expectedIdentity)) {
  requireText(relativePath, requiredText);
}

const podspecs = readdirSync(packageDirectory)
  .filter((filename) => filename.endsWith('.podspec'))
  .sort();
if (podspecs.length !== 1 || podspecs[0] !== 'LniLexe.podspec') {
  failures.push(`expected only LniLexe.podspec, found: ${podspecs.join(', ')}`);
}

const obsoleteFiles = [
  'ReactNativeLniLexe.podspec',
  'android/src/main/java/com/reactnativelnilexe/ReactNativeLniLexeModule.kt',
  'android/src/main/java/com/reactnativelnilexe/ReactNativeLniLexePackage.kt',
  'cpp/sunnyln-react-native-lni-lexe.cpp',
  'cpp/sunnyln-react-native-lni-lexe.h',
  'ios/ReactNativeLniLexe.h',
  'ios/ReactNativeLniLexe.mm',
  'src/NativeReactNativeLniLexe.ts',
];

for (const relativePath of obsoleteFiles) {
  if (existsSync(path.join(packageDirectory, relativePath))) {
    failures.push(`${relativePath}: obsolete scoped-name native file exists`);
  }
}

if (process.argv.includes('--generated')) {
  const expectedGeneratedIdentity = {
    'android/generated/java/com/reactnativelnilexe/NativeLniLexeSpec.java': [
      'abstract class NativeLniLexeSpec',
      'public static final String NAME = "LniLexe"',
    ],
    'android/generated/jni/ReactNativeLexeSpec-generated.cpp': [
      'NativeLniLexeSpecJSI',
      'moduleName == "LniLexe"',
    ],
    'ios/generated/ReactCodegen/ReactNativeLexeSpec/ReactNativeLexeSpec.h': [
      '@protocol NativeLniLexeSpec',
      'NativeLniLexeSpecJSI',
    ],
    'lib/module/NativeLniLexe.js': [
      "TurboModuleRegistry.getEnforcing('LniLexe')",
    ],
    'lib/module/index.js': ['import installer from "./NativeLniLexe.js"'],
    'lib/typescript/src/NativeLniLexe.d.ts': [
      'declare const _default: Spec;',
    ],
  };

  for (const [relativePath, requiredText] of Object.entries(
    expectedGeneratedIdentity
  )) {
    requireText(relativePath, requiredText);
  }

  const obsoleteGeneratedFiles = [
    'android/generated/java/com/reactnativelnilexe/NativeReactNativeLniLexeSpec.java',
    'lib/module/NativeReactNativeLniLexe.js',
    'lib/typescript/src/NativeReactNativeLniLexe.d.ts',
  ];
  for (const relativePath of obsoleteGeneratedFiles) {
    if (existsSync(path.join(packageDirectory, relativePath))) {
      failures.push(`${relativePath}: obsolete generated identity exists`);
    }
  }
}

if (failures.length > 0) {
  console.error('Native identity verification failed:');
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log('Verified stable LniLexe native identity.');
