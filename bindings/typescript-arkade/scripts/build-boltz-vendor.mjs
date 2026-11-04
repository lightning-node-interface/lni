import { build } from 'esbuild';
import { mkdir, readFile, writeFile, copyFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bridge = path.join(root, 'src/internal/boltz-compat.ts');
const directory = path.join(root, 'dist/vendor');
await mkdir(directory, { recursive: true });
const result = await build({
  entryPoints: [fileURLToPath(import.meta.resolve('@arkade-os/boltz-swap'))],
  outfile: path.join(directory, 'boltz-swap.js'),
  bundle: true,
  metafile: true,
  format: 'esm',
  platform: 'browser',
  target: 'es2022',
  external: ['@scure/btc-signer', '@scure/btc-signer/*'],
  plugins: [{
    name: 'boltz-current-sdk',
    setup(builder) {
      builder.onResolve({ filter: /^@arkade-os\/sdk$/ }, (args) =>
        args.importer === bridge
          ? { path: '@arkade-os/sdk', external: true }
          : { path: bridge }
      );
    },
  }],
});
// These are private implementation declarations, outside the public API graph.
await rm(path.join(directory, 'boltz-swap.d.ts'), { force: true });
await rm(path.join(root, 'dist/internal/boltz.d.ts'), { force: true });

// Preserve licenses for the third-party modules included in the bundle.
const packages = new Set();
for (const input of Object.keys(result.metafile.inputs)) {
  let current = path.dirname(path.resolve(root, input));
  while (current !== path.dirname(current)) {
    try {
      const manifest = JSON.parse(await readFile(path.join(current, 'package.json'), 'utf8'));
      if (manifest.name && current !== root) packages.add(current);
      break;
    } catch { current = path.dirname(current); }
  }
}
const licenseDirectory = path.join(directory, 'licenses');
await mkdir(licenseDirectory, { recursive: true });
const attribution = [];
for (const folder of packages) {
  const manifest = JSON.parse(await readFile(path.join(folder, 'package.json'), 'utf8'));
  attribution.push(`${manifest.name}@${manifest.version}: ${JSON.stringify(manifest.license ?? 'see package license')}`);
  for (const filename of ['LICENSE', 'LICENSE.md', 'LICENSE.txt', 'LICENCE', 'license', 'license.md']) {
    try {
      await copyFile(path.join(folder, filename), path.join(licenseDirectory, `${manifest.name.replaceAll('/', '-')}-${filename}`));
    } catch (error) { if (error.code !== 'ENOENT') throw error; }
  }
}
await writeFile(path.join(licenseDirectory, 'PACKAGES.txt'), attribution.sort().join('\n') + '\n');
