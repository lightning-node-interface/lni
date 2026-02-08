# Thread Context (Sanitized)

## Primary Goal
- Build and ship a frontend-capable TypeScript port of LNI (`bindings/typescript`) without UniFFI.
- Support browser and Expo Go usage.
- Add Spark support in pure TypeScript (no WASM dependency required by app code).

## Current State
- NPM package: `@sunnyln/lni`
- TypeScript binding location: `bindings/typescript`
- Spark is implemented in `bindings/typescript/src/nodes/spark.ts`.
- Public polymorphic factory is available via `createNode(...)` with typed backend config unions.

## Implemented Nodes
- `PhoenixdNode`
- `ClnNode`
- `LndNode`
- `NwcNode`
- `StrikeNode`
- `SpeedNode`
- `BlinkNode`
- `SparkNode`

## Major Architecture Decisions
- Keep a single cross-node interface (`LightningNode`) in `bindings/typescript/src/types.ts`.
- Use HTTP/fetch-first adapters for frontend compatibility.
- Implement Spark with a pure TypeScript FROST patch path, not UniFFI bindings.
- Package Spark browser/runtime helper artifacts directly in `@sunnyln/lni` so example apps do not need local vendor/shim boilerplate.
- Use `createNode({ kind: 'spark', ... })` as the canonical API in docs/examples (not direct `SparkWallet` usage).

## Spark Implementation Details
- Core adapter: `bindings/typescript/src/nodes/spark.ts`
- Runtime helper: `bindings/typescript/src/spark-runtime.ts`
- Packaged vendor outputs (generated at build time):
  - `bindings/typescript/dist/vendor/spark-sdk-bare.js`
  - `bindings/typescript/dist/vendor/frosts-bridge.js`
  - `bindings/typescript/dist/vendor/spark-runtime.browser.js`
- Source vendor bridge modules (for source-mode/test resolution):
  - `bindings/typescript/src/vendor/spark-sdk-bare.js`
  - `bindings/typescript/src/vendor/frosts-bridge.js`
- Vendor build pipeline: `bindings/typescript/scripts/build-spark-vendor.mjs`

## Spark Runtime Model
- `installSparkRuntime(...)` handles:
  - missing `Buffer` in browser/Expo environments
  - missing `atob`/`btoa`
  - fetch compatibility (stream reader fallback)
  - optional API key header injection
- Spark SDK entry selection (`sdkEntry`):
  - `auto` (default): packaged bare in browser/Expo, default entry in Node
  - `bare`: force packaged bare runtime
  - `default`: default Spark SDK entry
  - `native`: React Native native entry

## Example Apps
- Web example: `bindings/typescript/examples/spark-web`
- Expo Go example: `bindings/typescript/examples/spark-expo-go`
- Both examples use:
  - `installSparkRuntime(...)`
  - `createNode({ kind: 'spark', ... })`
  - `getInfo()` + `listTransactions()`

## Real Integration Tests
- Node-specific real tests in:
  - `bindings/typescript/src/__tests__/integration/*.real.test.ts`
- Spark real test:
  - `bindings/typescript/src/__tests__/integration/spark.real.test.ts`
- Tests load env values from local `.env` during execution only (no committed credentials).

## Important Issues Encountered and Resolved

### 1) Web ESM/default export breakage (`bare-https` default export error)
- Symptom: browser ESM runtime failed on transitive modules.
- Resolution: bundle and patch Spark bare runtime for browser-safe transport and dependency behavior in the vendor build script.

### 2) Browser transport errors (`node:https is not supported`)
- Symptom: authentication challenge requests failed in browser.
- Resolution: patched Spark transport path to fetch/gRPC-web compatible browser transport in generated vendor bundle.

### 3) Browser `Buffer is not defined`
- Symptom: runtime failures after transport/auth began working.
- Resolution: `installSparkRuntime(...)` now installs `Buffer`/base64 compatibility.

### 4) Expo could not resolve local package / vendor paths
- Symptom: module resolution failures for `@sunnyln/lni` and `dist/vendor/*`.
- Resolution: example app uses local package dependency and Metro resolver handling for linked package `./dist/*` paths.

### 5) Expo Hermes `import.meta` not supported
- Symptom: bundling failed on `import.meta.url` usage.
- Resolution: removed `import.meta` dependent path logic.

### 6) Expo Hermes invalid dynamic import call (`import(specifier)`)
- Symptom: `Invalid call ... import(specifier)` at runtime.
- Resolution: removed non-literal dynamic import fallback and switched to explicit static-branch import paths.

### 7) Spark integration test load failures
- Symptom: source-mode Spark tests failed to resolve bridge files and then failed under VM dynamic import constraints.
- Resolution: added source vendor bridge files and replaced VM-based import callbacks with explicit import branches.

### 8) Adaptor signature validation using wrong public key in `pureAggregateFrost`
- Symptom: `payInvoice` fails intermittently (~50%) on swap path with `initiate_swap_primary_transfer` R-point mismatch / adaptor signature validation errors.
- Root cause: `validateOutboundAdaptorSignatureLocal` was called with `params.selfPublicKey` (user's individual share key) instead of the aggregated tweaked verifying key. The BIP-340 challenge hash depends on the aggregated key, so using the share key caused both z-candidate validations to fail, with the fallback candidate being correct only ~50% of the time.
- Resolution: changed validation to use `preAggregatedPublicKeyPackage.verifyingKey` (the tweaked+even-Y-normalized aggregated verifying key) which matches the key used in the FROST challenge computation during signing.

### 9) Non-adaptor aggregate path using `aggregateWithTweak` fails with `Unknown identifier`
- Symptom: Wallet initialization fails during leaf claim/optimize with `FrostError: Unknown identifier`, blocking all operations including `payInvoice` (insufficient balance).
- Root causes:
  1. `@frosts/core` `aggregateCustom` internally calls `signingPackage.signingCommitments.get(identifier)` using raw `Map.get()` with Identifier objects. JavaScript Maps use reference equality (`===`) for object keys, so different Identifier instances with the same value don't match.
  2. `coreAggregate` computes binding factors using the non-even-Y-normalized tweaked verifying key, while `pureSignFrost` uses the even-Y-normalized key. This binding factor mismatch causes aggregate verification to fail, triggering cheater detection which hits the identifier issue.
- Resolution: Replaced `aggregateWithTweak` in non-adaptor path with manual aggregation matching the adaptor path pattern — tweak and even-Y-normalize the public key package, compute binding factors with the normalized key, sum shares directly, and serialize the signature without @frosts internal verification.
- Key insight from Lightspark reference implementation (`buildonspark/spark`): Spark uses "nested FROST" where the user is in a singleton participant group, making lambda_i=1 correct. The Rust aggregate also skips verification for adaptor signatures.

### 10) `payInvoice` returns before Lightning payment settles (no preimage)
- Symptom: `payInvoice` returns `paymentPreimage: null` and `status: "LIGHTNING_PAYMENT_INITIATED"`. Payment is in-flight but not complete; no preimage returned.
- Root cause: The Spark SDK's `payLightningInvoice()` initiates the swap and Lightning send, then returns immediately without waiting for settlement. The preimage becomes available only after the Lightning payment succeeds and the transfer completes (status progression: `LIGHTNING_PAYMENT_INITIATED` → `LIGHTNING_PAYMENT_SUCCEEDED` → `PREIMAGE_PROVIDED` → `TRANSFER_COMPLETED`).
- Resolution: After the initial `payLightningInvoice` call, poll `wallet.getLightningSendRequest(id)` at 2-second intervals (up to 60 seconds) until a terminal status is reached. On success (`TRANSFER_COMPLETED`, `PREIMAGE_PROVIDED`, `LIGHTNING_PAYMENT_SUCCEEDED`), extract the preimage. On failure statuses (`LIGHTNING_PAYMENT_FAILED`, `USER_SWAP_RETURNED`, etc.), throw an error.

## Recent Successes (Spark Frontend Work)
- **payInvoice works end-to-end** with real Spark backend — payment confirmed received on destination node, preimage returned via polling (fee 3000 msats).
- Spark node runs through public LNI factory API (`createNode({ kind: 'spark', ... })`) in both example apps.
- Web example can initialize wallet, load balance, and list transactions without requiring WASM.
- Expo Go example can initialize wallet and run refresh/list transaction flow without app-local shim boilerplate.
- Spark runtime helper centralizes browser/Expo compatibility setup (`Buffer`, base64 helpers, fetch compatibility).
- Spark signer phase debugging is available in Expo example (sanitized checkpoints + copy action).
- Spark package/build validation succeeded in maintainer workflow (`typecheck`, `build`, `pack:dry-run`, Spark integration run).

## Recent Failures / Open Issues
- Browser transport incompatibilities appeared during development (AbortSignal and transport adapter issues) and were mitigated, but remain a regression risk when vendor/runtime changes.
- Expo/Web bundling can regress if vendor outputs are stale or cache is not cleared after runtime import-path changes.
- Spark `payInvoice` now polls for completion, but the polling timeout (60s) may be too short for slow Lightning routes.

## Current Troubleshooting Sequence
1. Rebuild TypeScript package artifacts before testing (`npm run build` in `bindings/typescript`).
2. Confirm runtime install executes before Spark node creation in app startup.
3. For Expo, restart Metro with cache clear after dependency/runtime edits.
4. Verify read path first (`getInfo`, `listTransactions`) before testing `payInvoice`.
5. On send failure, capture sanitized signer/debug checkpoints and backend error text for comparison across web vs Expo.

## Outstanding Technical Follow-ups
- Add deterministic signer/adaptor vector checks to catch parity or R-point divergence before runtime.
- Add documented backend-side diagnostics for `request_swap` generic failures (to distinguish client math vs service-side policy/state issues).
- Consider making payInvoice polling timeout configurable (currently 60s hardcoded).

## README / Packaging Updates
- Main TS README includes Spark usage and frontend runtime guidance.
- Expo Go example README includes setup + troubleshooting.
- Package install/publish flow documented for `@sunnyln/lni`.

## Security Notes (Sanitized)
- No credentials, mnemonics, or private runtime values are stored in this context file.
- Do not commit API keys, macaroons, runes, passwords, NWC URIs, or local machine-only details.
- Integration logs may include invoice objects when explicitly enabled for manual verification; treat logs as sensitive.

## Validation Workflow (Maintainer)
- `npm run typecheck`
- `npm run build`
- `npm run pack:dry-run`
- For Spark integration (real env only): `npm run test:integration:spark`

## Source of Truth for Spark Semantics
- Rust Spark implementation remains canonical reference:
  - `crates/lni/spark/api.rs`
  - `crates/lni/spark/lib.rs`
  - `crates/lni/spark/types.rs`
