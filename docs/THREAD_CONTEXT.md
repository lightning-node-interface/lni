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
