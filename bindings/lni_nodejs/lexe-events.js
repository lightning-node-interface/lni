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

function installLexeInvoiceEvents(LexeNode) {
  LexeNode.prototype.onInvoiceEvents = function onInvoiceEvents(params, callback) {
    if (typeof callback !== 'function') throw new TypeError('callback must be a function')
    return new Promise((resolve, reject) => {
      try {
        this.onInvoiceEventsRaw(params, (payload) => {
          try {
            const event = JSON.parse(payload)
            callback(event.status, transactionFromRust(event.transaction))
            if (event.status === 'success' || event.status === 'failure') resolve()
          } catch (error) {
            reject(error)
          }
        })
      } catch (error) {
        reject(error)
      }
    })
  }
}

module.exports = { installLexeInvoiceEvents }
