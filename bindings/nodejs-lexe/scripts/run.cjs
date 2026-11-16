'use strict'

const assert = require('node:assert/strict')
const { resolve } = require('node:path')
const command = process.argv.splice(2, 1)[0]
assert(/^[a-z-]+(?:\.test)?$/.test(command), 'Expected a shared package script name')
process.env.LNI_NODEJS_PACKAGE_DIR = resolve(__dirname, '..')
require(resolve(__dirname, '../../lni_nodejs/scripts', `${command}.cjs`))
