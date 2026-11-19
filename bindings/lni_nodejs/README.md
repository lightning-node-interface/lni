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

## Platform binary

The supported native target is Linux x64 with glibc, built on Ubuntu CI. The root
package contains JavaScript and generated TypeScript declarations. Its
exact-version optional dependency, `@sunnyln/lni-nodejs-linux-x64-gnu`, supplies
the addon. Do not omit optional dependencies.

Other operating systems, ARM64, and musl are not release targets. Linux release
binaries are built against glibc 2.28. The generated napi-rs loader still uses
the internal filename `lni_js.linux-x64-gnu.node`.

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
# Keep tarballs for a local consumer. Build Linux x64 first.
corepack yarn pack:platforms ./dist linux-x64-gnu
corepack yarn test:install ./dist
```

The second command tests the existing tarballs. `pack:platforms` requires the
Linux x64 binary and fails if it is missing. Root tarballs exclude binaries,
tests, scripts, Rust source, and environment files.

When changing the npm version, update `package.json`, run
`corepack yarn version:platforms`, and run `corepack yarn install` to update the
workspace lockfile. There is one package-manager lockfile, `yarn.lock`.

## Release workflow

[build-nodejs.yml](../../.github/workflows/build-nodejs.yml) builds and tests on
Ubuntu x64, tests a packed consumer install, and collects the native binary with
napi-rs before packing release tarballs. Release builds strip symbols. Pull requests, branch pushes, and manual
workflow runs only build, test, and upload artifacts.

Publishing requires a tag exactly matching `lni-nodejs-v<package.json version>`.
Generic repository tags such as `v0.26.1` do not release this package. The publish
job runs in the `npm-publish` GitHub environment, which waits for a maintainer's
approval, and authenticates with npm trusted publishing, so no npm token is
stored. It publishes the tested platform tarballs first, followed by the root
tarball, and a rerun skips packages already on npm. It never runs
`napi prepublish`, which can publish packages as a lifecycle side effect.

npm can only trust a workflow for a package that already exists. Before the
first release, a maintainer publishes each package once by hand, then adds the
trusted publisher (repository `lightning-node-interface/lni`, workflow
`build-nodejs.yml`, environment `npm-publish`) to each of them.
