# Thread Context (Sanitized)

## Goal
- Build and publish a frontend-capable TypeScript port of LNI without UniFFI.
- Keep Spark out initially, then prepare clean handoff context so another model can implement Spark next.

## Current Package State
- Package: `@sunnyln/lni`
- Location: `bindings/typescript`
- Build/publish scripts are in `bindings/typescript/package.json`.
- Publish flow is ready (`prepack`, `pack:dry-run`, `publish:public`).

## Implemented TypeScript Nodes
- `PhoenixdNode`
- `ClnNode`
- `LndNode`
- `NwcNode`
- `StrikeNode`
- `SpeedNode`
- `BlinkNode`

Spark is intentionally not implemented yet.

## Important Architecture Decisions
- Pure TypeScript implementation (no UniFFI bindings).
- Frontend-first runtime model using `fetch`.
- Shared node interface in `bindings/typescript/src/types.ts`.
- Shared HTTP and helpers in:
  - `bindings/typescript/src/internal/http.ts`
  - `bindings/typescript/src/internal/transform.ts`
  - `bindings/typescript/src/internal/polling.ts`
- LNURL helpers in `bindings/typescript/src/lnurl.ts`.

## Test Strategy
- Integration tests are real-backend style (no mocks).
- Tests are split by node:
  - `bindings/typescript/src/__tests__/integration/*.real.test.ts`
- Shared test helpers:
  - `bindings/typescript/src/__tests__/integration/helpers.ts`
- Packaging excludes tests (`bindings/typescript/tsconfig.build.json` excludes `src/**/__tests__/**`).

## Security/Sanitization Notes
- No secrets are committed in TypeScript sources.
- Integration tests read env at runtime only.
- Verbose test logs that printed invoice objects were removed.
- Avoid storing:
  - API keys
  - macaroons/runes/passwords
  - NWC URIs
  - local absolute filesystem paths
  - internal hostnames/IPs

## Docs Updated
- Root README has install instructions for npm package and source fallback.
- TypeScript README has install/publish instructions and package import examples for `@sunnyln/lni`.

## Spark Handoff: What to Build Next
- Implement `SparkNode` in `bindings/typescript/src/nodes/spark.ts`.
- Export it from `bindings/typescript/src/index.ts`.
- Add Spark integration tests:
  - `bindings/typescript/src/__tests__/integration/spark.real.test.ts`
- Keep interface parity with existing nodes and Rust spark module behavior.

## Spark Reference Source (Rust)
- `crates/lni/spark/`
  - `api.rs`
  - `lib.rs`
  - `types.rs`

Use Rust Spark behavior as the source of truth for endpoint mapping, request/response handling, and method semantics.
