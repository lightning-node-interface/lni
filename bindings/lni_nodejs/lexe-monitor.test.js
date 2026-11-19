'use strict'

// Native invoice-monitor lifecycle tests. They need an addon built with the
// offline `lexe_monitor_test` feature: run `npm run test:lexe-monitor`.
// Synthetic monitors use the same registration and close path as
// onInvoiceEventsRaw; no wallet credentials or network are involved.

const assert = require('node:assert/strict')
const { test } = require('node:test')
const { spawnSync } = require('node:child_process')
const { installLexeInvoiceEvents } = require('./lexe-events')

const addonPath = process.env.LNI_MONITOR_TEST_ADDON
const skip = addonPath ? false : 'requires LNI_MONITOR_TEST_ADDON; run `npm run test:lexe-monitor`'

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
async function until(condition, message) {
  const deadline = Date.now() + 5000
  while (!condition()) {
    if (Date.now() > deadline) throw new Error(`Timed out: ${message}`)
    await sleep(5)
  }
}
function busyWait(ms) {
  const end = Date.now() + ms
  while (Date.now() < end) {}
}

const unhandled = []
process.on('unhandledRejection', (error) => unhandled.push(error))

let LexeNode
let closeNative
if (addonPath) {
  const addon = { exports: {} }
  process.dlopen(addon, addonPath)
  LexeNode = addon.exports.LexeNode
  assert.equal(typeof LexeNode.monitorTestInstance, 'function', 'addon lacks lexe_monitor_test hooks')
  closeNative = LexeNode.prototype.close
  // Route the JS wrapper to the synthetic native monitor.
  LexeNode.prototype.onInvoiceEventsRaw = function (params, callback) {
    return this.startTestMonitor(params.intervalMs, params.count, callback)
  }
  installLexeInvoiceEvents(LexeNode)
}

test('native close stops monitors, drops queued events and releases resources', { skip }, async () => {
  const node = LexeNode.monitorTestInstance()
  const events = []
  node.startTestMonitor(1, 1000000, (payload) => events.push(JSON.parse(payload).status))
  node.startTestMonitor(1, 1000000, (payload) => events.push(JSON.parse(payload).status))
  assert.equal(node.activeMonitorCount(), 2)
  assert.equal(LexeNode.liveTestResources(), 2)
  await until(() => events.length >= 4, 'monitor events')
  // Block the event loop so both monitors queue events, then close at once.
  busyWait(100)
  closeNative.call(node)
  const delivered = events.length
  assert.equal(node.activeMonitorCount(), 0)
  await sleep(100)
  assert.equal(events.length, delivered, 'no callback may run after close')
  await until(() => LexeNode.liveTestResources() === 0, 'monitor resources released')
  closeNative.call(node)
  assert.throws(() => node.startTestMonitor(1, 1, () => {}), /closed/)
  assert.equal(LexeNode.liveTestResources(), 0)
})

test('a monitor registered in the same tick as close never calls back', { skip }, async () => {
  const node = LexeNode.monitorTestInstance()
  let calls = 0
  node.startTestMonitor(0, 10, () => { calls += 1 })
  closeNative.call(node)
  await sleep(50)
  assert.equal(calls, 0)
  await until(() => LexeNode.liveTestResources() === 0, 'monitor resources released')
})

test('a finished monitor deregisters and lets Node exit', { skip }, () => {
  const script = `
    const addon = { exports: {} }
    process.dlopen(addon, process.env.LNI_MONITOR_TEST_ADDON)
    const node = addon.exports.LexeNode.monitorTestInstance()
    const statuses = []
    node.startTestMonitor(5, 2, (payload) => statuses.push(JSON.parse(payload).status))
    process.on('exit', () => {
      console.log(JSON.stringify({ statuses, active: node.activeMonitorCount() }))
    })
  `
  const result = spawnSync(process.execPath, ['-e', script], { encoding: 'utf8', timeout: 10000 })
  assert.equal(result.status, 0, result.stderr || 'process did not exit')
  assert.deepEqual(JSON.parse(result.stdout), { statuses: ['pending', 'pending', 'success'], active: 0 })
})

test('close resolves wrapper promises without further callbacks', { skip }, async () => {
  const node = LexeNode.monitorTestInstance()
  const statuses = []
  const first = node.onInvoiceEvents({ intervalMs: 2, count: 1000000 }, (status) => statuses.push(status))
  const second = node.onInvoiceEvents({ intervalMs: 2, count: 1000000 }, (status) => statuses.push(status))
  await until(() => statuses.length >= 4, 'wrapper events')
  node.close()
  const delivered = statuses.length
  await Promise.all([first, second])
  await sleep(50)
  assert.equal(statuses.length, delivered)
  node.close()
  await assert.rejects(node.onInvoiceEvents({ intervalMs: 1, count: 1 }, () => {}), /closed/)
  await until(() => LexeNode.liveTestResources() === 0, 'monitor resources released')
})

test('a wrapper monitor resolves on its terminal event', { skip }, async () => {
  const node = LexeNode.monitorTestInstance()
  const statuses = []
  await node.onInvoiceEvents({ intervalMs: 5, count: 2 }, (status) => statuses.push(status))
  assert.deepEqual(statuses, ['pending', 'pending', 'success'])
  await until(() => node.activeMonitorCount() === 0, 'monitor deregistered')
  node.close()
})

test('stopInvoiceEvents stops one monitor and releases its resources', { skip }, async () => {
  const node = LexeNode.monitorTestInstance()
  const events = []
  const first = node.startTestMonitor(1, 1000000, (payload) => events.push(JSON.parse(payload).status))
  node.startTestMonitor(1, 1000000, () => {})
  assert.equal(node.activeMonitorCount(), 2)
  node.stopInvoiceEvents(first)
  assert.equal(node.activeMonitorCount(), 1)
  const delivered = events.length
  await sleep(50)
  assert.equal(events.length, delivered, 'no callback may run after stop')
  node.stopInvoiceEvents(first)
  closeNative.call(node)
  await until(() => LexeNode.liveTestResources() === 0, 'monitor resources released')
})

test('a throwing wrapper callback stops its native monitor', { skip }, async () => {
  const node = LexeNode.monitorTestInstance()
  await assert.rejects(
    node.onInvoiceEvents({ intervalMs: 2, count: 1000000 }, () => {
      throw new Error('callback failed')
    }),
    /callback failed/
  )
  assert.equal(node.activeMonitorCount(), 0)
  await until(() => LexeNode.liveTestResources() === 0, 'monitor resources released')
  node.close()
})

test('no unhandled rejections', { skip }, async () => {
  await sleep(20)
  assert.deepEqual(unhandled, [])
})
