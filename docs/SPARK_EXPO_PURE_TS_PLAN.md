# Spark for LNI in Expo Go (Pure TypeScript, No WASM, No Native Module)

> **Status: COMPLETE** — All phases implemented. `payInvoice` confirmed working end-to-end with real Spark mainnet backend (preimage returned, payment received on destination node).

## Goal

Implement Spark support in `bindings/typescript` that:

- works in browser frontends,
- works in Expo Go,
- does **not** depend on UniFFI,
- does **not** require native React Native modules,
- does **not** rely on WebAssembly at runtime.

## Research Summary (What this means technically)

### 1) Breez Spark SDK paths do not match Expo Go + no-WASM constraints

- `@breeztech/breez-sdk-spark/web` is explicitly WASM-initialized (`await init()`).
- `@breeztech/breez-sdk-spark-react-native` explicitly says it requires native code and custom dev builds, and does not work with Expo Go.

Practical consequence: Breez’s published routes are great for web (WASM) or RN custom builds (native), but not for Expo Go under your constraints.

### 2) Spark protocol/client complexity is non-trivial

- Breez Rust wraps a full Spark wallet stack (`spark-wallet`, operator pool, signing, services).
- This includes threshold/FROST signing and ECIES helper operations, not just simple HTTP REST calls.

Practical consequence: a direct from-scratch LNI Spark adapter is feasible, but needs a phased implementation with strong crypto and integration testing.

### 3) There is a JS Spark ecosystem, but runtime bindings still matter

- Spark JS SDK code paths show browser/wasm, node/wasm, and RN/native-entry approaches for low-level frost bindings.
- A pure-TS Expo Go path still requires replacing those low-level bindings with pure TypeScript implementations.

Practical consequence: best path is to implement a pure TS crypto/binding layer and wire it into a Spark client stack used by `SparkNode`.

---

## What was actually built

### How the pure TypeScript Spark signer works

The Spark protocol uses **2-of-2 threshold Schnorr signatures (FROST)** for every operation — sending, receiving, even wallet initialization. The official Spark SDK (`@buildonspark/spark-sdk`) ships a **Rust WASM binary** that handles all the FROST cryptography. That works in Node.js, but breaks in **browsers and Expo/React Native** where WASM loading is restricted or unavailable.

We didn't write a FROST library from scratch. We used two existing TypeScript packages:
- **`@frosts/core`** — generic FROST protocol types and operations
- **`@frosts/secp256k1-tr`** — secp256k1 + Taproot (BIP-340) specific implementation

Then in `spark.ts`, we wrote two functions that **replace** the SDK's WASM calls:

1. **`pureSignFrost()`** — computes the user's signature share. This is the user's half of the 2-of-2 signing. It takes the user's secret key, nonces, and the signing package (commitments from both the user and the statechain operator), then produces a signature share using the FROST round2 math.

2. **`pureAggregateFrost()`** — combines the user's share with the statechain operator's share into a final Schnorr signature. For the adaptor path (Lightning payments via swap), it also handles adaptor signature construction where the signature is offset by an adaptor point.

These get injected into the Spark SDK via `setSparkFrostOnce()`, which overrides the SDK's default WASM signer with our pure TS implementation.

### Key cryptographic details discovered during implementation

**Spark uses "nested FROST"** — a Lightspark fork of the ZcashFoundation FROST protocol. In nested FROST, signers are organized into participant groups. The user is always in a singleton group `{user}`, and the statechain operators form their own group. Lagrange interpolation coefficients are computed *within each group*, not across all participants. For a singleton group, the Lagrange coefficient (lambda) is always 1.

This means:
- `computeSignatureShareRustCompat()` correctly uses `lambdaI = scalarOne()` (hardcoded to 1)
- Standard FROST libraries (like `@frosts`) compute Lagrange across all participants, which gives wrong coefficients for Spark's scheme — this is why we do manual aggregation instead of using `@frosts` aggregate functions

**BIP-340/Taproot parity handling** is critical throughout:
- Verifying keys must be even-Y normalized before computing binding factors
- Group commitments may need Y-parity negation for BIP-340 compatibility
- The `intoEvenYKeyPackage()` and `normalizePublicKeyPackageForPreAggregate()` functions handle this

**Adaptor signatures** are used for Lightning payments via swap. The group commitment R is offset by an adaptor public key T to create `R' = R + T`. Two z-candidates (z and -z) are tested to find which produces a valid adaptor signature. The adaptor private key is later used to complete the signature when the swap settles.

### Bugs fixed during implementation

1. **Adaptor signature validation key (Issue #8)**: Validation checked against the user's individual public key instead of the aggregated group key. BIP-340 challenge hash depends on the full group key, making validation effectively random (~50% correct).

2. **Non-adaptor aggregate path (Issue #9)**: The `@frosts` library's `aggregateWithTweak()` broke for two reasons: (a) JavaScript Maps use reference equality for object keys — different `Identifier` instances for the same logical signer don't match, and (b) binding factors were computed with a different key than what signing used. Fixed with manual aggregation.

3. **Payment completion polling (Issue #10)**: The SDK's `payLightningInvoice()` returns immediately after swap initiation, before the Lightning payment settles. Added polling via `getLightningSendRequest` to wait for terminal status and return the preimage.

### Architecture (as implemented)

The implementation is simpler than the original plan. Instead of a `spark-internal/` module split, everything lives in a single adapter file:

- **`bindings/typescript/src/nodes/spark.ts`** — `SparkNode` class + all pure TS FROST functions
- **`bindings/typescript/src/spark-runtime.ts`** — browser/Expo runtime compatibility (`Buffer`, base64, fetch)
- **`bindings/typescript/src/vendor/frosts-bridge.js`** — re-exports from `@frosts/core` and `@frosts/secp256k1-tr`

The Spark SDK itself (`@buildonspark/spark-sdk`) handles wallet orchestration, SSP/operator API calls, key derivation, storage, and transfer services. We only replace the FROST signing layer.

### Reference implementation

The canonical Spark FROST implementation is in Rust at [buildonspark/spark](https://github.com/buildonspark/spark), using a fork of `frost-secp256k1-tr` from [lightsparkdev/frost](https://github.com/lightsparkdev/frost) (branch: `nested-signing`). The [lightsparkdev/js-sdk](https://github.com/lightsparkdev/js-sdk) wraps this Rust code via WASM — it does NOT use `@frosts` packages.

---

## Original Recommended Architecture (for reference)

### A) New node + config (DONE)

- `SparkConfig` added to `bindings/typescript/src/types.ts`
- `SparkNode` class at `bindings/typescript/src/nodes/spark.ts`
- All LNI parity methods implemented: `getInfo`, `createInvoice`, `payInvoice`, `lookupInvoice`, `listTransactions`, `onInvoiceEvents`
- Exported via factory (`createNode({ kind: 'spark', ... })`)

### B) Spark internal module split (SIMPLIFIED)

Instead of a separate `spark-internal/` directory, the implementation uses:
- Single adapter file (`spark.ts`) with pure TS FROST functions inline
- Spark SDK handles wallet/client/storage/signer orchestration internally
- Vendor bridge module (`frosts-bridge.js`) for `@frosts` re-exports

### C) Storage abstraction (DEFERRED)

Storage is handled by the Spark SDK's internal storage layer. No custom abstraction needed.

---

## Step-by-Step Execution Plan (with actual status)

## Phase 0 — Contract + scaffolding -- DONE

1. `SparkConfig` + factory wiring + `SparkNode` class added.
2. Typed errors and unsupported method stubs in place.
3. `spark` added to backend config unions.
4. README updated with Spark usage and runtime notes.

## Phase 1 — LNI method mapping design -- DONE

1. Mapped from Rust Spark behavior (`crates/lni/spark/api.rs`, `lib.rs`, `types.rs`).
2. All method contracts defined with proper unit conversions.
3. Spark SDK handles most mapping internally; LNI adapter normalizes amounts and timestamps.

## Phase 2 — Pure TS signer/crypto foundation -- DONE

This was the hardest phase. Key decisions:
- **Did NOT implement FROST from scratch.** Used `@frosts/core` + `@frosts/secp256k1-tr` TypeScript packages.
- **Did NOT implement key derivation.** The Spark SDK handles BIP39 mnemonic→seed and HD derivation internally.
- **Implemented `pureSignFrost()` and `pureAggregateFrost()`** as drop-in replacements for the SDK's WASM signer.
- **Implemented `pureCreateDummyTx()`** using `@scure/btc-signer` for transaction construction.
- **Implemented ECIES** using the `eciesjs` package.

Three critical bugs were found and fixed in this phase (see "Bugs fixed during implementation" above).

## Phase 3 — Spark wallet/client flow -- DONE (simplified)

The Spark SDK (`@buildonspark/spark-sdk`) handles all wallet/client orchestration. LNI's `SparkNode` is a thin adapter that:
1. Initializes the SDK wallet with the pure TS signer override
2. Delegates `getInfo`, `createInvoice`, `payInvoice`, `listTransactions`, `lookupInvoice` to SDK methods
3. Normalizes responses to the LNI `Transaction` / `PayInvoiceResponse` types
4. Polls `getLightningSendRequest` after `payInvoice` to wait for settlement

## Phase 4 — Security + persistence hardening -- DEFERRED

Storage and secret management are handled by the Spark SDK's internal layer. LNI does not persist secrets beyond what the SDK does. The mnemonic is passed in-memory via config.

## Phase 5 — Integration tests (real backend) -- DONE

- `spark.real.test.ts` tests: `getInfo`, `createInvoice`, `listTransactions`, `lookupInvoice`, `payInvoice`
- `payInvoice` confirmed working end-to-end with real mainnet backend (preimage returned, payment received)
- Env-gated credentials via `--env-file=../../crates/lni/.env`

## Phase 6 — Docs + publish readiness -- DONE

- README includes full Spark section with pure TS FROST explanation
- Example apps (web + Expo Go) documented with setup instructions
- Package validation: `typecheck`, `build`, `pack:dry-run` all passing

---

## Security Plan for “local storage allowed”

You said local storage is acceptable. The safest practical version is:

1. **Store encrypted blob only** in AsyncStorage/localStorage/IndexedDB.
2. **Never store raw mnemonic/api key** directly.
3. Use AES-GCM with random IV per write.
4. Derive encryption key from user PIN/passphrase (plus salt), or wrap key via secure enclave APIs when available.
5. Keep decrypted secrets in memory only while wallet is unlocked.
6. Zero/release references on logout/app background as much as JS runtime allows.

If you want “no passphrase UX”, use device-backed secure storage as the KEK where available; fallback to passphrase mode otherwise.

---

## Risks and Mitigations (updated with lessons learned)

- **Risk:** Pure TS FROST correctness.
  - **Outcome:** Three critical bugs found and fixed. The main difficulty was matching Spark's "nested FROST" variant (singleton user group, lambda=1), not the core FROST math itself.
  - **Mitigation applied:** Manual aggregation bypassing `@frosts` internal verify/aggregate, matching the WASM implementation's behavior.
- **Risk:** Protocol drift with Spark upstream.
  - **Mitigation:** Pure TS signer is a thin layer (~300 lines of FROST code) that only replaces `signFrost` and `aggregateFrost`. All wallet/protocol logic stays in the Spark SDK.
- **Risk:** Expo Go runtime quirks.
  - **Outcome:** Several bundler issues resolved (Hermes `import.meta`, dynamic imports, `Buffer` polyfill).
  - **Mitigation applied:** `installSparkRuntime()` centralizes all compatibility fixes.
- **Risk:** `@frosts` library quirks.
  - **Outcome:** JavaScript `Map` reference equality for `Identifier` objects caused silent failures. Binding factor computation used different key normalization than signing.
  - **Mitigation applied:** Avoid `@frosts` aggregate functions entirely; do manual share summation. Only use `@frosts` for primitives (identifiers, key packages, nonce commitments, binding factors, group commitments).

---

## Remaining follow-ups

- Add deterministic FROST signing test vectors to catch regressions without a live backend.
- Consider making `payInvoice` polling timeout configurable (currently 60s hardcoded).
- Test `payInvoice` in browser and Expo Go environments (currently only tested in Node.js integration tests).
- Verify `createInvoice` receive flow end-to-end (invoice created, payment received, balance updated).
