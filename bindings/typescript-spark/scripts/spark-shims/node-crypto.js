function assertWebCrypto() {
  if (!globalThis.crypto?.getRandomValues) {
    throw new Error('Web Crypto API is required for Spark browser runtime.');
  }
}

export function randomFillSync(array) {
  assertWebCrypto();
  globalThis.crypto.getRandomValues(array);
  return array;
}

export default {
  randomFillSync,
};
