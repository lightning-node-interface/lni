import { afterEach, describe, expect, it, vi } from 'vitest';

const expoCrypto = vi.hoisted(() => ({
  digest: vi.fn(),
  loadCount: 0,
}));

vi.mock('expo-crypto', () => {
  expoCrypto.loadCount += 1;

  return {
    CryptoDigestAlgorithm: { SHA256: 'SHA-256' },
    digest: expoCrypto.digest,
  };
});

const originalMessageChannelDescriptor = Object.getOwnPropertyDescriptor(
  globalThis,
  'MessageChannel'
);
const originalCryptoDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'crypto');

function restoreMessageChannel(): void {
  if (originalMessageChannelDescriptor) {
    Object.defineProperty(globalThis, 'MessageChannel', originalMessageChannelDescriptor);
  } else {
    delete (globalThis as { MessageChannel?: typeof MessageChannel }).MessageChannel;
  }
}

function clearMessageChannel(): void {
  Object.defineProperty(globalThis, 'MessageChannel', {
    configurable: true,
    writable: true,
    value: undefined,
  });
}

function restoreCrypto(): void {
  if (originalCryptoDescriptor) {
    Object.defineProperty(globalThis, 'crypto', originalCryptoDescriptor);
  } else {
    delete (globalThis as { crypto?: Crypto }).crypto;
  }
}

function clearCrypto(): void {
  Object.defineProperty(globalThis, 'crypto', {
    configurable: true,
    writable: true,
    value: undefined,
  });
}

function flushScheduledMessage(): Promise<void> {
  return new Promise((resolve) => {
    const scheduler = globalThis.setImmediate ?? ((fn: () => void) => setTimeout(fn, 0));
    scheduler(resolve);
  });
}

afterEach(() => {
  vi.resetModules();
  vi.clearAllMocks();
  restoreMessageChannel();
  restoreCrypto();
});

describe('expo-polyfills', () => {
  it('installs a MessageChannel shim when imported and MessageChannel is missing', async () => {
    clearMessageChannel();

    await import('../expo-polyfills.js');

    expect(typeof globalThis.MessageChannel).toBe('function');

    const channel = new MessageChannel();
    const messages: unknown[] = [];
    const listener = (event: MessageEvent) => messages.push(event.data);

    channel.port1.addEventListener('message', listener);
    channel.port2.postMessage('first');
    await flushScheduledMessage();

    channel.port1.removeEventListener('message', listener);
    channel.port2.postMessage('second');
    await flushScheduledMessage();

    expect(messages).toEqual(['first']);
  });

  it('does not replace an existing MessageChannel implementation', async () => {
    const ExistingMessageChannel = class {};
    Object.defineProperty(globalThis, 'MessageChannel', {
      configurable: true,
      writable: true,
      value: ExistingMessageChannel,
    });

    const { installExpoPolyfills } = await import('../expo-polyfills.js');
    installExpoPolyfills();

    expect(globalThis.MessageChannel).toBe(ExistingMessageChannel);
  });

  it('loads expo-crypto statically and preserves the SHA-256 fallback result', async () => {
    clearCrypto();
    const bytes = Uint8Array.from([1, 2, 3]);
    expoCrypto.digest.mockResolvedValueOnce(Uint8Array.from([0xab, 0xcd]).buffer);

    await import('../expo-polyfills.js');

    expect(expoCrypto.loadCount).toBe(1);

    const { sha256Hex } = await import('../internal/sha256.js');
    await expect(sha256Hex(bytes)).resolves.toBe('abcd');
    expect(expoCrypto.digest).toHaveBeenCalledWith('SHA-256', bytes);
  });
});
