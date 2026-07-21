import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';

const cmakePath = fileURLToPath(
  new URL('../android/CMakeLists.txt', import.meta.url)
);
const generatedResolver = `execute_process(
    COMMAND node -p "require.resolve('uniffi-bindgen-react-native/package.json')"
    OUTPUT_VARIABLE UNIFFI_BINDGEN_PATH
    OUTPUT_STRIP_TRAILING_WHITESPACE
)
# Get the directory; get_filename_component and cmake_path will normalize
# paths with Windows path separators.
get_filename_component(UNIFFI_BINDGEN_PATH "\${UNIFFI_BINDGEN_PATH}" DIRECTORY)`;
const exportedResolver = `execute_process(
    COMMAND node -p "require('path').resolve(require.resolve('uniffi-bindgen-react-native'), '../../../..')"
    OUTPUT_VARIABLE UNIFFI_BINDGEN_PATH
    OUTPUT_STRIP_TRAILING_WHITESPACE
)`;

const cmake = await readFile(cmakePath, 'utf8');
if (!cmake.includes(generatedResolver) && !cmake.includes(exportedResolver)) {
  throw new Error('Could not find the generated UBRN package resolver');
}

if (cmake.includes(generatedResolver)) {
  await writeFile(
    cmakePath,
    cmake.replace(generatedResolver, exportedResolver)
  );
}

const iosPath = fileURLToPath(new URL('../ios/LniLexe.mm', import.meta.url));
const generatedExample = `// Automated testing checks lnilexe
// by comparing the whole line here.
/*
- (NSNumber *)multiply:(double)a b:(double)b {
    NSNumber *result = @(lnilexe::multiply(a, b));
}
*/

`;
const ios = await readFile(iosPath, 'utf8');
if (ios.includes(generatedExample)) {
  await writeFile(iosPath, ios.replace(generatedExample, ''));
}

const androidAdapterPath = fileURLToPath(
  new URL('../android/cpp-adapter.cpp', import.meta.url)
);
const generatedTestBlock =
  /\/\/ Automated testing checks [^\n]+\n\/\/ by comparing the whole line here\.\n\/\*\n[\s\S]*?\n\*\/\n\n/;
const androidAdapter = await readFile(androidAdapterPath, 'utf8');
const fixedAndroidAdapter = androidAdapter.replace(generatedTestBlock, '');
const normalizedAndroidAdapter = fixedAndroidAdapter.endsWith('\n')
  ? fixedAndroidAdapter
  : `${fixedAndroidAdapter}\n`;
if (normalizedAndroidAdapter !== androidAdapter) {
  await writeFile(androidAdapterPath, normalizedAndroidAdapter);
}
