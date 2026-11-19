'use strict'

// Verifies the npm package from this checkout without installing anything:
// committed helper output matches its TypeScript source (when TypeScript is
// available), and the pack manifest contains every entry point but no tests.

const { spawnSync } = require('child_process')
const { mkdtempSync, readFileSync, readdirSync, rmSync } = require('fs')
const { tmpdir } = require('os')
const { join, resolve } = require('path')

const { root } = require('./package-context.cjs')
const failures = []

function checkHelpersAreCurrent() {
  let tsc
  try {
    tsc = require.resolve('typescript/bin/tsc', { paths: [root] })
  } catch (_) {
    console.log('TypeScript is not installed; using the committed helper output.')
    return
  }
  const outDir = mkdtempSync(join(tmpdir(), 'lni-helpers-'))
  try {
    const result = spawnSync(
      process.execPath,
      [tsc, '-p', 'tsconfig.helpers.json', '--outDir', outDir],
      {
        cwd: root,
        stdio: 'inherit',
      }
    )
    if (result.status !== 0) {
      failures.push('helpers do not compile')
      return
    }
    for (const file of ['lexe.js', 'lexe.d.ts']) {
      const committed = readFileSync(join(root, 'helpers', file), 'utf8')
      if (readFileSync(join(outDir, file), 'utf8') !== committed) {
        failures.push(`helpers/${file} is stale; run the package build`)
      }
    }
  } finally {
    rmSync(outDir, { recursive: true, force: true })
  }
}

function packedFiles() {
  const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm'
  const result = spawnSync(npm, ['pack', '--dry-run', '--json', '--ignore-scripts'], {
    cwd: root,
    encoding: 'utf8',
    shell: process.platform === 'win32',
  })
  if (result.status !== 0) throw new Error(`npm pack failed: ${result.stderr}`)
  return new Set(JSON.parse(result.stdout)[0].files.map((file) => file.path))
}

checkHelpersAreCurrent()

const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'))
for (const { path, content } of require('./runtime-files.cjs').entries()) {
  if (readFileSync(join(root, path), 'utf8') !== content) {
    failures.push(`${path} differs from the shared runtime; run sync:runtime`)
  }
}
const exportTargets = Object.values(pkg.exports).flatMap((entry) => Object.values(entry))
const required = [
  'package.json',
  pkg.main,
  pkg.types,
  'native.js',
  'generated/index.d.ts',
  'contracts.d.ts',
  'lexe-connect.js',
  'lexe-connect.d.ts',
  'lexe-events.js',
  'lexe-errors.js',
  'lexe/package.json',
  ...exportTargets,
].map((path) => path.replace(/^\.\//, ''))

const files = packedFiles()
for (const path of new Set(required)) {
  if (!files.has(path)) failures.push(`package is missing ${path}`)
}
for (const path of files) {
  if (
    /\.test\.js$|^helpers\/.*(?<!\.d)\.ts$|^scripts\/|^tsconfig|(^|\/)\.env|\.node$|^npm\/|^src\//.test(
      path
    )
  ) {
    failures.push(`package must not include ${path}`)
  }
}

// Local workspaces make unpublished platform packages resolvable for development.
// Published optional dependencies must still use exact registry versions.
const platforms = readdirSync(join(root, 'npm')).sort()
const optional = pkg.optionalDependencies
if (
  !optional ||
  platforms.length !== pkg.napi.triples.additional.length ||
  pkg.napi.triples.defaults !== false
) {
  failures.push('platform directories must match the explicit napi target list')
}
if (Object.keys(optional || {}).length !== platforms.length) {
  failures.push('optionalDependencies must contain exactly the supported platforms')
}
for (const platform of platforms) {
  const platformPkg = JSON.parse(readFileSync(join(root, 'npm', platform, 'package.json'), 'utf8'))
  const name = `${pkg.name}-${platform}`
  if (
    platformPkg.name !== name ||
    platformPkg.version !== pkg.version ||
    optional[name] !== pkg.version
  ) {
    failures.push(`${platform}: package name and exact version must match the parent`)
  }
  if (platformPkg.main !== `${pkg.napi.name}.${platform}.node`) {
    failures.push(`${platform}: incorrect binary name`)
  }
}
for (const file of ['native.js', 'generated/index.d.ts', 'index.d.ts']) {
  const content = readFileSync(join(root, file), 'utf8')
  const otherProviders = /(?:Lnd|Cln|Phoenixd|Blink|Strike|Speed|Nwc|Galoy|Flash)(?:Node|Config)/
  if (
    /Spark|Arkade|lni_js-/.test(content) ||
    (pkg.lni?.lexeOnly && file !== 'generated/index.d.ts' && otherProviders.test(content))
  ) {
    failures.push(`${file}: stale or experimental native exports; regenerate bindings`)
  }
}

if (failures.length) {
  console.error(failures.join('\n'))
  process.exit(1)
}
console.log(`Package manifest OK (${files.size} files).`)
