import { afterEach, describe, expect, it, vi } from 'vitest';

const originalMessageChannelDescriptor = Object.getOwnPropertyDescriptor(
  globalThis,
  'MessageChannel'
);

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

function flushScheduledMessage(): Promise<void> {
  return new Promise((resolve) => {
    const scheduler = globalThis.setImmediate ?? ((fn: () => void) => setTimeout(fn, 0));
    scheduler(resolve);
  });
}

afterEach(() => {
  vi.resetModules();
  restoreMessageChannel();
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
});
