const nativeFetch =
  typeof globalThis.fetch === 'function'
    ? globalThis.fetch.bind(globalThis)
    : null;

const bareFetch = (input, init) => {
  if (!nativeFetch) {
    throw new Error('globalThis.fetch is not available in this browser runtime.');
  }
  return nativeFetch(input, init);
};

export default bareFetch;
