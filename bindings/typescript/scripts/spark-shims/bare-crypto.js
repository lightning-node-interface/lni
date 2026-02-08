const cryptoImpl =
  globalThis.crypto ??
  {
    getRandomValues(array) {
      throw new Error('globalThis.crypto is not available in this browser runtime.');
    },
    subtle: undefined,
  };

export const webcrypto = cryptoImpl;
export default { webcrypto };
