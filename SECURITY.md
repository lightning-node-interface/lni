# Security Policy

## Reporting a Vulnerability

Please do not disclose suspected vulnerabilities in public issues, discussions,
pull requests, or other public channels.

GitHub private vulnerability reporting is not currently enabled for this
repository. To request a private reporting channel, open a GitHub issue titled
`Security contact request` containing only:

- Your preferred contact method or GitHub username.
- The affected LNI component and version, if known.
- No vulnerability details, credentials, wallet data, invoices, preimages, or
  reproduction steps.

A maintainer will arrange private follow-up. Reports should include affected
versions, impact, reproduction steps, and any suggested remediation once a
private channel is established.

Reports are accepted for any version. Remediation is prioritized for the latest
release and the `master` branch. Response and remediation times depend on
severity, complexity, and maintainer availability.

## System and Scope

LNI is a client library and collection of language bindings for interacting
with Lightning nodes, wallets, and hosted payment providers. This policy covers
first-party code and published artifacts in this repository, including:

- The Rust `lni` crate.
- TypeScript, Node.js, React Native, Kotlin, and Swift bindings.
- The optional Spark, Arkade, and Lexe adapters.
- Shared protocol parsing, HTTP transport, error normalization, and permission
  handling.
- Build, packaging, release, and repository-maintenance code.

LNI is not a hosted service. It runs inside applications controlled by library
users and communicates with caller-selected nodes, wallets, relays, and payment
providers.

## Threat Model and Trust Boundaries

Security-sensitive assets include wallet mnemonics, API keys, macaroons, runes,
passwords, NWC connection secrets, access tokens, payment preimages, wallet
state, and the authority to move funds.

Callers are trusted to decide which operations to perform and to provide
appropriate credentials and explicitly configured service endpoints.
Applications embedding LNI remain responsible for authenticating their own
users and authorizing calls into the library.

Potentially attacker-controlled data includes:

- BOLT 11 invoices, BOLT 12 offers, LNURL values, Lightning addresses, and
  payment metadata.
- Responses, errors, redirects, and event streams from remote services.
- Data received across JavaScript, native, and FFI boundaries.
- Files or storage values consumed by wallet adapters.
- Dependency and packaging inputs processed during builds or releases.

Important trust boundaries exist between application callers and LNI, LNI and
remote providers, shared Rust behavior and language bindings, wallet secrets
and application-visible output, and source code and published package
artifacts.

## Security Invariants

The following properties must hold:

- Secrets must not appear in logs, debug output, error messages, exceptions,
  generated artifacts, package contents, or test output.
- Credentials must be attached only to their intended protocol and endpoint.
  Redirects, proxies, error handling, and caller-controlled response data must
  not expose them.
- Payment recipient, amount, fee limits, network, and caller-selected options
  must not be silently changed across adapters or language boundaries.
- Malformed, unsupported, rejected, or unknown payment states must fail closed
  and must not be reported as settled.
- Provider-specific status-only payment behavior must remain explicit; an
  unavailable preimage must not be fabricated or treated as cryptographic
  proof.
- Parsers and protocol decoders must reject invalid input without panics,
  unbounded resource consumption, or unsafe behavior.
- Mnemonic and key generation must use cryptographically secure randomness.
  Wallet secrets and signing material must not cross an API boundary unless
  the documented API explicitly requires it.
- Rust implementations and language bindings must preserve security-relevant
  validation, error, permission, and payment semantics.
- Published packages must contain only intended runtime files and must not
  include tests, local configuration, credentials, or other development data.

## Reportable Findings and Severity Context

A finding is reportable when it demonstrates a realistic security impact in
first-party LNI code or artifacts. Examples include:

- Unauthorized payments, loss of funds, or exposure of wallet signing material.
- Disclosure of credentials, mnemonics, NWC secrets, or payment preimages.
- Authentication bypass or credentials being sent to an unintended endpoint.
- Payment destination, amount, fee, network, or settlement-status confusion.
- Validation differences between bindings that bypass a security control.
- Remotely reachable crashes or resource exhaustion from untrusted protocol or
  provider data.
- Build or packaging behavior that could publish secrets or attacker-controlled
  code.
- A vulnerable dependency with a demonstrated reachable path through LNI.

Severity depends on realistic reachability and impact:

- Critical: practical theft of funds, wallet compromise, or remote extraction
  of high-value secrets with little or no victim interaction.
- High: unauthorized payment behavior, credential disclosure, or a broadly
  reachable security-control bypass.
- Moderate: constrained information disclosure, denial of service, or a bypass
  requiring significant prerequisites.
- Low: limited security impact with narrow reachability and no credible path to
  funds or sensitive credentials.

## Out of Scope and Accepted Risk

The following are generally not reportable unless first-party LNI behavior
creates or materially worsens the impact:

- Vulnerabilities solely in third-party nodes, wallets, providers, relays, or
  upstream dependencies.
- Availability, policy, or API changes in third-party services.
- Expected network access to an endpoint explicitly configured by a trusted
  application caller.
- Operations performed with credentials intentionally granted the required
  permissions.
- Vulnerabilities in examples or development tooling that cannot affect a
  shipped library, published artifact, credential, or maintainer workflow.
- Public test vectors, obviously fake credentials, and documented example
  mnemonics that do not control real funds.
- General correctness, compatibility, or performance bugs without a concrete
  security consequence.
- Social engineering, phishing, physical attacks, or attacks requiring a
  compromised developer or user device.

Dependency version reports without evidence of reachability are useful
maintenance reports but are not automatically security vulnerabilities in LNI.

## Known Limitations and Compensating Controls

LNI depends on external nodes, wallet SDKs, payment providers, and protocol
libraries. Their behavior and security properties are outside LNI's direct
control.

The Spark and Arkade adapters are experimental and should not be used in
production. They may have narrower validation or platform coverage. This does
not exempt them from the security invariants above.

Some hosted providers expose status-only payment results without a preimage.
LNI documents and surfaces this distinction rather than treating an empty
preimage as proof of settlement.

Framework example projects may retain moderate development-toolchain
advisories when remediation requires a broader framework upgrade. Repository
security checks continue to reject high and critical advisories in those
projects and moderate-or-higher advisories in the primary TypeScript packages.

Maintainers can run the repository dependency and packaging checks with:

```sh
node scripts/security-check.mjs --full
```
