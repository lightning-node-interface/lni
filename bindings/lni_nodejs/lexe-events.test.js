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
