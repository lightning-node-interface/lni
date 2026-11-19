# Invoice event statuses

`on_invoice_events` (`onInvoiceEvents` in the bindings) watches one invoice and
calls back with `success`, `pending` or `failure`. Every provider and every
binding follows the same rules, matching the TypeScript package's
`pollInvoiceEvents` (`bindings/typescript/src/internal/polling.ts`).

| Status | When | Polling |
| --- | --- | --- |
| `success` | The provider reports the invoice settled. | Stops. |
| `pending` | The invoice is not settled yet, **or a lookup failed** (HTTP error, timeout, provider outage, unparseable response). A failed lookup carries no transaction. | Continues after `polling_delay_sec`. |
| `failure` | `max_polling_sec` elapsed without settlement, the request cannot be monitored (no payment hash or search term, non-positive timings, or a provider without invoice lookup), or the provider itself reports the payment failed (Lexe `Failed`). | Stops. |

A failed lookup is never reported as `failure`: one network error says nothing
about the invoice, and consumers often treat `failure` as final (for example by
cancelling an order or closing a Tor circuit). A persistent error, such as a
revoked credential, therefore shows as `pending` until `max_polling_sec`, then
as `failure`. Check credentials separately (for example with `get_info`) when a
monitor never leaves `pending`.

Spark is event-driven rather than polled. It reports `success` when the
settlement event arrives and `failure` when `max_polling_sec` elapses or the SDK
event stream ends, after a final lookup.
