# AGENTS.md

This file defines stable, repository-level guidance for future coding agents.

## Primary Goal
- Maintain and extend LNI bindings with consistent API behavior across languages.
- Current priority for TypeScript handoff: implement Spark support in `bindings/typescript`.

## Spark Implementation Guidance (TypeScript)
- Treat Rust Spark implementation as source of truth:
  - `crates/lni/spark/api.rs`
  - `crates/lni/spark/lib.rs`
  - `crates/lni/spark/types.rs`
- Match existing TypeScript node conventions:
  - Implement class in `bindings/typescript/src/nodes/`.
  - Use shared types from `bindings/typescript/src/types.ts`.
  - Reuse helpers from `bindings/typescript/src/internal/*`.
  - Export from `bindings/typescript/src/index.ts`.

## Shared Helper Guidance
- Reuse or add shared Rust calculation, conversion, and validation helpers in:
  - `crates/lni/utils.rs`
- Reuse or add the corresponding TypeScript helpers in:
  - `bindings/typescript/src/internal/transform.ts`
- Keep helper semantics consistent across Rust and TypeScript, and avoid duplicating
  reusable helpers inside individual node adapters.

## Test/Validation Expectations
- Prefer real integration-style tests over mocks for node adapters.
- Add/maintain node-specific integration tests under:
  - `bindings/typescript/src/__tests__/integration/`
- Ensure `npm run typecheck` passes.
- Ensure `npm run pack:dry-run` succeeds and does not include tests/secrets.

## TypeScript Package Versioning
- Keep `@sunnyln/lni`, `@sunnyln/lni-arkade`, and `@sunnyln/lni-spark` on the same npm version unless a release explicitly says otherwise.
- When bumping the core TypeScript package, also bump Arkade and Spark package versions plus their `@sunnyln/lni` peer dependency ranges.
- Refresh the affected `package-lock.json` files after version changes so local `file:` package metadata points at the same version.

## Security Expectations
- Never commit credentials or local machine details.
- Do not print sensitive runtime values in tests/logging:
  - API keys, macaroons, runes, passwords, NWC URIs, full invoices/preimages.
- Keep examples sanitized.

## Handoff Context
- High-level sanitized thread context is in:
  - `docs/THREAD_CONTEXT.md`
