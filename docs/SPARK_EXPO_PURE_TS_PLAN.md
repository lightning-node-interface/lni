# Spark for LNI in Expo Go (Pure TypeScript, No WASM, No Native Module)

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

## Recommended Architecture in `bindings/typescript`

### A) New node + config

- Add `SparkConfig` to `bindings/typescript/src/types.ts`:
  - `network: 'mainnet' | 'regtest'`
  - `apiKey?: string`
  - `mnemonic?: string`
  - `passphrase?: string`
  - `storage?: SparkStorageConfig`
  - `sspBaseUrl?`, `electrsUrl?`, `operatorOverrides?` (advanced escape hatches)

- Add `SparkNode` class:
  - `bindings/typescript/src/nodes/spark.ts`
  - Implement LNI parity methods:
    - `getInfo`
    - `createInvoice`
    - `payInvoice`
    - `lookupInvoice`
    - `listTransactions`
    - `onInvoiceEvents`
  - Keep unsupported methods explicit (`createOffer`, `listOffers`, `payOffer`) if Spark behavior remains unsupported in LNI parity.

- Export via:
  - `bindings/typescript/src/factory.ts`
  - `bindings/typescript/src/index.ts`

### B) Spark internal module split

Create `bindings/typescript/src/nodes/spark-internal/`:

- `client.ts` — SSP/operator API client wrapper(s)
- `wallet.ts` — wallet/session orchestration
- `signer.ts` — key derivation + signing interface
- `spark-frost-pure.ts` — pure TS implementation of required frost/ecies functions
- `storage.ts` — secure persistence adapter
- `mapping.ts` — Spark payment objects → LNI `Transaction`
- `errors.ts` — Spark-specific error normalization

### C) Storage abstraction (cross-platform)

Define interface:

- `get(key): Promise<string | null>`
- `set(key, value): Promise<void>`
- `remove(key): Promise<void>`

Implement adapters:

- Browser: `localStorage` (or IndexedDB wrapper)
- Expo Go: AsyncStorage adapter

Wrap all persisted secrets in encryption-at-rest (details below).

---

## Step-by-Step Execution Plan

## Phase 0 — Contract + scaffolding (1 day)

1. Add `SparkConfig` + factory wiring + placeholder `SparkNode`.
2. Add typed errors and explicit “not implemented yet” methods.
3. Add `spark` to backend config unions.
4. Add README section describing constraints and current status.

**Exit criteria**
- `npm run typecheck` passes.
- Package builds with Spark stubs included.

## Phase 1 — LNI method mapping design (1 day)

1. Map Rust Spark behavior from:
   - `crates/lni/spark/api.rs`
   - `crates/lni/spark/lib.rs`
2. Define exact TypeScript method contracts:
   - timestamp units = seconds
   - `amountMsats` conversions
   - pagination semantics (`from`, `limit`, `search`, `paymentHash`)
3. Write mapping tests (pure unit tests) for Spark→LNI transforms.

**Exit criteria**
- Documented mapping table in code/docs.
- Unit tests cover conversion edge cases.

## Phase 2 — Pure TS signer/crypto foundation (hard part) (3–7 days)

1. Implement seed/mnemonic/key derivation in TS:
   - BIP39 mnemonic→seed
   - HD derivation paths needed by Spark
2. Implement pure TS ECIES helpers.
3. Implement pure TS FROST functions required by wallet flow:
   - signing share
   - aggregate signature
4. Implement dummy tx helper (if required by flow) in TS.
5. Port from Spark/Breez reference behavior and validate with cross-language vectors.

**Exit criteria**
- Deterministic test vectors pass against known-good fixtures.
- No wasm/native runtime dependency in the Spark path.

## Phase 3 — Spark wallet/client flow (3–5 days)

1. Build API clients for SSP/operators used by Spark wallet flow.
2. Implement wallet init/connect path with signer + config.
3. Implement:
   - `createInvoice` (Lightning receive)
   - `payInvoice` (Lightning send)
   - `listTransactions`
   - `lookupInvoice` (paginate until found)
4. Implement `getInfo` using Spark balance/info calls.
5. Implement polling-based `onInvoiceEvents`.

**Exit criteria**
- SparkNode methods function in local smoke tests.
- Error handling and retries follow existing node conventions.

## Phase 4 — Security + persistence hardening (1–2 days)

1. Add encrypted secret container for persisted data:
   - mnemonic
   - optional api key
2. Key management strategy:
   - Browser: derive key from user passphrase (PBKDF2/Argon2) + salt; store only ciphertext, iv, salt.
   - Expo Go: prefer `expo-secure-store` for wrapping key material; fallback to passphrase-derived key if secure store unavailable.
3. Enforce log redaction in Spark paths.
4. Add explicit unsafe mode flag if plain storage is enabled.

**Exit criteria**
- No plaintext secrets in logs or serialized fixtures.
- Secure mode is default; unsafe mode is opt-in.

## Phase 5 — Integration tests (real backend) (2–4 days)

1. Add `bindings/typescript/src/__tests__/integration/spark.real.test.ts`.
2. Use env-based runtime credentials (never commit values).
3. Test real flows:
   - connect/info
   - create+lookup invoice
   - pay invoice
   - list transactions pagination/search/paymentHash filters
4. Add package checks:
   - `npm run typecheck`
   - `npm run pack:dry-run`
   - ensure no test/secrets in package files.

**Exit criteria**
- Integration suite passes with valid env.
- Pack dry-run excludes tests and secrets.

## Phase 6 — Docs + publish readiness (1 day)

1. Update `bindings/typescript/README.md` with Spark setup and runtime caveats.
2. Add `.env.example` entries for Spark test keys (placeholders only).
3. Add migration note: “Spark pure-TS path for Expo Go”.

**Exit criteria**
- Docs are sufficient for another model/engineer to continue implementation directly.

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

## Risks and Mitigations

- **Risk:** Pure TS FROST correctness/perf.
  - **Mitigation:** Cross-language fixtures from Spark Rust tests before integration.
- **Risk:** Protocol drift with Spark upstream.
  - **Mitigation:** isolate protocol code in `spark-internal/*`, add compatibility tests.
- **Risk:** Expo Go runtime quirks.
  - **Mitigation:** dedicated Expo smoke app + CI matrix for browser + Expo runtime tests.
- **Risk:** Secret leakage via debugging.
  - **Mitigation:** centralized redaction utility + strict no-secret logging tests.

---

## Immediate Next Actions (what to do first on this branch)

1. Add `SparkConfig` + factory wiring + `SparkNode` stub.
2. Add `spark.real.test.ts` skeleton gated by env.
3. Add `spark-internal/` scaffolding with interfaces only.
4. Start Phase 2 with ECIES + test vectors before FROST aggregate logic.
