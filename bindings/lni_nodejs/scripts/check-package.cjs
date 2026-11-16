'use strict'

// Verifies the npm package from this checkout without installing anything:
// committed helper output matches its TypeScript source (when TypeScript is
// available), and the pack manifest contains every entry point but no tests.

const { spawnSync } = require('child_process')
const { mkdtempSync, readFileSync, rmSync } = require('fs')
const { tmpdir } = require('os')
const { join, resolve } = require('path')

const root = resolve(__dirname, '..')
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
    const result = spawnSync(process.execPath, [tsc, '-p', 'tsconfig.helpers.json', '--outDir', outDir], {
      cwd: root,
      stdio: 'inherit',
    })
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
const exportTargets = Object.values(pkg.exports).flatMap((entry) => Object.values(entry))
const required = [
  'package.json',
  pkg.main,
  pkg.types,
  'native.js',
  'generated/index.d.ts',
  'lexe-connect.js',
  'lexe-connect.d.ts',
  'lexe-events.js',
  'lexe/package.json',
  ...exportTargets,
].map((path) => path.replace(/^\.\//, ''))

const files = packedFiles()
for (const path of new Set(required)) {
  if (!files.has(path)) failures.push(`package is missing ${path}`)
}
for (const path of files) {
  if (/\.test\.js$|^helpers\/.*(?<!\.d)\.ts$|^scripts\/|^tsconfig|(^|\/)\.env/.test(path)) {
    failures.push(`package must not include ${path}`)
  }
}

if (failures.length) {
  console.error(failures.join('\n'))
  process.exit(1)
}
console.log(`Package manifest OK (${files.size} files).`)
