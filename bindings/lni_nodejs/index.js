'use strict'

const native = require('./native')
const { installErrorCodes } = require('./lexe-errors')

// Both install in place, before the spread below copies the native classes.
const LexeNode = installErrorCodes(native.LexeNode)
require('./lexe-events').installLexeInvoiceEvents(native.LexeNode)
const LexeConnectSession = installErrorCodes(native.LexeConnectSession)
const LexeConnect = require('./lexe-connect').createLexeConnectClass(LexeConnectSession)

// Spread require() itself and list the rest by name: Node's ESM loader detects
// both, so `import { LexeNode } from '...'` works. Spreading a variable hides
// every name from it.
module.exports = {
  ...require('./native'),
  LexeNode,
  LexeConnectSession,
  LexeConnect,
}
