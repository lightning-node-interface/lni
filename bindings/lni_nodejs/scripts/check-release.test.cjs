'use strict'

const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const { createHash } = require('node:crypto')
const { mkdtempSync, readFileSync, rmSync, writeFileSync } = require('node:fs')
const { tmpdir } = require('node:os')
const { join } = require('node:path')
const { test } = require('node:test')
const { pkg, tagPrefix } = require('./package-context.cjs')

// Metadata fixtures exercise release gates without building other platforms or
// invoking npm publish. Real native loading is checked by test-install.cjs.
test('release validation rejects wrong tags, missing packages, ordering errors, and corrupted tarballs', () => {
  const scratch = mkdtempSync(join(tmpdir(), 'lni-release-policy-'))
  try {
    const manifest = [...Object.keys(pkg.optionalDependencies), pkg.name].map((name) => {
      const payload = Buffer.from(`Release metadata fixture for ${name}`)
      const filename = `${name.replace('@', '').replace('/', '-')}-${pkg.version}.tgz`
      writeFileSync(join(scratch, filename), payload)
      return {
        name,
        version: pkg.version,
        filename,
        integrity: `sha512-${createHash('sha512').update(payload).digest('base64')}`,
      }
    })
    const manifestPath = join(scratch, 'pack-manifest.json')
    const validate = (tag) =>
      spawnSync(process.execPath, [join(__dirname, 'check-release.cjs'), scratch], {
        env: { ...process.env, RELEASE_TAG: tag, GITHUB_OUTPUT: '' },
        encoding: 'utf8',
      })
    const tag = `${tagPrefix}${pkg.version}`
    writeFileSync(manifestPath, JSON.stringify(manifest))
    assert.equal(validate(tag).status, 0)
    assert.notEqual(validate(`v${pkg.version}`).status, 0)
    assert.notEqual(validate(`${tagPrefix}999.0.0`).status, 0)
    writeFileSync(manifestPath, JSON.stringify(manifest.slice(1)))
    assert.notEqual(validate(tag).status, 0)
    writeFileSync(manifestPath, JSON.stringify([...manifest].reverse()))
    assert.notEqual(validate(tag).status, 0)
    writeFileSync(manifestPath, JSON.stringify(manifest))
    const target = join(scratch, manifest[0].filename)
    writeFileSync(target, Buffer.concat([readFileSync(target), Buffer.from('corrupted')]))
    assert.notEqual(validate(tag).status, 0)
  } finally {
    rmSync(scratch, { recursive: true, force: true })
  }
})

for (const [version, distTag, prerelease] of [
  ['0.1.0', 'latest', false],
  ['0.1.0-beta.1', 'beta', true],
  ['0.1.0-rc.1', 'next', true],
]) {
  test(`release ${version} selects npm tag ${distTag}`, () => {
    const scratch = mkdtempSync(join(tmpdir(), 'lni-release-tags-'))
    try {
      const optionalDependencies = Object.fromEntries(
        Object.keys(pkg.optionalDependencies).map((name) => [name, version])
      )
      writeFileSync(join(scratch, 'package.json'), JSON.stringify({
        ...pkg, version, optionalDependencies,
      }))
      const manifest = [...Object.keys(optionalDependencies), pkg.name].map((name) => {
        const payload = Buffer.from(`Release metadata fixture for ${name}`)
        const filename = `${name.replace('@', '').replace('/', '-')}-${version}.tgz`
        writeFileSync(join(scratch, filename), payload)
        return {
          name, version, filename,
          integrity: `sha512-${createHash('sha512').update(payload).digest('base64')}`,
        }
      })
      writeFileSync(join(scratch, 'pack-manifest.json'), JSON.stringify(manifest))
      const output = join(scratch, 'github-output')
      const result = spawnSync(process.execPath, [join(__dirname, 'check-release.cjs'), scratch], {
        env: {
          ...process.env,
          LNI_NODEJS_PACKAGE_DIR: scratch,
          RELEASE_TAG: `${tagPrefix}${version}`,
          GITHUB_OUTPUT: output,
        },
        encoding: 'utf8',
      })
      assert.equal(result.status, 0, result.stderr)
      assert.equal(readFileSync(output, 'utf8'), `prerelease=${prerelease}\ndist-tag=${distTag}\n`)
    } finally {
      rmSync(scratch, { recursive: true, force: true })
    }
  })
}
