# LexeConnect helpers

LNI implements the **requester** side of [LexeConnect v1](https://github.com/lexe-app/lexe-connect)
using the official `lexe-connect` Rust crate. The same implementation backs Rust
and `@sunnyln/react-native-lni-lexe`. The Lexe wallet handles approval and
credential creation.

A connection attempt creates a fresh HPKE key and one-time secret. Open the
connection URL in Lexe or display it as a QR code. Accept the callback or poll the
mailbox, then pass an approved `client_credentials` / `clientCredentials` to the
existing Lexe node constructor.

## Rust

```rust,no_run
use lni::{ApiError, lexe::{LexeConfig, LexeConnectOptions, LexeConnectSession, LexeNode}};
use std::time::Duration;

async fn connect() -> Result<Option<LexeNode>, ApiError> {
    let session = LexeConnectSession::new(LexeConnectOptions {
        scopes: vec!["read_info".into(), "receive".into()],
        account: Some("@example".into()),
        label: Some("Example application".into()),
        ..Default::default()
    })?;

    let connection_url = session.connection_string()?;
    // Open connection_url in Lexe or display a QR code. Do not log it.
    let _ = connection_url;

    let response = loop {
        if let Some(response) = session.poll_mailbox().await? {
            break response;
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
    };
    match response.client_credentials {
        Some(client_credentials) => Ok(Some(LexeNode::new(LexeConfig {
            client_credentials,
            ..Default::default()
        })?)),
        None => Ok(None), // Inspect response.error: user_rejected or other.
    }
}
```

For app callbacks, set `redirect_uri` and call
`session.accept_redirect(callback_url)`. For an HTTPS endpoint, set `post_url`
and pass the **raw encrypted request body** to `session.accept_body(body)`.
POST responses are encrypted too; plaintext responses are intentionally not
requested by these helpers.

`poll_mailbox()` performs one GET. `Ok(None)` means HTTP 404 (not delivered yet).
Other HTTP statuses, transport failures, and invalid encrypted responses return
an error and leave the session available for retry until it expires. It follows
no redirects and limits response bodies to 64 KiB.

## React Native

The ergonomic `LexeConnect` wrapper uses `number` timestamps and a discriminated
result. The generated `LexeConnectSession` is also exported for callers that
prefer native records and `bigint` timestamps. Neither API needs WebCrypto,
Node.js crypto, or JavaScript-held HPKE private keys.

```typescript
import { Linking } from 'react-native';
import { LexeConnect, LexeLniNode } from '@sunnyln/react-native-lni-lexe';

async function connectWallet(signal: AbortSignal) {
  const connection = new LexeConnect({
    scopes: ['read_info', 'receive'],
    account: '@example',
    label: 'Example application',
  });

  try {
    await Linking.openURL(connection.connectionString());
    const result = await connection.waitForResponse({ signal });
    if (result.status !== 'approved') {
      // Display rejection or result.message as appropriate for your UI.
      return undefined;
    }
    return new LexeLniNode({ clientCredentials: result.clientCredentials });
  } finally {
    connection.dispose();
  }
}
```

Omitting delivery destinations uses `https://lexe.app/mailbox`. You can instead
set `mailboxUrl` for your own HTTPS relay. `waitForResponse` polls every second
(configurable with `pollIntervalMs`, 250–5000 ms). An abort cancels the session;
a transport failure stops polling but permits retry on the same session.
A single `pollMailbox(signal?)` abort stops that poll without cancelling the
session. Call `cancel()` or `dispose()` to discard the pending request.

For app-to-app callbacks, configure an OS deep link or verified universal/app
link and use a redirect session:

```typescript
import { Linking } from 'react-native';
import { LexeConnect } from '@sunnyln/react-native-lni-lexe';
import type { LexeConnectResult } from '@sunnyln/react-native-lni-lexe';

function startRedirectConnection(
  onResult: (result: LexeConnectResult) => void,
  onError: (error: unknown) => void
) {
  const connection = new LexeConnect({
    redirectUri: 'exampleapp://lexe-connect?flow=wallet',
    scopes: ['read_info', 'receive'],
    account: '@example',
  });
  const subscription = Linking.addEventListener('url', ({ url }) => {
    if (!url.startsWith('exampleapp://lexe-connect?')) return;
    try {
      const result = connection.acceptRedirect(url);
      cleanup();
      onResult(result);
    } catch (error) {
      onError(error); // A bad callback does not consume a live request.
    }
  });
  const expiry = setTimeout(cleanup, 300_000);
  function cleanup() {
    clearTimeout(expiry);
    subscription.remove();
    connection.dispose();
  }
  // Register the listener before opening the wallet.
  void Linking.openURL(connection.connectionString()).catch((error) => {
    cleanup();
    onError(error);
  });
  return cleanup; // Call on screen unmount or cancellation.
}
```

Keep the connection instance alive while the app is backgrounded. Requests are
in memory only: if the process is killed, discard the old callback and start a
new connection. Returning from Lexe is controlled by the wallet/OS; mailbox
polling does not automatically return the user to your app.

## Options and lifecycle

- Set at most one of `redirect_uri` / `redirectUri`, `post_url` / `postUrl`, and
  `mailbox_url` / `mailboxUrl`. Request at least one scope or permission.
- `account` is shown to the user by Lexe and bound into response verification.
  Use the actual signed-in account identity, not the example value above.
- `metadata` is echoed and checked. `requester_name` / `requesterName` and the
  icon are only displayed by Lexe for independently verified requesters.
- `expires_at_ms` / `expiresAtMs` suggests a **credential** expiry in Unix
  milliseconds. Lexe lets the user change it. It does not extend request lifetime.
- `request_ttl_secs` / `requestTtlSecs` sets **request** lifetime (1–300 seconds,
  default 300), measured with a monotonic clock in Rust. Expired requests cannot
  accept responses. Cancel/drop/dispose promptly when leaving the connect flow.
- A verified approval **or rejection** consumes the request atomically. Replays
  fail, including concurrent attempts. Invalid responses do not consume it.
- Verification checks HPKE authentication, the one-time secret, account,
  metadata, and exact scopes/permissions. Redirect acceptance also checks the
  callback destination and existing query parameters, and rejects duplicate
  response parameters. Returned grants reflect protocol verification, not an
  independent lookup of the credential on the Lexe node.
- Once connected, query `get_human_bitcoin_address()` /
  `getHumanBitcoinAddress()` and show the connected wallet identity to the user.
- Connection strings and results are sensitive. Do not log them, put them in
  analytics, or store credentials in ordinary app preferences. If persistence
  is needed, use the platform's secure credential storage. Rust session/response
  debug formatting redacts sensitive content; JavaScript results are plain data.
- Budget fields are not exposed because LexeConnect currently rejects them.

## Validation and native builds

```sh
cargo test -p lni --no-default-features --features rustls-tls lexe::connect::tests --locked
# Optional live relay test: only exchanges an encrypted synthetic rejection.
cargo test -p lni --no-default-features --features rustls-tls public_mailbox_round_trip --locked -- --ignored
cargo check -p react-native-lni-lexe --locked
cd bindings/react-native-lexe
npm run typecheck
npm test
npm run pack:dry-run
```

Changes to the Rust bridge require regenerated TypeScript/C++ bindings and
rebuilt iOS/Android libraries before packaging, following the React Native
package's existing native build workflow. Existing prebuilt binaries from an
older release do not contain these new methods. A native app rebuild is required;
a JavaScript-only OTA update cannot add them. Expo Go is not supported.
