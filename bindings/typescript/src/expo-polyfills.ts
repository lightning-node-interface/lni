import { registerSha256DigestFallback } from './internal/sha256.js';

type MessageListener = (event: MessageEvent) => void;

const scheduleMessage = (callback: () => void): void => {
  const scheduler = globalThis.setImmediate ?? ((fn: () => void) => setTimeout(fn, 0));
  scheduler(callback);
};

class MessagePortPolyfill {
  onmessage: ((this: MessagePort, event: MessageEvent) => unknown) | null = null;

  private readonly listeners = new Set<MessageListener>();
  private readonly listenerMap = new Map<EventListenerOrEventListenerObject, MessageListener>();
  private pairedPort: MessagePortPolyfill | undefined;
  private isClosed = false;

  pairWith(port: MessagePortPolyfill): void {
    this.pairedPort = port;
  }

  addEventListener(type: string, listener: EventListenerOrEventListenerObject | null): void {
    if (type !== 'message' || !listener) {
      return;
    }

    const messageListener = toMessageListener(listener);
    this.listenerMap.set(listener, messageListener);
    this.listeners.add(messageListener);
  }

  removeEventListener(type: string, listener: EventListenerOrEventListenerObject | null): void {
    if (type !== 'message' || !listener) {
      return;
    }

    const messageListener = this.listenerMap.get(listener);
    if (!messageListener) {
      return;
    }

    this.listeners.delete(messageListener);
    this.listenerMap.delete(listener);
  }

  postMessage(value: unknown): void {
    if (this.isClosed) {
      return;
    }

    const target = this.pairedPort;
    if (!target || target.isClosed) {
      return;
    }

    scheduleMessage(() => target.dispatchMessage(value));
  }

  start(): void {
    // Message delivery is always active for this minimal React Native shim.
  }

  close(): void {
    this.isClosed = true;
    this.listeners.clear();
    this.listenerMap.clear();
    this.onmessage = null;
  }

  private dispatchMessage(data: unknown): void {
    if (this.isClosed) {
      return;
    }

    const event = { data } as MessageEvent;
    this.onmessage?.call(this as unknown as MessagePort, event);

    for (const listener of this.listeners) {
      listener(event);
    }
  }
}

function toMessageListener(listener: EventListenerOrEventListenerObject): MessageListener {
  if (typeof listener === 'function') {
    return listener as MessageListener;
  }

  return (event) => listener.handleEvent(event);
}

class MessageChannelPolyfill {
  readonly port1: MessagePort;
  readonly port2: MessagePort;

  constructor() {
    const port1 = new MessagePortPolyfill();
    const port2 = new MessagePortPolyfill();

    port1.pairWith(port2);
    port2.pairWith(port1);

    this.port1 = port1 as unknown as MessagePort;
    this.port2 = port2 as unknown as MessagePort;
  }
}

export function installExpoPolyfills(): void {
  if (typeof globalThis.MessageChannel !== 'function') {
    globalThis.MessageChannel = MessageChannelPolyfill as unknown as typeof MessageChannel;
  }

  registerSha256DigestFallback(async (bytes) => {
    const Crypto = await import('expo-crypto');
    return Crypto.digest(Crypto.CryptoDigestAlgorithm.SHA256, bytes as BufferSource);
  });
}

installExpoPolyfills();
