'use strict'

const assert = require('node:assert/strict')
const { createHash } = require('node:crypto')
const { appendFileSync, readFileSync } = require('node:fs')
const { basename, join, resolve } = require('node:path')

const { pkg, tagPrefix } = require('./package-context.cjs')
const output = resolve(process.argv[2] || 'dist')
const manifest = JSON.parse(readFileSync(join(output, 'pack-manifest.json'), 'utf8'))
const expected = [...Object.keys(pkg.optionalDependencies), pkg.name]
assert.match(pkg.version, /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$/)
assert.equal(
  process.env.RELEASE_TAG,
  `${tagPrefix}${pkg.version}`,
  'Release tag must match package.json'
)
assert.deepEqual(
  manifest.map(({ name }) => name),
  expected,
  'Require all platform packages, with root last'
)
for (const entry of manifest) {
  assert.equal(entry.version, pkg.version)
  assert.equal(
    entry.filename,
    `${entry.name.replace('@', '').replace('/', '-')}-${pkg.version}.tgz`
  )
  assert.equal(basename(entry.filename), entry.filename)
  const integrity = `sha512-${createHash('sha512')
    .update(readFileSync(join(output, entry.filename)))
    .digest('base64')}`
  assert.equal(entry.integrity, integrity, `Tarball integrity failed: ${entry.name}`)
}
if (process.env.GITHUB_OUTPUT) {
  const prerelease = pkg.version.includes('-')
  const distTag = /^\d+\.\d+\.\d+-beta(?:\.|$)/.test(pkg.version)
    ? 'beta'
    : prerelease ? 'next' : 'latest'
  appendFileSync(
    process.env.GITHUB_OUTPUT,
    `prerelease=${prerelease}\ndist-tag=${distTag}\n`
  )
}
console.log(`Release package set and integrity OK: ${pkg.version}`)
