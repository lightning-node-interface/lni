# Error Normalization

This document maps node-specific errors into the LNI `NwcError` surface. The goal is for consumers to branch on `NwcError.nwcCode` consistently across node adapters, while adapter implementations keep provider-specific status, code, message, and raw body available for debugging.

Sources checked on 2026-07-07:

- NWC error code reference: https://docs.nwc.dev/reference-api/error-codes
- Strike API error reference: https://docs.strike.me/api/
- Blink error handling: https://dev.blink.sv/api/errors
- Blink GraphQL API reference: https://dev.blink.sv/public-api-reference.html
- Blink Lightning send tutorial: https://dev.blink.sv/api/btc-ln-send
- Core Lightning REST documentation: https://docs.corelightning.org/docs/rest
- Core Lightning `pay` reference: https://docs.corelightning.org/reference/pay
- Core Lightning `invoice` reference: https://docs.corelightning.org/reference/invoice
- LND `SendPaymentV2` reference: https://lightning.engineering/api-docs/api/lnd/router/send-payment-v2/
- phoenixd API reference: https://phoenix.acinq.co/server/api
- phoenixd source: https://github.com/ACINQ/phoenixd

## Canonical Codes

`bindings/typescript/src/errors.ts` currently exposes these standard NWC-style codes:

| LNI/NWC code | Meaning for adapters |
| --- | --- |
| `RATE_LIMITED` | Provider rejected the request because the client is sending too fast or has hit a short-term attempt limit. |
| `NOT_IMPLEMENTED` | The requested method is intentionally unsupported by the adapter or provider. |
| `INSUFFICIENT_BALANCE` | The wallet cannot cover the amount, route fee, reserve, or provider fee. |
| `PAYMENT_FAILED` | LNI extension for a terminal send failure, invalid invoice, expired invoice, route failure, or payment-specific failure. |
| `NOT_FOUND` | LNI extension for a missing provider resource or missing transaction. |
| `QUOTA_EXCEEDED` | A spending quota, provider amount limit, transaction count limit, or configured account limit blocks the operation. |
| `RESTRICTED` | Credentials are valid but do not allow the requested operation. |
| `UNAUTHORIZED` | Credentials are absent, invalid, expired, or not connected to an account. |
| `INTERNAL` | Provider-side or required-service failure, including unavailable Lightning infrastructure. |
| `UNSUPPORTED_ENCRYPTION` | NWC-specific encryption mismatch. Other adapters should rarely emit this. |
| `OTHER` | The provider error is known but does not fit a stable category yet. |

Local input validation should remain `LniError('InvalidInput', ...)` unless the provider accepted the request and returned a categorized business error.

## Local Fee Errors

Rust exposes `ApiError::FeeError(String)` and TypeScript exposes `FeeError` (with
`LniError.code === 'FeeError'`) for a valid quoted or provider fee that exceeds a
caller-configured fee limit. This is a local LNI guardrail error, not a standard NWC
error code and not a provider payment failure.

Current Strike Lightning behavior:

| Condition | Error |
| --- | --- |
| Quoted fee exceeds the absolute limit | Rust `ApiError::FeeError` / TypeScript `FeeError`, with quoted and limit amounts displayed in sats plus the exact msat value needed to allow the payment |
| Quoted fee exceeds the percentage limit | Rust `ApiError::FeeError` / TypeScript `FeeError`, with quoted fee and payment amount displayed in sats, the percentage limit, and guidance to increase the percentage or use a sufficient absolute limit |
| Quoted fee equals either limit | Payment is allowed |
| Both fee-limit forms are supplied | `InvalidInput` |
| A fee limit is negative, non-finite, or unsafe | `InvalidInput` |
| A limit is supplied but the quote fee cannot be determined safely | `InvalidInput` |

Strike creates the quote before enforcing the limit because its API does not accept a
maximum fee parameter. LNI checks the returned quote before calling `/execute`; a
`FeeError` therefore means the quote was not executed. When Strike supplies both its
total fee and Lightning network fee fields, LNI enforces the total fee; it uses the
network fee only when the total fee is absent.

## Fallback Rules

Apply exact provider-code mappings first. If no exact mapping exists, use these fallbacks:

| Native signal | Normalized code | Notes |
| --- | --- | --- |
| HTTP 401 | `UNAUTHORIZED` | Invalid token, expired token, missing account connection. |
| HTTP 403 | `RESTRICTED` | Authenticated but missing permission/scope. |
| HTTP 404 | `NOT_FOUND` | Use `OTHER` if strict NWC parity is preferred over LNI's extended set. |
| HTTP 409 | `OTHER` | Usually conflict/idempotency/in-flight state; exact provider code should refine this. |
| HTTP 422 | `OTHER` | Business-rule failure; exact provider code should refine this. |
| HTTP 425 | `OTHER` | Retryable account/provider state, not necessarily a rate limit. |
| HTTP 429 | `RATE_LIMITED` | Safe retry after provider backoff. |
| HTTP 500, 502, 503, 504 | `INTERNAL` | Provider or required-service failure. |
| Network failure / timeout | `INTERNAL` | Transport failed before the provider returned a structured business error. |
| Malformed provider response | `INTERNAL` | Provider returned an unusable response for an otherwise valid request. |

## Strike

Strike REST errors have a structured body with `data.code`, `data.status`, and `data.message`. The implementation should parse `LniError('Http')` bodies before falling back to HTTP status.

| Strike code/status | Operation(s) | Normalized code | Confidence | Notes |
| --- | --- | --- | --- | --- |
| `BALANCE_TOO_LOW` / 422 | `pay_invoice`, `prepare_onchain_transaction`, `pay_onchain` | `INSUFFICIENT_BALANCE` | Documented | Direct match for insufficient funds. |
| `RATE_LIMIT_EXCEEDED` / 429 | any | `RATE_LIMITED` | Documented | Provider rate limit. |
| `TOO_MANY_ATTEMPTS` / 429 | any | `RATE_LIMITED` | Documented | Short-term attempt limit. |
| `FORBIDDEN` / 403 | any | `RESTRICTED` | Documented | Valid identity without sufficient permission. |
| `UNAUTHORIZED` / 401 | any | `UNAUTHORIZED` | Documented | Invalid or unspecified identity. |
| `AMOUNT_TOO_HIGH` / 422 | payment creation/execution | `QUOTA_EXCEEDED` | Candidate | Provider/account limit, not balance. Preserve native limit details. |
| `TOO_MANY_TRANSACTIONS` / 422 | payment creation/execution | `QUOTA_EXCEEDED` | Candidate | Transaction-count limit exceeded. |
| `DEPOSIT_LIMIT_EXCEEDED` / 422 | deposits only | `QUOTA_EXCEEDED` | Candidate | Not used by current LNI Strike methods, but same class of limit. |
| `INVALID_LN_INVOICE` / 422 | `pay_invoice` | `PAYMENT_FAILED` | Documented | Invalid invoice for payment. |
| `INVALID_STATE_FOR_INVOICE_EXPIRED` / 422 | `pay_invoice` | `PAYMENT_FAILED` | Documented | Expired Lightning invoice. |
| `LN_ROUTE_NOT_FOUND` / 422 | `pay_invoice` | `PAYMENT_FAILED` | Documented | Terminal route failure. |
| `PAYMENT_QUOTE_EXPIRED` / 422 | `pay_invoice`, `pay_onchain` | `PAYMENT_FAILED` | Documented | Quote expired before execution. |
| `PROCESSING_PAYMENT` / 422 | `pay_invoice`, `pay_onchain` | `OTHER` | Documented | In-flight state. Caller may retry lookup/poll rather than treat as final failure. |
| `PROCESSING_CONFLICT` / 409 | any write | `OTHER` | Documented | Conflict with current provider state. Exact retry behavior should remain provider-specific. |
| `DUPLICATE_PAYMENT_QUOTE` / 422 | `prepare_onchain_transaction` | special case, otherwise `OTHER` | Implemented special case | Current code recovers by extracting `paymentQuoteId`; if not recoverable, normalize to `OTHER`. |
| `PAYMENT_PROCESSED` / 422 | payment execution | `OTHER` | Documented | Idempotency/already-processed state; do not collapse into success without fetching state. |
| `LN_INVOICE_PROCESSED` / 422 | `pay_invoice` | `OTHER` | Documented | Already processed invoice; may be duplicate payment protection. |
| `SELF_PAYMENT_NOT_ALLOWED` / 422 | `pay_invoice` | `RESTRICTED` | Candidate | Provider policy denies this operation. |
| `INVALID_RECIPIENT` / 422 | send operations | `PAYMENT_FAILED` | Documented | Recipient cannot currently receive. |
| `INVALID_AMOUNT` / 422 | payment creation | `OTHER` | Documented | If caused by local params, prefer local `InvalidInput` before calling Strike. |
| `AMOUNT_TOO_LOW` / 422 | payment creation | `OTHER` | Documented | Provider minimum amount. Keep native limit details. |
| `UNSUPPORTED_PAYMENT_METHOD` / 422 | payment creation | `OTHER` | Documented | Provider/payment-method mismatch. |
| `INVALID_PAYMENT_METHOD` / 422 | payment creation | `OTHER` | Documented | Provider/payment-method mismatch. |
| `CURRENCY_UNSUPPORTED` / 422 | any | `OTHER` | Documented | Config or provider support issue. |
| `USER_CURRENCY_UNAVAILABLE` / 422 | any | `OTHER` | Documented | Account does not support requested currency. |
| `EXCHANGE_RATE_NOT_AVAILABLE` / 422 | quote creation | `INTERNAL` | Candidate | Required provider pricing service unavailable. |
| `LN_UNAVAILABLE` / 503 | Lightning operations | `INTERNAL` | Documented | Strike's Lightning service unavailable. |
| `SERVICE_UNAVAILABLE`, `MAINTENANCE_MODE`, `BAD_GATEWAY`, `GATEWAY_TIMEOUT` / 5xx | any | `INTERNAL` | Documented | Provider/server availability class. |
| `INTERNAL_SERVER_ERROR` / 500 | any | `INTERNAL` | Documented | Provider internal error. |
| `NOT_FOUND` / 404 | lookup/list/fetch after write | `NOT_FOUND` | Documented | Current `listTransactions` intentionally treats some 404 responses as an empty list. |
| `INVALID_DATA`, `INVALID_DATA_QUERY`, validation error codes / 400 | any | `OTHER` | Documented | Prefer local validation when possible; provider validation errors are not necessarily auth/payment failures. |

## Blink

Blink uses GraphQL. Its docs note that HTTP 200 can still carry operation errors in the response body, and the public examples request `errors { code message path }` for send operations. The public reference describes operation-level `errors: [Error]` and payment statuses such as `SUCCESS`, `FAILED`, `PENDING`, and `ALREADY_PAID`, but does not publish a stable Blink error-code table comparable to Strike.

Current implementation gap: `GraphQLError` only stores `message`, and several Blink queries request only `message`, so a first implementation pass should request and preserve `code` and `path` everywhere Blink exposes operation errors.

| Blink signal | Operation(s) | Normalized code | Confidence | Notes |
| --- | --- | --- | --- | --- |
| Top-level GraphQL error message `Not authorized` | any authenticated operation | `UNAUTHORIZED` | Documented example | Blink's error-handling docs show this top-level GraphQL error for unauthorized `me`. |
| HTTP 401 | any | `UNAUTHORIZED` | Fallback | Transport/auth layer rejection. |
| HTTP 403 | any | `RESTRICTED` | Fallback | Authenticated but insufficient scope if Blink returns a REST-like status. |
| API key lacks `Write` scope | `pay_invoice`, `pay_onchain` | `RESTRICTED` | Documented scope model | Blink docs define `Write` as required to send payments. Prefer exact GraphQL `code` once captured. |
| API key lacks `Receive` scope | `make_invoice` | `RESTRICTED` | Documented scope model | Blink docs define `Receive` as required to create invoices. Prefer exact GraphQL `code` once captured. |
| API key lacks `Read` scope | `get_info`, `lookup_invoice`, `list_transactions` | `RESTRICTED` | Documented scope model | Blink docs define `Read` as required for balance/history. Prefer exact GraphQL `code` once captured. |
| Payment/fee-probe error code or message indicates insufficient balance | `pay_invoice`, `pay_onchain` | `INSUFFICIENT_BALANCE` | Candidate | Blink docs state payment wallets must have sufficient balance, but do not publish the specific error code. Verify with a fixture. |
| `lnInvoicePaymentSend.status === 'FAILED'` | `pay_invoice` | `PAYMENT_FAILED` | Documented status | Current code throws generic `Api`; normalize as terminal payment failure. |
| `lnInvoicePaymentSend.status === 'PENDING'` | `pay_invoice` | `OTHER` | Documented status | In-flight state; future implementation may poll rather than throw. |
| `lnInvoicePaymentSend.status === 'ALREADY_PAID'` | `pay_invoice` | `OTHER` | Documented status | Duplicate/already-paid protection. Do not collapse into success without transaction proof. |
| Fee-probe error code or message indicates invalid invoice | `pay_invoice` | `PAYMENT_FAILED` | Candidate | Verify exact Blink `error.code`; likely detected before payment send. |
| Fee-probe error code or message indicates no route / routing failure | `pay_invoice` | `PAYMENT_FAILED` | Candidate | Verify exact Blink `error.code`; route failure is payment-specific. |
| Invoice-create operation errors | `make_invoice` | `OTHER` | Candidate | Keep provider `code`/`path`; refine once real fixtures identify stable codes for invalid amount, wallet, or account state. |
| No BTC wallet found in account | `get_info`, `make_invoice`, `pay_invoice` | `OTHER` | Implemented local condition | This is an account/config shape mismatch rather than an auth failure. |
| Network failure / timeout | any | `INTERNAL` | Fallback | Transport failed before structured GraphQL data was available. |
| Malformed GraphQL response or missing `data` | any | `INTERNAL` | Fallback | Current code throws `Json`; normalized adapter errors should categorize this as provider/response failure. |

Blink implementation checklist:

1. Extend `GraphQLError` with optional `code?: string` and `path?: string[]`.
2. Request `code` and `path` in every operation-level `errors` selection where the schema supports it.
3. Preserve top-level GraphQL `errors` separately from operation payload errors.
4. Add fixtures from real sanitized Blink failures before locking any candidate message-based mappings.
5. Prefer exact `error.code` mappings over message substring matching once fixtures exist.

## CLN

CLN is accessed through `clnrest`. The REST docs require a `rune` header for POST authorization, while payment business errors come from CLN JSON-RPC errors. The `pay` reference publishes numeric error codes for common payment failures, and the `invoice` reference publishes creation errors.

Current implementation gap: `ClnNode` uses `requestJson` directly, so CLN REST or JSON-RPC error bodies are currently flattened into `LniError('Http')`. A normalization pass should parse JSON-RPC-shaped bodies for `error.code`, `error.message`, and `error.data`.

| CLN signal | Operation(s) | Normalized code | Confidence | Notes |
| --- | --- | --- | --- | --- |
| Missing/invalid `rune` header | any REST call | `UNAUTHORIZED` | Documented auth model | If the node cannot authenticate the caller. |
| Valid rune lacks method permission | any REST call | `RESTRICTED` | Candidate | Verify actual `clnrest` status/body with a restricted rune fixture. |
| `pay` error `201` | `pay_invoice`, `pay_offer` | `OTHER` | Documented | Already paid with the same hash using a different amount or destination. Preserve native code. |
| `pay` error `203` | `pay_invoice`, `pay_offer` | `PAYMENT_FAILED` | Documented | Permanent failure at destination. Keep `error.data` routing details. |
| `pay` error `205` | `pay_invoice`, `pay_offer` | `PAYMENT_FAILED` | Documented | Unable to find a route. |
| `pay` error `206` | `pay_invoice`, `pay_offer` | `PAYMENT_FAILED` | Documented | Route too expensive due to fee or locktime limits. Preserve fee/delay data for debugging. |
| `pay` error `207` | `pay_invoice`, `pay_offer` | `PAYMENT_FAILED` | Documented | Invoice expired. |
| `pay` error `210` | `pay_invoice`, `pay_offer` | `PAYMENT_FAILED` | Documented | Payment timed out without an in-progress payment. |
| `pay` error `-1` | `pay_invoice`, `pay_offer` | `OTHER` | Documented | Catchall. Use message/body heuristics only as a last resort. |
| `invoice` error `900` | `make_invoice` | `OTHER` | Documented | Duplicate label. Current LNI labels are generated, so this should be rare. |
| `invoice` error `901` | `make_invoice` | `OTHER` | Documented | Duplicate preimage. |
| `invoice` error `902` | `make_invoice` | `OTHER` | Documented | No specified private channels were usable. |
| JSON-RPC `-32602` | any | `OTHER` | Documented broadly in CLN refs | Invalid parameters. Prefer local `InvalidInput` before making the call where possible. |
| `listinvoices` returns no match | `lookup_invoice` | `NOT_FOUND` | Implemented local condition | Current code throws generic `Api`; normalize to missing resource. |
| Missing BOLT12 invoice from `fetchinvoice` | `make_invoice`, `pay_offer` | `INTERNAL` | Implemented local condition | Provider returned an unusable response. |
| Network failure / timeout | any | `INTERNAL` | Fallback | Transport failed before CLN returned structured JSON-RPC data. |

CLN implementation checklist:

1. Add a CLN JSON-RPC error parser that reads `error.code`, `error.message`, and `error.data` from HTTP bodies.
2. Normalize payment errors by numeric code before falling back to status/message rules.
3. Preserve CLN routing failure data in provider metadata, but do not expose full invoices or preimages in logs.
4. Add restricted-rune fixtures to distinguish `UNAUTHORIZED` from `RESTRICTED`.

## LND

LND is accessed through its REST gateway. For `payInvoice`, LNI uses `/v2/router/send`, which streams payment updates; the final update includes `status` and, on failure, `failure_reason`. The LND reference documents `PaymentStatus` values and `PaymentFailureReason` values.

Current implementation gap: `LndNode.payInvoice` parses the final stream line but flattens `wrapped.error`, `FAILED`, `IN_FLIGHT`, and unknown statuses into `LniError('Api')`. A normalization pass should keep `status`, `failure_reason`, and any stream error body as provider metadata.

| LND signal | Operation(s) | Normalized code | Confidence | Notes |
| --- | --- | --- | --- | --- |
| HTTP 401 | any REST call | `UNAUTHORIZED` | Fallback | Missing/invalid macaroon or transport auth rejection. |
| HTTP 403 or body contains `permission denied` | any REST call | `RESTRICTED` | Implemented detection | Current `getInfo` already detects permission denied for optional balance reads. |
| gRPC `UNAUTHENTICATED` via REST body | any REST call | `UNAUTHORIZED` | gRPC standard | Parse REST-gateway error body when present. |
| gRPC `PERMISSION_DENIED` via REST body | any REST call | `RESTRICTED` | gRPC standard | Parse REST-gateway error body when present. |
| `failure_reason === 'FAILURE_REASON_INSUFFICIENT_BALANCE'` | `pay_invoice` | `INSUFFICIENT_BALANCE` | Documented | LND describes this as insufficient local balance. |
| `failure_reason === 'FAILURE_REASON_NO_ROUTE'` | `pay_invoice` | `PAYMENT_FAILED` | Documented | All routes tried and failed, or no route exists. |
| `failure_reason === 'FAILURE_REASON_TIMEOUT'` | `pay_invoice` | `PAYMENT_FAILED` | Documented | Payment timeout exceeded. |
| `failure_reason === 'FAILURE_REASON_INCORRECT_PAYMENT_DETAILS'` | `pay_invoice` | `PAYMENT_FAILED` | Documented | Unknown hash, invalid amount, or invalid final CLTV. |
| `failure_reason === 'FAILURE_REASON_ERROR'` | `pay_invoice` | `PAYMENT_FAILED` | Documented | Non-recoverable payment error. |
| `failure_reason === 'FAILURE_REASON_CANCELED'` | `pay_invoice` | `OTHER` | Documented | Caller/system cancellation. If LNI adds a cancellation-specific NWC code later, map there. |
| `status === 'FAILED'` with no failure reason | `pay_invoice` | `PAYMENT_FAILED` | Documented status | Preserve raw final stream line. |
| `status === 'IN_FLIGHT'` | `pay_invoice` | `OTHER` | Documented status | Current code asks caller to increase timeout. It is not a terminal wallet/category failure. |
| `status === 'UNKNOWN'` or unexpected status | `pay_invoice` | `OTHER` | Documented/deprecated status | Preserve status for debugging. |
| Missing final stream line or missing result payload | `pay_invoice` | `INTERNAL` | Implemented local condition | REST stream produced an unusable response. |
| `lookupInvoice` HTTP 404 | `lookup_invoice` | `NOT_FOUND` | Fallback | Missing invoice/resource. |
| Local Bolt12 stubs | offer operations | `NOT_IMPLEMENTED` | Implemented local condition | Current code throws generic `Api`; normalize unsupported methods explicitly. |
| Network failure / timeout | any | `INTERNAL` | Fallback | Transport failed before LND returned structured data. |

LND implementation checklist:

1. Parse REST-gateway error JSON for gRPC `code`, `message`, and `details`.
2. Normalize `/v2/router/send` final `Payment.status` and `failure_reason`.
3. Keep `isPermissionDenied` behavior for optional balance reads, but make thrown permission errors structured.
4. Add fixtures for failed route, insufficient balance, permission-denied macaroon, and malformed stream responses.

## Phoenixd

phoenixd uses Basic auth and two passwords. Its API docs state that the primary `http-password` gives access to all endpoints, while the limited-access password cannot call spend-sensitive endpoints such as `payinvoice`, `payoffer`, and `sendtoaddress`. The source converts missing payment lookups to HTTP 404 and returns payment failures as response payloads from `payinvoice`/`payoffer`, not necessarily as non-2xx HTTP errors.

Current implementation gap: `PhoenixdNode` expects successful `payinvoice` responses to match `PhoenixdPayResponse`. It does not yet model the `PaymentFailed` payload shape or text response failures from on-chain/channel endpoints.

| Phoenixd signal | Operation(s) | Normalized code | Confidence | Notes |
| --- | --- | --- | --- | --- |
| HTTP 401 with invalid Basic auth text | any | `UNAUTHORIZED` | Source-confirmed | Invalid password or missing Basic auth. |
| Limited-access password used on full-access endpoint | `pay_invoice`, `pay_offer`, spend/admin operations | `RESTRICTED` | Documented/source-confirmed | Full-access routes use the `full-access` auth configuration. |
| HTTP 404 `Not found` from payment lookup | `lookup_invoice` | `NOT_FOUND` | Source-confirmed | Source converts empty payment lookup to 404. |
| HTTP 405 invalid method text | any | `OTHER` | Source-confirmed | Wrong method for endpoint. Should generally be adapter bug. |
| HTTP 400 missing/invalid request parameter | any | `OTHER` | Source-confirmed | Prefer local `InvalidInput` before calling phoenixd. |
| `PaymentFailed` body from `payinvoice` / `payoffer` with reason indicating insufficient funds/balance | `pay_invoice`, `pay_offer` | `INSUFFICIENT_BALANCE` | Candidate | Need real sanitized fixture for exact `PaymentFailed` JSON fields/reason names. |
| `PaymentFailed` body from `payinvoice` / `payoffer` with reason indicating no route, timeout, expired invoice, invalid invoice, or recipient failure | `pay_invoice`, `pay_offer` | `PAYMENT_FAILED` | Candidate | phoenixd returns a failure payload when the peer reports payment not sent. Exact field parsing needs fixtures. |
| `PaymentFailed` body with unknown reason | `pay_invoice`, `pay_offer` | `OTHER` | Candidate | Preserve provider reason and body. |
| Text response `no channel available` | spend/channel operations | `INSUFFICIENT_BALANCE` | Candidate | Indicates there is no channel capacity to spend or manage. Verify endpoint-specific behavior before locking. |
| Text response from `sendtoaddress` / `bumpfee` / `closechannel` containing `Failure` | on-chain/channel operations | `OTHER` | Source-confirmed shape | Current TypeScript node does not expose these operations, but future on-chain support should parse these text failures. |
| Missing or malformed JSON response from JSON endpoints | any | `INTERNAL` | Fallback | Provider returned an unusable response for the expected endpoint. |
| Network failure / timeout | any | `INTERNAL` | Fallback | Transport failed before phoenixd returned structured data. |

Phoenixd implementation checklist:

1. Add response-shape guards for `PaymentSent` vs `PaymentFailed` from spend endpoints.
2. Capture real sanitized `PaymentFailed` payloads for insufficient funds, no route, expired invoice, and invalid invoice.
3. Treat limited-access password failures on spend endpoints as `RESTRICTED`, not `UNAUTHORIZED`, when the body/status makes that distinguishable.
4. Normalize lookup 404s to `NOT_FOUND` and keep local missing-search validation as `InvalidInput`.

## Speed

Speed uses REST endpoints for balances, payments, and sends. The current adapter authenticates with Basic auth, creates invoices through `/payments`, sends through `/send`, and looks up outgoing sends through `/send/filter`.

Current implementation gap: public Speed error-code documentation was not fetchable from this environment, and `SpeedNode` currently flattens non-2xx bodies through `requestJson` into `LniError('Http')`. Treat this section as implementation-driven until real sanitized Speed error fixtures are captured.

| Speed signal | Operation(s) | Normalized code | Confidence | Notes |
| --- | --- | --- | --- | --- |
| HTTP 401 | any | `UNAUTHORIZED` | Fallback | Missing/invalid API key. |
| HTTP 403 | any | `RESTRICTED` | Fallback | Valid key without permission for endpoint. |
| HTTP 404 | lookup/filter by id or request | `NOT_FOUND` | Fallback | Missing provider resource. Current lookup also has a local no-match path. |
| HTTP 429 | any | `RATE_LIMITED` | Fallback | Provider rate limit. |
| HTTP 5xx | any | `INTERNAL` | Fallback | Provider/server failure. |
| Error body/code/message indicates insufficient funds, insufficient balance, or available balance too low | `pay_invoice` | `INSUFFICIENT_BALANCE` | Candidate | Capture real Speed fixture before locking exact parser. |
| Error body/code/message indicates send amount exceeds account/transaction limit | `pay_invoice` | `QUOTA_EXCEEDED` | Candidate | Preserve native code and limit details. |
| Error body/code/message indicates invalid Lightning invoice or unsupported withdraw request | `pay_invoice` | `PAYMENT_FAILED` | Candidate | Payment-specific failure. |
| Error body/code/message indicates route failure, expired invoice, or recipient failure | `pay_invoice` | `PAYMENT_FAILED` | Candidate | Payment-specific failure. |
| `/send` returns `status === 'failed'` | `pay_invoice`, `lookup_invoice`, `list_transactions` | `PAYMENT_FAILED` for immediate send result; transaction state otherwise | Implemented response field | Current `payInvoice` does not check status. For history/listing, keep it as transaction state rather than throwing. |
| `/send` returns `status === 'unpaid'` | `pay_invoice` | `OTHER` | Implemented response field | In-flight or pending send state. Do not treat as terminal success. |
| `lookupInvoice` local no-match path | `lookup_invoice` | `NOT_FOUND` | Implemented local condition | Current code throws generic `Api`; normalize to missing resource. |
| Missing `amountMsats` in `payInvoice` | `pay_invoice` | local `InvalidInput` | Implemented local condition | This is caller input, not provider normalization. |
| Local Bolt12 stubs | offer operations | `NOT_IMPLEMENTED` | Implemented local condition | Current code throws generic `Api`; normalize unsupported methods explicitly. |
| Malformed provider response | any | `INTERNAL` | Fallback | Provider returned an unusable response. |
| Network failure / timeout | any | `INTERNAL` | Fallback | Transport failed before Speed returned structured data. |

Speed implementation checklist:

1. Capture sanitized non-2xx Speed bodies for auth failure, permission failure, insufficient balance, invalid invoice, and route failure.
2. Add a Speed error parser that preserves native status, code, message, and any request id.
3. Make `payInvoice` inspect `/send` status and avoid returning success for `failed` or pending statuses.
4. Normalize lookup no-match errors to `NOT_FOUND` and unsupported methods to `NOT_IMPLEMENTED`.
