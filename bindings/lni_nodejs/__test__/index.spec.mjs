import test from 'ava'
import { validateMnemonic } from '@scure/bip39'
import { wordlist } from '@scure/bip39/wordlists/english.js'
import { BlinkNode, generateMnemonic } from '../index.js'

for (const count of [12, 24]) {
  test(`native mnemonic generation preserves the ${count}-word BIP39 contract`, (t) => {
    const phrase = generateMnemonic(count)
    t.is(phrase.split(' ').length, count)
    t.true(validateMnemonic(phrase, wordlist))
  })
}

test('native mnemonic generation preserves the default length', (t) => {
  t.is(generateMnemonic().split(' ').length, 12)
})

test('native node configuration keeps spending credentials redacted', (t) => {
  const node = new BlinkNode({ apiKey: 'sanitized-test-key', baseUrl: 'https://example.invalid' })
  t.is(node.getConfig().apiKey, '<redacted>')
})

test('native invoice subscription rejects a non-function callback before polling', (t) => {
  const node = new BlinkNode({ apiKey: 'sanitized-test-key', baseUrl: 'https://example.invalid' })
  t.throws(() => node.onInvoiceEvents({ maxPollingSec: 0 }, null))
})
