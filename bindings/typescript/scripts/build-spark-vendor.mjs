import { mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { build } from 'esbuild';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, '..');
const shimsDir = path.resolve(__dirname, 'spark-shims');
const distVendorDir = path.resolve(projectRoot, 'dist/vendor');
const vendorEntriesDir = path.resolve(__dirname, 'spark-vendor-entries');

const runtimeOutputPath = path.resolve(distVendorDir, 'spark-runtime.browser.js');
const sparkOutputPath = path.resolve(distVendorDir, 'spark-sdk-bare.js');
const frostsBridgeOutputPath = path.resolve(distVendorDir, 'frosts-bridge.js');
const legacySparkOutputPath = path.resolve(distVendorDir, 'spark-sdk-bare.browser.js');
const sparkSourcePath = path.resolve(
  projectRoot,
  'node_modules/@buildonspark/spark-sdk/dist/bare/index.js',
);

const browserTransportPattern = 'return new ConnectionManagerBrowser(config, BareHttpTransport());';
const browserTransportReplacement =
  'return new ConnectionManagerBrowser(config, (0, import_nice_grpc_web.FetchTransport)());';

const browserAlias = {
  http: path.resolve(shimsDir, 'node-http.js'),
  https: path.resolve(shimsDir, 'node-https.js'),
  'node:crypto': path.resolve(shimsDir, 'node-crypto.js'),
  events: path.resolve(shimsDir, 'node-events.cjs'),
  vitest: path.resolve(shimsDir, 'vitest.js'),
  'bare-crypto': path.resolve(shimsDir, 'bare-crypto.js'),
  'bare-fetch': path.resolve(shimsDir, 'bare-fetch.js'),
  'bare-fetch/headers': path.resolve(shimsDir, 'bare-fetch-headers.js'),
};

async function bundleRuntimeHelper() {
  await build({
    entryPoints: [path.resolve(projectRoot, 'src/spark-runtime.ts')],
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    outfile: runtimeOutputPath,
    logLevel: 'silent',
  });
}

async function bundleFrostsBridge() {
  await build({
    entryPoints: [path.resolve(vendorEntriesDir, 'frosts-bridge.js')],
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    outfile: frostsBridgeOutputPath,
    logLevel: 'silent',
    alias: browserAlias,
  });
}

async function bundleSparkBare() {
  await build({
    entryPoints: [sparkSourcePath],
    bundle: true,
    format: 'esm',
    platform: 'browser',
    target: 'es2022',
    outfile: sparkOutputPath,
    logLevel: 'silent',
    alias: browserAlias,
  });
}

async function patchAndValidateSparkBundle() {
  let source = await readFile(sparkOutputPath, 'utf8');
  source = source.replace(browserTransportPattern, browserTransportReplacement);
  await writeFile(sparkOutputPath, source, 'utf8');

  if (/WebAssembly|\.wasm|wasm-node|wasm-browser/.test(source)) {
    throw new Error('Spark vendor bundle contains WASM references.');
  }

  if (!source.includes(browserTransportReplacement)) {
    throw new Error('Spark vendor bundle is missing the browser FetchTransport patch.');
  }

  if (source.includes(browserTransportPattern)) {
    throw new Error('Spark vendor bundle still references BareHttpTransport.');
  }
}

async function validateFrostsBundle() {
  const source = await readFile(frostsBridgeOutputPath, 'utf8');
  if (source.includes('from "vitest"') || source.includes("from 'vitest'")) {
    throw new Error('Frosts bridge bundle still imports vitest.');
  }
}

async function validateRuntimeBundle() {
  const source = await readFile(runtimeOutputPath, 'utf8');
  if (!source.includes('installSparkRuntime')) {
    throw new Error('Spark runtime bundle is missing installSparkRuntime export.');
  }
}

async function main() {
  await mkdir(distVendorDir, { recursive: true });
  await rm(legacySparkOutputPath, { force: true });
  await bundleRuntimeHelper();
  await bundleFrostsBridge();
  await bundleSparkBare();
  await patchAndValidateSparkBundle();
  await validateFrostsBundle();
  await validateRuntimeBundle();
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  console.error(`Failed to build Spark vendor bundles: ${message}`);
  process.exit(1);
});
