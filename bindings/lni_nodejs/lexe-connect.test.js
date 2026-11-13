'use strict'

const assert = require('node:assert/strict')
const { test } = require('node:test')
const { createLexeConnectClass } = require('./lexe-connect')

class Session {
  constructor(options) {
    this.options = options
    this.polls = []
    this.cancelled = false
  }
  connectionString() { return 'lexe://example' }
  acceptRedirect() { return { error: 'user_rejected', scopes: [], permissions: [] } }
  acceptBody(body) {
    assert.deepEqual([...body], [2, 3])
    return { clientCredentials: 'test-only', scopes: ['receive'], permissions: [] }
  }
  async pollMailbox() { return this.polls.shift() }
  cancel() { this.cancelled = true }
}

const LexeConnect = createLexeConnectClass(Session)

test('maps verified responses and copies only the supplied byte view', () => {
  const connect = new LexeConnect({ scopes: ['receive'] })
  assert.equal(connect.connectionString(), 'lexe://example')
  assert.equal(connect.acceptRedirect('https://example.test').status, 'rejected')
  const bytes = new Uint8Array([1, 2, 3, 4]).subarray(1, 3)
  assert.equal(connect.acceptBody(bytes).status, 'approved')
  connect.dispose()
  assert.throws(() => connect.connectionString(), /disposed/)
})

test('waits for response and rejects a second waiter', async () => {
  const native = new Session({})
  native.polls.push(null, { error: 'user_rejected', scopes: [], permissions: [] })
  const connect = new LexeConnect({}, native)
  const pending = connect.waitForResponse({ pollIntervalMs: 250 })
  await assert.rejects(connect.waitForResponse(), /already waiting/)
  assert.equal((await pending).status, 'rejected')
})

test('abort cancels the native session', async () => {
  const native = new Session({})
  native.pollMailbox = () => new Promise(() => {})
  const connect = new LexeConnect({}, native)
  const controller = new AbortController()
  const pending = connect.waitForResponse({ signal: controller.signal, pollIntervalMs: 250 })
  controller.abort()
  await assert.rejects(pending, { name: 'AbortError' })
  assert.equal(native.cancelled, true)
})
