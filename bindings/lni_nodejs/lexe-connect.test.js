'use strict'

const assert = require('node:assert/strict')
const { test } = require('node:test')
const { spawnSync } = require('node:child_process')
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

function deferred() {
  let resolve
  let reject
  const promise = new Promise((res, rej) => { resolve = res; reject = rej })
  return { promise, resolve, reject }
}

// Models native poll_mailbox: a verified response consumes the session.
class DeferredSession extends Session {
  constructor() {
    super({})
    this.requests = []
    this.completed = false
  }
  pollMailbox() {
    if (this.completed) return Promise.reject(new Error('LexeConnect session already completed'))
    const request = deferred()
    this.requests.push(request)
    return request.promise.then((value) => {
      if (value) this.completed = true
      return value
    })
  }
}

const approval = { clientCredentials: 'test-only', scopes: ['receive'], permissions: [] }

test('an approval arriving after an aborted poll is returned by the next poll', async () => {
  const native = new DeferredSession()
  const connect = new LexeConnect({}, native)
  const controller = new AbortController()
  const aborted = connect.pollMailbox(controller.signal)
  await Promise.resolve()
  controller.abort()
  await assert.rejects(aborted, { name: 'AbortError' })
  native.requests[0].resolve(approval)
  await new Promise((resolve) => setImmediate(resolve))
  const next = await connect.pollMailbox()
  assert.equal(next.status, 'approved')
  assert.equal(native.requests.length, 1, 'the retained response must not start another native poll')
  await assert.rejects(connect.pollMailbox(), /already completed/)
})

test('a poll after an abort joins the in-flight native request', async () => {
  const native = new DeferredSession()
  const connect = new LexeConnect({}, native)
  const controller = new AbortController()
  const aborted = connect.pollMailbox(controller.signal)
  await Promise.resolve()
  controller.abort()
  await assert.rejects(aborted, { name: 'AbortError' })
  const retry = connect.pollMailbox()
  native.requests[0].resolve(approval)
  assert.equal((await retry).status, 'approved')
  assert.equal(native.requests.length, 1)
})

test('an abandoned poll failure is not unhandled and the next poll retries', async () => {
  const unhandled = []
  const onUnhandled = (error) => unhandled.push(error)
  process.on('unhandledRejection', onUnhandled)
  try {
    const native = new DeferredSession()
    const connect = new LexeConnect({}, native)
    const controller = new AbortController()
    const aborted = connect.pollMailbox(controller.signal)
    await Promise.resolve()
    controller.abort()
    await assert.rejects(aborted, { name: 'AbortError' })
    native.requests[0].reject(new Error('LexeConnect mailbox request failed'))
    await new Promise((resolve) => setImmediate(resolve))
    const retry = connect.pollMailbox()
    await Promise.resolve()
    assert.equal(native.requests.length, 2)
    native.requests[1].resolve(null)
    assert.equal(await retry, undefined)
    assert.deepEqual(unhandled, [])
  } finally {
    process.removeListener('unhandledRejection', onUnhandled)
  }
})

test('a response verified before cancel took effect is not discarded', async () => {
  const native = new DeferredSession()
  const connect = new LexeConnect({}, native)
  const waiting = connect.waitForResponse({ pollIntervalMs: 250 })
  await Promise.resolve()
  await Promise.resolve()
  connect.cancel()
  await assert.rejects(waiting, { name: 'AbortError' })
  assert.equal(native.cancelled, true)
  native.requests[0].resolve(approval)
  await new Promise((resolve) => setImmediate(resolve))
  assert.equal((await connect.pollMailbox()).status, 'approved')
  connect.dispose()
})

test('dispose releases a retained response', async () => {
  const native = new DeferredSession()
  const connect = new LexeConnect({}, native)
  const controller = new AbortController()
  const aborted = connect.pollMailbox(controller.signal)
  await Promise.resolve()
  controller.abort()
  await assert.rejects(aborted, { name: 'AbortError' })
  native.requests[0].resolve(approval)
  await new Promise((resolve) => setImmediate(resolve))
  connect.dispose()
  assert.equal(connect.retained, undefined)
  await assert.rejects(connect.pollMailbox(), /disposed/)
})

test('works without global AbortController or AbortSignal (Node < 15)', () => {
  const script = `
    delete globalThis.AbortController
    delete globalThis.AbortSignal
    const assert = require('node:assert/strict')
    const { createLexeConnectClass } = require(${JSON.stringify(require.resolve('./lexe-connect'))})
    class Session {
      constructor() { this.cancelled = false; this.polls = 0 }
      connectionString() { return 'lexe://example' }
      pollMailbox() { this.polls += 1; return this.polls < 3 ? Promise.resolve(null) : new Promise(() => {}) }
      cancel() { this.cancelled = true }
    }
    const LexeConnect = createLexeConnectClass(Session)
    const native = new Session()
    const connect = new LexeConnect({}, native)
    const pending = connect.waitForResponse({ pollIntervalMs: 250 })
    setTimeout(() => connect.cancel(), 600)
    pending.then(() => process.exit(2), (error) => {
      assert.equal(error.name, 'AbortError')
      assert.equal(native.cancelled, true)
      assert.equal(native.polls, 3)
      console.log('ok')
    })
  `
  const result = spawnSync(process.execPath, ['-e', script], { encoding: 'utf8', timeout: 10000 })
  assert.equal(result.status, 0, result.stderr)
  assert.equal(result.stdout.trim(), 'ok')
})

function codedError(code, message) {
  const error = new Error(message)
  error.code = code
  return error
}

test('waitForResponse retries failed mailbox requests until a response arrives', async () => {
  const native = new Session({})
  const outcomes = [
    codedError('LEXE_CONNECT_UNAVAILABLE', 'LexeConnect mailbox request failed'),
    codedError('LEXE_CONNECT_UNAVAILABLE', 'LexeConnect mailbox request failed'),
    { error: 'user_rejected', scopes: [], permissions: [] },
  ]
  native.pollMailbox = async () => {
    const next = outcomes.shift()
    if (next instanceof Error) throw next
    return next
  }
  const connect = new LexeConnect({}, native)
  const started = Date.now()
  const result = await connect.waitForResponse({ pollIntervalMs: 250 })
  assert.equal(result.status, 'rejected')
  assert.equal(outcomes.length, 0)
  // Backoff doubles from the poll interval: 500 ms, then 1000 ms.
  assert.ok(Date.now() - started >= 1400)
})

test('waitForResponse stops on errors that are not transient', async () => {
  const native = new Session({})
  let polls = 0
  native.pollMailbox = async () => {
    polls += 1
    throw codedError('LEXE_CONNECT_EXPIRED', 'LexeConnect session expired')
  }
  const connect = new LexeConnect({}, native)
  await assert.rejects(connect.waitForResponse({ pollIntervalMs: 250 }), { code: 'LEXE_CONNECT_EXPIRED' })
  assert.equal(polls, 1)
})

test('abort stops a retry backoff', async () => {
  const native = new Session({})
  native.pollMailbox = async () => {
    throw codedError('LEXE_CONNECT_UNAVAILABLE', 'LexeConnect mailbox request failed')
  }
  const connect = new LexeConnect({}, native)
  const controller = new AbortController()
  const pending = connect.waitForResponse({ signal: controller.signal, pollIntervalMs: 5000 })
  setTimeout(() => controller.abort(), 20)
  await assert.rejects(pending, { name: 'AbortError' })
  assert.equal(native.cancelled, true)
})
