'use strict'

const binding = require('./native')
require('./lexe-events').installLexeInvoiceEvents(binding.LexeNode)
module.exports = {
  ...binding,
  LexeConnect: require('./lexe-connect').createLexeConnectClass(binding.LexeConnectSession),
}
