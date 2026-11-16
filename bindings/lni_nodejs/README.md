# @sunnyln/lni-nodejs

Native Node.js bindings for LNI. Supports Lexe, including Lexe Connect, exclusive
Bitcoin addresses, and receiving capability checks, plus LND, CLN, Phoenixd,
Blink, Strike, Speed, NWC, and LNURL helpers.

For a smaller Lexe-only build, use [@sunnyln/lni-lexe-nodejs](../nodejs-lexe/README.md).

Spark and Arkade are excluded from this package. The Node addon does not depend
on the Spark SDK. Experimental adapters remain in their separate packages.

```sh
npm install @sunnyln/lni-nodejs
```

```js
const { LexeNode } = require('@sunnyln/lni-nodejs')
const { withLexeNode } = require('@sunnyln/lni-nodejs/lexe')

// Keep credentials on the server. This helper closes the wallet on every exit.
const capabilities = await withLexeNode(credentials, (node) => node.getReceivingCapabilities())
```

The root entry point loads the native addon. The `/lexe` entry point loads it
only when a helper operation needs it. See [LEXE.md](LEXE.md) for Connect session
storage, authorization, and exclusive-address synchronization requirements.

ES modules can use a default import for the CommonJS root:

```js
import lni from '@sunnyln/lni-nodejs'
import { withLexeNode } from '@sunnyln/lni-nodejs/lexe'
const { LexeNode } = lni
```

## Platform binaries

The JavaScript and generated TypeScript declarations are in the root package.
Exact-version optional dependencies supply one native binary per platform:

| System       | Architecture  | Package suffix    |
| ------------ | ------------- | ----------------- |
| Linux, glibc | x64           | `linux-x64-gnu`   |
| Linux, glibc | arm64         | `linux-arm64-gnu` |
| macOS        | Intel         | `darwin-x64`      |
| macOS        | Apple Silicon | `darwin-arm64`    |
| Windows      | x64           | `win32-x64-msvc`  |

For example, Linux x64 installs `@sunnyln/lni-nodejs-linux-x64-gnu`. Package
managers select by OS, CPU, and libc. Do not omit optional dependencies. Linux
musl, Windows ARM, Android, and other targets are not published by this workflow.
Linux release binaries are built against glibc 2.28. Their generated napi-rs
loader still uses the internal filename `lni_js.<platform>.node`.

Use a Node.js server runtime, including serverless Node.js functions. This addon
cannot load in an Edge runtime or browser. Configure your bundler to externalize
`@sunnyln/lni-nodejs` and preserve its matching optional dependency.

## Local development and packed install checks

Use the pinned Yarn release in this directory and a Rust toolchain compatible
with the workspace lockfile. The Rust build also requires a C/C++ toolchain.
Node.js 22 is used by the build and test workflow.

```sh
corepack yarn install --immutable
corepack yarn build
corepack yarn typecheck
corepack yarn test
corepack yarn test:release
corepack yarn check:native-deps
corepack yarn pack:dry-run
corepack yarn test:install
```

`test:install` packs the root and current platform package, installs those
tarballs into a clean consumer using an isolated offline npm cache, loads the
real addon, checks CommonJS and ESM imports, and compiles a TypeScript consumer.
It checks that Lexe's receiving APIs are present and Spark and Arkade are absent.
It makes no provider requests and does not publish anything.

```sh
# Keep tarballs for a local consumer. Build the current platform first.
corepack yarn pack:platforms ./dist linux-x64-gnu
corepack yarn test:install ./dist
```

The second command tests the existing tarballs. Without a platform argument,
`pack:platforms` requires all five binaries and fails if any are missing. Root
tarballs exclude binaries, tests, scripts, Rust source, and environment files.

When changing the npm version, update `package.json`, run
`corepack yarn version:platforms`, and run `corepack yarn install` to update the
workspace lockfile. There is one package-manager lockfile, `yarn.lock`.

## Release workflow

[build-nodejs.yml](../../.github/workflows/build-nodejs.yml) builds and tests each
binary on its native architecture, tests a packed consumer install on each
platform, and collects all five binaries with napi-rs before packing release
tarballs. Release builds strip symbols. Pull requests, branch pushes, and manual
workflow runs only build, test, and upload artifacts.

Publishing requires a tag exactly matching `lni-nodejs-v<package.json version>`.
Generic repository tags such as `v0.26.1` do not release this package. The publish
job needs the repository's `NPM_TOKEN` secret and publishes the tested platform
tarballs first, followed by the root tarball. It never runs `napi prepublish`,
which can publish packages as a lifecycle side effect.
