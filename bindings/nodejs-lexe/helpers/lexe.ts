import type { LexeConnectOptions, LexeNode } from '../generated'

// Keep importing server-side helpers cheap and independent of native artifacts.
// Load through the package entry point, so errors carry `error.code` and
// LexeNode has its JavaScript helpers.
const getBinding = () => {
  const binding: typeof import('../generated') = require('../index')
  return binding
}

/**
 * Restore one short-lived Connect request across stateless Node invocations.
 *
 * Every restore from the same seed can accept the same wallet response again.
 * Before acting on a response, the caller must atomically claim the attempt in
 * durable storage (for example, a compare-and-set on its attempt ID) and reject
 * a second claim. Keep the seed encrypted, bound to the initiating user and
 * organization, and delete it after the claim or expiry.
 */
export function restoreLexeSession(
  options: LexeConnectOptions,
  seed: Buffer,
  expiresAtMs: number,
) {
  const session = getBinding().LexeConnectSession.restore(options, seed, expiresAtMs)
  return {
    connectionString: () => session.connectionString(),
    acceptRedirect: (url: string) => session.acceptRedirect(url),
    pollMailbox: () => session.pollMailbox(),
    close: () => session.cancel(),
  }
}

/** Open a stateless wallet for one operation and release it on every exit path. */
export async function withLexeNode<T>(
  clientCredentials: string,
  fn: (node: LexeNode) => Promise<T>,
  network = 'mainnet',
): Promise<T> {
  const node = new (getBinding().LexeNode)(clientCredentials, network)
  try {
    return await fn(node)
  } finally {
    node.close()
  }
}
