'use strict'

const assert = require('node:assert/strict')
const { test } = require('node:test')
const { installLexeInvoiceEvents } = require('./lexe-events')

class LexeNode {
  onInvoiceEventsRaw(_params, callback) {
    callback(JSON.stringify({ status: 'pending', transaction: { type_: 'incoming', amount_msats: 1000 } }))
    callback(JSON.stringify({ status: 'success', transaction: { type_: 'incoming', amount_msats: 1000 } }))
  }
}

installLexeInvoiceEvents(LexeNode)

test('passes event status and camel-case LNI transaction to callback', async () => {
  const events = []
  await new LexeNode().onInvoiceEvents({ paymentHash: 'test' }, (status, transaction) => {
    events.push({ status, transaction })
  })
  assert.deepEqual(events, [
    { status: 'pending', transaction: { type: 'incoming', amountMsats: 1000 } },
    { status: 'success', transaction: { type: 'incoming', amountMsats: 1000 } },
  ])
})

// Models the native contract: close() stops monitors; a closed node rejects registration.
class ClosableNode {
  constructor() {
    this.callbacks = []
    this.closed = false
    this.closeCalls = 0
  }
  onInvoiceEventsRaw(_params, callback) {
    if (this.closed) throw new Error('Lexe node closed')
    this.callbacks.push(callback)
  }
  close() {
    this.closeCalls += 1
    this.closed = true
  }
}

installLexeInvoiceEvents(ClosableNode)

const pending = JSON.stringify({ status: 'pending', transaction: null })

test('close resolves every active monitor and ignores later native events', async () => {
  const node = new ClosableNode()
  const statuses = []
  const first = node.onInvoiceEvents({}, (status) => statuses.push(status))
  const second = node.onInvoiceEvents({}, (status) => statuses.push(status))
  node.callbacks.forEach((callback) => callback(pending))
  node.close()
  node.callbacks.forEach((callback) => callback(pending))
  await Promise.all([first, second])
  assert.deepEqual(statuses, ['pending', 'pending'])
  node.close()
  assert.equal(node.closeCalls, 2)
  await assert.rejects(node.onInvoiceEvents({}, () => {}), /closed/)
})

test('a settled monitor is not resolved again by close', async () => {
  const node = new ClosableNode()
  const statuses = []
  const done = node.onInvoiceEvents({}, (status) => statuses.push(status))
  node.callbacks[0](JSON.stringify({ status: 'failure', transaction: null }))
  await done
  node.close()
  node.callbacks[0](pending)
  assert.deepEqual(statuses, ['failure'])
})

class FailingCloseNode {
  onInvoiceEventsRaw() {}
  close() {
    throw new Error('native close failed')
  }
}
installLexeInvoiceEvents(FailingCloseNode)

test('monitors still settle if native close throws', async () => {
  const node = new FailingCloseNode()
  const done = node.onInvoiceEvents({}, () => {})
  assert.throws(() => node.close(), /native close failed/)
  await done
})
