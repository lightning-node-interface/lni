# Security Check Script

This directory contains repository maintenance scripts. The current security helper is:

```bash
node scripts/security-check.mjs
```

Run it from the repository root.

## What It Checks

`security-check.mjs` automates the dependency checks used to remediate the Socket dependency alert report.

It performs three groups of checks:

1. Socket report range check
   - Scans every tracked `package-lock.json` outside `node_modules`.
   - Fails if any dependency is still inside one of the vulnerable ranges from the report.
   - Covers the report packages such as `brace-expansion`, `lodash`, `js-yaml`, `ajv`, `minimatch`, `picomatch`, `esbuild`, `glob`, `tar`, `vite`, `yaml`, `rollup`, `left-pad`, and `es5-ext`.

2. Moderate-threshold npm audits
   - Runs `npm audit` and fails on `moderate`, `high`, or `critical` findings for packages that should currently be clean:

```text
bindings/typescript
bindings/typescript-spark
bindings/typescript-arkade
bindings/typescript-spark/examples/spark-web
bindings/lni_nodejs
```

3. High-threshold npm audits for framework examples
   - Runs `npm audit` and fails only on `high` or `critical` findings for:

```text
bindings/typescript-spark/examples/spark-expo-go
bindings/lni_react_native
```

These example packages still have moderate advisories in Expo / React Native CLI dependency chains. Fixing those requires broader framework-version decisions, so the script allows moderate findings there while still blocking high-severity regressions.

## Default Usage

```bash
node scripts/security-check.mjs
```

Expected successful output ends with:

```text
Security checks passed.
```

The default mode is intended for a quick dependency-security gate.

## Full Usage

```bash
node scripts/security-check.mjs --full
```

`--full` runs the default security checks, then also runs:

```text
npm --prefix bindings/typescript run typecheck
npm --prefix bindings/typescript-spark run typecheck
npm --prefix bindings/typescript-arkade run typecheck
npm --prefix bindings/typescript run pack:dry-run
```

Use full mode before handing off dependency-security changes.

`--with-build` is accepted as an alias for `--full`.

## Failure Behavior

The script exits with status `1` when a required check fails.

Common failure cases:

- A lockfile contains a package version still inside a Socket report vulnerable range.
- `npm audit` finds a vulnerability at or above the configured threshold for a package.
- `--full` is used and a typecheck or dry-pack command fails.

For framework examples, moderate advisories may be reported as ignored below the high threshold. That is expected unless the project decides to do a larger Expo or React Native toolchain upgrade.

## Notes

- Run from `/Users/nick/code/lni` or another checkout root.
- Use Node `20.19+` or `22.12+` when possible. Some updated tooling declares those engine ranges, and older Node versions may print engine warnings.
- The script uses only Node built-ins and npm; it does not add a package dependency.
