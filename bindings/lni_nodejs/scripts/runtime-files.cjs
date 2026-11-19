'use strict'

const { mkdirSync, readFileSync, writeFileSync } = require('node:fs')
const { dirname, join, resolve } = require('node:path')
const { root, pkg } = require('./package-context.cjs')

// Copies are committed build output. Edit the shared originals, not the copies.
const files = [
  'index.js',
  'contracts.d.ts',
  'lexe-connect.js',
  'lexe-connect.d.ts',
  'lexe-events.js',
  'lexe-errors.js',
  'helpers/lexe.ts',
  'helpers/lexe.js',
  'helpers/lexe.d.ts',
  'lexe/package.json',
  'LEXE.md',
]
const tests = [
  'lexe-connect.test.js',
  'lexe-events.test.js',
  'lexe-errors.test.js',
  'lexe-monitor.test.js',
  'helpers/lexe.test.js',
]
function entries(includeTests = false) {
  if (!pkg.lni?.lexeOnly) return []
  return [...files, ...(includeTests ? tests : [])].map((path) => {
    const content = readFileSync(resolve(root, '../lni_nodejs', path), 'utf8')
    return {
      path,
      content: path === 'LEXE.md' ? content.replaceAll('@sunnyln/lni-nodejs', pkg.name) : content,
    }
  })
}
function sync() {
  for (const { path, content } of entries(true)) {
    const output = join(root, path)
    mkdirSync(dirname(output), { recursive: true })
    writeFileSync(output, content)
  }
}
module.exports = { entries, sync }
