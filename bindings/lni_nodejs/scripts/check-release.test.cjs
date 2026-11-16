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
