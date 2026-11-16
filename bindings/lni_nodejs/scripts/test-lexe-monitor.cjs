'use strict'

// Builds a debug addon with the offline `lexe_monitor_test` hooks and runs the
// native invoice-monitor lifecycle tests against it. Extra arguments go to cargo
// (for example `-- --offline -j 2`). This overwrites the debug cdylib in the
// Cargo target directory, not the packaged `.node` file.

const { spawnSync } = require('child_process')
const { join, resolve } = require('path')

const root = resolve(__dirname, '..')
const workspace = resolve(root, '../..')
const run = (command, args, options) => {
  const result = spawnSync(command, args, { stdio: 'inherit', ...options })
  if (result.error) throw result.error
  if (result.status !== 0) process.exit(result.status === null ? 1 : result.status)
}

run('cargo', ['build', '-p', 'lni_napi_rs', '--features', 'lexe_monitor_test', '--locked', ...process.argv.slice(2)], {
  cwd: workspace,
})
const library = { linux: 'liblni_napi_rs.so', darwin: 'liblni_napi_rs.dylib', win32: 'lni_napi_rs.dll' }[process.platform]
const targetDir = process.env.CARGO_TARGET_DIR ? resolve(process.env.CARGO_TARGET_DIR) : join(workspace, 'target')
run(process.execPath, ['--test', 'lexe-monitor.test.js'], {
  cwd: root,
  env: { ...process.env, LNI_MONITOR_TEST_ADDON: join(targetDir, 'debug', library) },
})
