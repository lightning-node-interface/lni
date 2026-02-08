const BareHeaders = globalThis.Headers;

if (typeof BareHeaders !== 'function') {
  throw new Error('globalThis.Headers is not available in this runtime.');
}

export default BareHeaders;
