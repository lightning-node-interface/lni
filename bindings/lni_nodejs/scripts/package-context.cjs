'use strict'

const { readFileSync } = require('node:fs')
const { join, resolve } = require('node:path')

// The Lexe-only package reuses these tools through its scripts/run.cjs launcher.
const root = process.env.LNI_NODEJS_PACKAGE_DIR
  ? resolve(process.env.LNI_NODEJS_PACKAGE_DIR)
  : resolve(__dirname, '..')
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'))
module.exports = {
  root,
  pkg,
  cargoPackage: pkg.lni?.cargoPackage || 'lni_napi_rs',
  tagPrefix: `${pkg.name.split('/').pop()}-v`,
}
