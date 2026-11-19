'use strict'

// An offline consumer install, not a require() from the source checkout.
// No wallet credentials, provider requests, or registry writes are used.
const assert = require('node:assert/strict')
const { spawnSync } = require('node:child_process')
const {
  mkdtempSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} = require('node:fs')
const { tmpdir } = require('node:os')
const { join, resolve } = require('node:path')

const { root, pkg } = require('./package-context.cjs')
const platform = (() => {
  if (process.platform === 'linux') {
    assert(
      process.report.getReport().header.glibcVersionRuntime,
      'Linux musl is not a supported target'
    )
    return `linux-${process.arch}-gnu`
  }
  if (process.platform === 'win32') return `win32-${process.arch}-msvc`
  return `${process.platform}-${process.arch}`
})()
assert(pkg.optionalDependencies[`${pkg.name}-${platform}`], `Unsupported platform: ${platform}`)
const scratch = mkdtempSync(join(tmpdir(), 'lni-nodejs-install-'))
const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm'
function run(command, args, cwd = scratch) {
  const result = spawnSync(command, args, {
    cwd,
    encoding: 'utf8',
    shell: process.platform === 'win32' && command === npm,
  })
  assert.equal(result.status, 0, `${command} failed:\n${result.stdout}\n${result.stderr}`)
  return result.stdout
}
try {
  const packs = process.argv[2] ? resolve(process.argv[2]) : join(scratch, 'packs')
  if (!process.argv[2])
    run(process.execPath, [join(__dirname, 'pack-platforms.cjs'), packs, platform], root)
  const manifest = JSON.parse(readFileSync(join(packs, 'pack-manifest.json'), 'utf8'))
  const tarballs = [pkg.name, `${pkg.name}-${platform}`].map((name) => {
    const entry = manifest.find((entry) => entry.name === name)
    assert(entry && entry.version === pkg.version, `Missing tarball: ${name}`)
    return join(packs, entry.filename)
  })
  writeFileSync(
    join(scratch, 'package.json'),
    JSON.stringify({ name: 'lni-consumer-test', private: true, version: '1.0.0' })
  )
  // Other platforms are absent from this fresh, isolated offline cache.
  run(npm, [
    'install',
    '--offline',
    '--ignore-scripts',
    '--no-audit',
    '--no-fund',
    '--cache',
    join(scratch, 'cache'),
    ...tarballs,
  ])
  const [scope, packageName] = pkg.name.split('/')
  const installed = join(scratch, 'node_modules', scope, packageName)
  assert(
    !readdirSync(installed).some((file) => file.endsWith('.node')),
    'Root package must not bundle any binaries'
  )
  assert.deepEqual(
    readdirSync(join(scratch, 'node_modules', scope)).filter((name) =>
      name.startsWith(`${packageName}-`)
    ),
    [`${packageName}-${platform}`],
    'Install must contain only the matching platform binary'
  )
  const commonJS = `
    const assert = require('node:assert/strict')
    const pkg = require('${pkg.name}')
    const helper = require('${pkg.name}/lexe')
    for (const name of ${JSON.stringify(pkg.lni?.lexeOnly ? ['LexeNode', 'LexeConnect', 'LexeConnectSession'] : ['LexeNode', 'LexeConnect', 'LexeConnectSession', 'LndNode', 'ClnNode', 'NwcNode'])}) {
      assert.equal(typeof pkg[name], 'function', name)
    }
    assert(!Object.keys(pkg).some((name) => /spark|arkade/i.test(name)))
    assert.equal(pkg.LexeNode.monitorTestInstance, undefined, 'Release must exclude monitor test hooks')
    assert.equal(pkg.LexeNode.prototype.startTestMonitor, undefined, 'Release must exclude monitor test hooks')
    ${pkg.lni?.lexeOnly ? "for (const name of ['LndNode','ClnNode','NwcNode','PhoenixdNode','BlinkNode','StrikeNode','SpeedNode']) assert(!(name in pkg), name)" : ''}
    const platformPath = require.resolve('${pkg.name}-${platform}')
    assert(platformPath.endsWith('.node'))
    assert(require.cache[platformPath], 'Loaded addon must come from its platform package')
    ${pkg.lni?.lexeOnly ? "const native = require(platformPath); for (const name of ['LndNode','ClnNode','NwcNode','PhoenixdNode','BlinkNode','StrikeNode','SpeedNode']) assert(!(name in native), name)" : ''}
    for (const name of ['getExclusiveAddress', 'checkAddress', 'getReceivingCapabilities']) {
      assert.equal(typeof pkg.LexeNode.prototype[name], 'function', name)
    }
    const seed = require('node:crypto').randomBytes(32)
    const session = helper.restoreLexeSession({ requesterName: 'Offline package test', scopes: ['read_info', 'receive'] }, seed, Date.now() + 60000)
    assert.equal(typeof session.connectionString(), 'string')
    session.close()
  `
  run(process.execPath, ['-e', commonJS])
  run(process.execPath, [
    '--input-type=module',
    '-e',
    `
    import pkg from '${pkg.name}'
    import { LexeNode, LexeConnect, LexeConnectSession } from '${pkg.name}'
    import { withLexeNode } from '${pkg.name}/lexe'
    if (typeof pkg.LexeNode !== 'function' || typeof withLexeNode !== 'function') throw new Error('ESM imports failed')
    // Named imports need names Node can detect in the CommonJS entry point.
    if (LexeNode !== pkg.LexeNode || LexeConnect !== pkg.LexeConnect || LexeConnectSession !== pkg.LexeConnectSession) {
      throw new Error('ESM named imports failed')
    }
    // Native errors carry a stable code.
    try {
      new LexeNode('', 'mainnet')
      throw new Error('empty credentials were accepted')
    } catch (error) {
      if (error.code !== 'LEXE_INVALID_INPUT') throw error
    }
  `,
  ])
  // The helper entry point must import without any installed native package.
  rmSync(join(scratch, 'node_modules', scope, `${packageName}-${platform}`), {
    recursive: true,
    force: true,
  })
  run(process.execPath, [
    '-e',
    `
    const assert = require('node:assert/strict')
    const helpers = require('${pkg.name}/lexe')
    assert.equal(typeof helpers.restoreLexeSession, 'function')
    assert.throws(() => require('${pkg.name}'))
  `,
  ])
  // Compile against the installed declarations and both exports-map entries.
  mkdirSync(join(scratch, 'node_modules', '@types'), { recursive: true })
  const { cpSync } = require('node:fs')
  cpSync(
    resolve(require.resolve('@types/node/package.json', { paths: [root] }), '..'),
    join(scratch, 'node_modules', '@types', 'node'),
    { recursive: true }
  )
  cpSync(
    resolve(require.resolve('undici-types/package.json', { paths: [root] }), '..'),
    join(scratch, 'node_modules', 'undici-types'),
    { recursive: true }
  )
  writeFileSync(
    join(scratch, 'consumer.cts'),
    `
    import { LexeNode, LexeConnect, LexeAddressCheck, LexeReceivingCapabilities, OnchainPayments } from '${pkg.name}'
    import { restoreLexeSession, withLexeNode } from '${pkg.name}/lexe'
    const node = null! as LexeNode
    const address: Promise<string> = node.getExclusiveAddress()
    const checked: Promise<LexeAddressCheck> = node.checkAddress('unused')
    const capabilities: Promise<LexeReceivingCapabilities> = node.getReceivingCapabilities()
    const onchain: OnchainPayments = node
    const monitor: Promise<void> = node.onInvoiceEvents({ pollingDelaySec: 1, maxPollingSec: 1 }, () => {})
    // @ts-expect-error Spark is excluded from the Node package.
    import { SparkNode } from '${pkg.name}'
    // @ts-expect-error Arkade is excluded from the Node package.
    import { ArkadeNode } from '${pkg.name}'
    ${pkg.lni?.lexeOnly ? "// @ts-expect-error Other providers are excluded from the Lexe-only package.\n    import { LndNode } from '" + pkg.name + "'" : ''}
    void [address, checked, capabilities, onchain, monitor, LexeConnect, restoreLexeSession, withLexeNode]
  `
  )
  // ESM TypeScript consumers resolve the same declarations through named imports.
  writeFileSync(
    join(scratch, 'consumer.mts'),
    `
    import { LexeNode, LexeConnect } from '${pkg.name}'
    import { withLexeNode } from '${pkg.name}/lexe'
    const node = null! as LexeNode
    const address: Promise<string> = node.getExclusiveAddress()
    void [address, LexeConnect, withLexeNode]
  `
  )
  const tsc = require.resolve('typescript/bin/tsc', { paths: [root] })
  run(process.execPath, [
    tsc,
    '--noEmit',
    '--strict',
    '--module',
    'Node16',
    '--moduleResolution',
    'Node16',
    '--target',
    'ES2020',
    'consumer.cts',
    'consumer.mts',
  ])
  console.log(
    `Packed install OK: ${pkg.name}@${pkg.version} + ${platform}; native loading, Lexe APIs, CommonJS, ESM, types, lazy helper, no Spark/Arkade.`
  )
} finally {
  rmSync(scratch, { recursive: true, force: true })
}
