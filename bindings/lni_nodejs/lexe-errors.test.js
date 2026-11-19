'use strict'

const assert = require('node:assert/strict')
const { test } = require('node:test')
const { installErrorCodes, withCode } = require('./lexe-errors')

// Models a native class: it throws "CODE: message" errors from the
// constructor, a static factory, a sync method and an async method.
class Native {
  constructor(value) {
    if (value === 'bad') throw new Error('LEXE_INVALID_INPUT: Invalid value')
    this.value = value
  }
  static restore(value) {
    if (value === 'bad') throw new Error('LEXE_CONNECT_EXPIRED: LexeConnect session expired')
    return new Native(value)
  }
  read() {
    throw new Error('LEXE_NODE_CLOSED: Lexe node closed')
  }
  async pay() {
    throw new Error('LEXE_PAYMENT_INDETERMINATE: Lexe payment outcome is unknown')
  }
  async ok() {
    return this.value
  }
}

const Coded = installErrorCodes(Native)

test('moves the code from the message to error.code everywhere', async () => {
  assert.throws(() => new Coded('bad'), { code: 'LEXE_INVALID_INPUT', message: 'Invalid value' })
  assert.throws(() => Coded.restore('bad'), { code: 'LEXE_CONNECT_EXPIRED', message: 'LexeConnect session expired' })
  const instance = new Coded('fine')
  assert.throws(() => instance.read(), { code: 'LEXE_NODE_CLOSED', message: 'Lexe node closed' })
  await assert.rejects(instance.pay(), {
    code: 'LEXE_PAYMENT_INDETERMINATE',
    message: 'Lexe payment outcome is unknown',
  })
  assert.equal(await instance.ok(), 'fine')
})

test('keeps the native class identity', () => {
  const instance = new Coded('fine')
  assert.ok(instance instanceof Native)
  assert.ok(instance instanceof Coded)
  assert.ok(Coded.restore('fine') instanceof Coded)
  class Sub extends Coded {}
  assert.ok(new Sub('fine') instanceof Sub)
})

test('leaves errors without a code untouched', () => {
  const plain = new Error('InvalidInput: something')
  assert.equal(withCode(plain), plain)
  assert.equal(plain.code, undefined)
  assert.equal(plain.message, 'InvalidInput: something')
  assert.equal(withCode('not an error'), 'not an error')
})

test('wraps statics that cannot be replaced in place', () => {
  class Frozen {}
  Object.defineProperty(Frozen, 'restore', {
    value() {
      throw new Error('LEXE_INVALID_INPUT: LexeConnect seed must be 32 bytes')
    },
    writable: false,
    configurable: false,
  })
  const Coded = installErrorCodes(Frozen)
  assert.throws(() => Coded.restore(), { code: 'LEXE_INVALID_INPUT', message: 'LexeConnect seed must be 32 bytes' })
  assert.equal(Coded.name, 'Frozen')
})
