'use strict'

const assert = require('node:assert/strict')
const { test } = require('node:test')
const { spawnSync } = require('node:child_process')
const { resolve } = require('node:path')

const instances = []
const sessions = []
class LexeConnectSession {
  static restore(options, seed, expiresAtMs) {
    const session = {
      options, seed, expiresAtMs,
      connectionString: () => 'https://lexe.app/connect?test-only',
      acceptRedirect: (url) => ({ clientCredentials: url }),
      pollMailbox: async () => null,
      cancel() { this.closed = true },
    }
    sessions.push(session)
    return session
  }
}
class LexeNode {
  constructor(credential, network) {
    this.credential = credential
    this.network = network
    this.closed = false
    instances.push(this)
  }
  close() { this.closed = true }
}
const bindingPath = require.resolve('../native')
require.cache[bindingPath] = { id: bindingPath, filename: bindingPath, loaded: true, exports: { LexeNode, LexeConnectSession } }
const { withLexeNode, restoreLexeSession } = require('./lexe')

test('restored sessions expose the native redirect receiver and can be closed', () => {
  const options = { redirectUri: 'https://example.com/callback' }
  const seed = Buffer.alloc(32, 7)
  const session = restoreLexeSession(options, seed, 1234)
  assert.deepEqual(session.acceptRedirect('test-only-callback'), { clientCredentials: 'test-only-callback' })
  assert.deepEqual(sessions.at(-1).options, options)
  assert.equal(sessions.at(-1).seed, seed)
  assert.equal(sessions.at(-1).expiresAtMs, 1234)
  session.close()
  assert.equal(sessions.at(-1).closed, true)
})

test('wallet operations receive the result and always release the native node', async () => {
  const result = await withLexeNode('test-only', async (node) => {
    assert.equal(node.closed, false)
    assert.equal(node.network, 'testnet')
    return 42
  }, 'testnet')
  assert.equal(result, 42)
  assert.equal(instances.at(-1).closed, true)
  await assert.rejects(withLexeNode('test-only', async () => { throw new Error('operation failed') }), /operation failed/)
  assert.equal(instances.at(-1).closed, true)
})

test('importing the helper does not load the native addon', () => {
  const helper = resolve(__dirname, 'lexe.js')
  const generated = resolve(__dirname, '../native.js')
  const result = spawnSync(process.execPath, ['-e', `
    const assert = require('node:assert/strict')
    require(${JSON.stringify(helper)})
    assert.equal(require.cache[${JSON.stringify(generated)}], undefined)
  `], { encoding: 'utf8' })
  assert.equal(result.status, 0, result.stderr)
})
