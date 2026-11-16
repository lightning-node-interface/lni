'use strict'

const { spawnSync } = require('node:child_process')
const { mkdirSync, readFileSync, writeFileSync } = require('node:fs')
const { resolve } = require('node:path')

const root = resolve(__dirname, '..')
const run = (command, args) => {
  const result = spawnSync(command, args, { cwd: root, stdio: 'inherit', shell: process.platform === 'win32' })
  if (result.error) throw result.error
  if (result.status !== 0) process.exit(result.status ?? 1)
}
mkdirSync(resolve(root, 'generated'), { recursive: true })
run('napi', ['build', '.', '--platform', '--dts', 'generated/index.d.ts', '--js', 'native.js', '--cargo-flags=--locked', ...process.argv.slice(2)])
const declarations = resolve(root, 'generated/index.d.ts')
writeFileSync(declarations, readFileSync(declarations, 'utf8').replace(/[ \t]+$/gm, ''))
run('tsc', ['-p', 'tsconfig.helpers.json'])
