"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.restoreLexeSession = restoreLexeSession;
exports.withLexeNode = withLexeNode;
// Keep importing server-side helpers cheap and independent of native artifacts.
const getBinding = () => {
    const binding = require('../native');
    return binding;
};
/**
 * Restore one short-lived Connect request across stateless Node invocations.
 *
 * Every restore from the same seed can accept the same wallet response again.
 * Before acting on a response, the caller must atomically claim the attempt in
 * durable storage (for example, a compare-and-set on its attempt ID) and reject
 * a second claim. Keep the seed encrypted, bound to the initiating user and
 * organization, and delete it after the claim or expiry.
 */
function restoreLexeSession(options, seed, expiresAtMs) {
    const session = getBinding().LexeConnectSession.restore(options, seed, expiresAtMs);
    return {
        connectionString: () => session.connectionString(),
        acceptRedirect: (url) => session.acceptRedirect(url),
        pollMailbox: () => session.pollMailbox(),
        close: () => session.cancel(),
    };
}
/** Open a stateless wallet for one operation and release it on every exit path. */
async function withLexeNode(clientCredentials, fn, network = 'mainnet') {
    const node = new (getBinding().LexeNode)(clientCredentials, network);
    try {
        return await fn(node);
    }
    finally {
        node.close();
    }
}
