'use strict'

const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const { root, cargoPackage } = require('./package-context.cjs')

const result = spawnSync('cargo', ['tree', '-p', cargoPackage, '--edges', 'normal', '--locked'], {
  cwd: root,
  encoding: 'utf8',
})
assert.equal(result.status, 0, result.stderr)
assert(
  !/breez-sdk-spark|arkade/i.test(result.stdout),
  'Experimental SDKs must not be linked into the Node addon'
)
console.log('Native dependency tree excludes Spark SDK and Arkade.')
