import type { LexeConnectOptions, LexeNode } from '../generated';
/**
 * Restore one short-lived Connect request across stateless Node invocations.
 *
 * Every restore from the same seed can accept the same wallet response again.
 * Before acting on a response, the caller must atomically claim the attempt in
 * durable storage (for example, a compare-and-set on its attempt ID) and reject
 * a second claim. Keep the seed encrypted, bound to the initiating user and
 * organization, and delete it after the claim or expiry.
 */
export declare function restoreLexeSession(options: LexeConnectOptions, seed: Buffer, expiresAtMs: number): {
    connectionString: () => string;
    acceptRedirect: (url: string) => import("../generated").LexeConnectResponse;
    pollMailbox: () => Promise<import("../generated").LexeConnectResponse | null>;
    close: () => void;
};
/** Open a stateless wallet for one operation and release it on every exit path. */
export declare function withLexeNode<T>(clientCredentials: string, fn: (node: LexeNode) => Promise<T>, network?: string): Promise<T>;
