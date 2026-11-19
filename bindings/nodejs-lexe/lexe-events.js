'use strict'

function transactionFromRust(transaction) {
  if (transaction == null) return undefined
  const result = {}
  for (const [key, value] of Object.entries(transaction)) {
    const name = key === 'type_' ? 'type' : key.replace(/_([a-z])/g, (_, letter) => letter.toUpperCase())
    result[name] = value
  }
  return result
}

// Active monitors per node. close() settles them, so no promise is left pending.
const activeMonitors = new WeakMap()

function installLexeInvoiceEvents(LexeNode) {
  const closeNative = LexeNode.prototype.close

  /**
   * Native close stops polling and discards queued events. Promises from
   * onInvoiceEvents then resolve without a further callback.
   */
  LexeNode.prototype.close = function close() {
    try {
      return closeNative.apply(this, arguments)
    } finally {
      const monitors = activeMonitors.get(this)
      if (monitors) {
        activeMonitors.delete(this)
        monitors.forEach((stop) => stop())
      }
    }
  }

  LexeNode.prototype.onInvoiceEvents = function onInvoiceEvents(params, callback) {
    if (typeof callback !== 'function') {
      const error = new TypeError('callback must be a function')
      error.code = 'LEXE_INVALID_INPUT'
      throw error
    }
    const node = this
    return new Promise((resolve, reject) => {
      let monitors = activeMonitors.get(node)
      if (!monitors) {
        monitors = new Set()
        activeMonitors.set(node, monitors)
      }
      let settled = false
      // The native monitor's id, and whether it must stop as soon as it is known
      // (the native callback can run before onInvoiceEventsRaw returns).
      let monitorId
      let stopNative = false
      const stopMonitor = () => {
        stopNative = true
        if (monitorId === undefined || typeof node.stopInvoiceEvents !== 'function') return
        try {
          node.stopInvoiceEvents(monitorId)
        } catch (_) {
          // Already finished or closed: nothing left to stop.
        }
      }
      const settle = (error) => {
        if (settled) return
        settled = true
        monitors.delete(stop)
        if (error) {
          // A throwing callback ends the monitor; native polling must end too.
          stopMonitor()
          reject(error)
        } else resolve()
      }
      const stop = () => settle()
      monitors.add(stop)
      try {
        monitorId = node.onInvoiceEventsRaw(params, (payload) => {
          if (settled) return
          try {
            const event = JSON.parse(payload)
            callback(event.status, transactionFromRust(event.transaction))
            if (event.status === 'success' || event.status === 'failure') settle()
          } catch (error) {
            settle(error)
          }
        })
        if (stopNative) stopMonitor()
      } catch (error) {
        settle(error)
      }
    })
  }
}

module.exports = { installLexeInvoiceEvents }
