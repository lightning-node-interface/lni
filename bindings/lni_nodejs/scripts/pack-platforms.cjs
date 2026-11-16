'use strict'

// Packs locally only. Publishing is an explicit, separate release-workflow step.
const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } = require('node:fs')
const { join, resolve } = require('node:path')

const { root, pkg } = require('./package-context.cjs')
const output = resolve(process.argv[2] || join(root, 'dist'))
const supported = Object.keys(pkg.optionalDependencies).map((name) =>
  name.slice(pkg.name.length + 1)
)
const selected = process.argv.length > 3 ? process.argv.slice(3) : supported
assert(
  selected.length > 0 && new Set(selected).size === selected.length,
  'Specify distinct platforms'
)

const check = spawnSync(process.execPath, [join(__dirname, 'check-package.cjs')], {
  cwd: root,
  stdio: 'inherit',
})
assert.equal(check.status, 0, 'Package checks failed')
const packages = selected.map((platform) => {
  assert(supported.includes(platform), `Unsupported platform: ${platform}`)
  const directory = join(root, 'npm', platform)
  const metadata = JSON.parse(readFileSync(join(directory, 'package.json'), 'utf8'))
  const nativePath = join(directory, metadata.main)
  const localPath = join(root, metadata.main)
  assert(existsSync(nativePath) || existsSync(localPath), `Missing binary: ${metadata.main}`)
  return { directory, metadata, nativePath, localPath }
})

mkdirSync(output, { recursive: true })
for (const { directory, nativePath, localPath } of packages) {
  // A matrix build produces a local binding; collection jobs use napi artifacts.
  if (existsSync(localPath)) copyFileSync(localPath, nativePath)
  const license = join(root, 'LICENSE')
  if (existsSync(license)) copyFileSync(license, join(directory, 'LICENSE'))
}
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm'
const manifest = [...packages.map(({ directory }) => directory), root].map((directory) => {
  const result = spawnSync(
    npm,
    ['pack', '--ignore-scripts', '--json', '--pack-destination', output],
    {
      cwd: directory,
      encoding: 'utf8',
      shell: process.platform === 'win32',
    }
  )
  assert.equal(result.status, 0, `npm pack failed: ${result.stderr}`)
  const packed = JSON.parse(result.stdout)[0]
  if (directory !== root) {
    const metadata = JSON.parse(readFileSync(join(directory, 'package.json'), 'utf8'))
    assert(
      packed.files.some(({ path }) => path === metadata.main),
      `Missing packed binary: ${metadata.name}`
    )
  }
  console.log(`${packed.name}: ${packed.size} bytes packed, ${packed.unpackedSize} bytes unpacked`)
  return {
    name: packed.name,
    version: packed.version,
    filename: packed.filename,
    integrity: packed.integrity,
  }
})
writeFileSync(join(output, 'pack-manifest.json'), `${JSON.stringify(manifest, null, 2)}\n`)
