declare module 'expo-crypto' {
  export const CryptoDigestAlgorithm: {
    SHA256: string;
  };

  export function digest(algorithm: string, data: BufferSource): Promise<ArrayBuffer>;
}
