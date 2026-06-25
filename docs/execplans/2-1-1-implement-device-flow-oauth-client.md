# Implement GitHub device-flow OAuth client

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETED

## Purpose / big picture

Roadmap item `2.1.1` enables the appliance to authenticate to GitHub from a
headless virtual machine using the OAuth 2.0 device authorization grant. After
this plan is approved and implemented, an operator can start a login flow from
the appliance, copy a short user code into `https://github.com/login/device`
from another browser, and have `repovecd` poll until GitHub returns a usable
bearer token or a terminal failure.

The observable success criterion is a repository test binary that completes the
device flow against a local mock OAuth server, receives a valid token, and
stores that token encrypted at rest below `/etc/repovec/`. The implementation
must handle the important GitHub device-flow outcomes: `authorization_pending`,
`slow_down`, `expired_token`, `access_denied`, malformed responses, and
transport failures.

Implementation began after explicit approval in the 2026-06-02 user request.

## Constraints

- Keep this work scoped to roadmap item `2.1.1`. Do not implement repository
  listing from `2.2.1`, branch discovery from `2.2.2`, token refresh from
  `2.1.2`, permission checking from `2.1.3`, or the terminal user interface from
  `5.2`.
- The GitHub device flow must use the documented endpoints:
  `POST https://github.com/login/device/code` and
  `POST https://github.com/login/oauth/access_token`.
- The OAuth app `client_secret` must not be required for the device flow.
  GitHub's documentation states that the device flow uses `client_id` plus the
  device code and grant type.
- Respect the polling interval returned with the device code. A `slow_down`
  response uses additive backoff on the current active polling interval: the
  next poll delay increases by at least five seconds, or by the server-supplied
  interval when one is present. The server interval must not substitute for or
  reduce the active interval. This additive behaviour is the intended behaviour
  modelled by the core state machine.
- Store the GitHub access token encrypted at rest under `/etc/repovec/`. The
  token must not appear in logs, command arguments, snapshots, panic messages,
  or committed files.
- Keep pure state transitions, response interpretation, and error
  classification in `repovec-core`. Keep HTTP, filesystem, command execution,
  systemd credential handling, and journald logging in `repovecd` adapters.
- Use `RuntimePaths` for appliance path construction. Add first-class helper
  methods only where they clarify the `/etc/repovec` token path contract.
- Use `rstest` for focused unit tests and `rstest-bdd` for behavioural tests
  where the change affects an externally observable workflow.
- Use `proptest` when testing invariants across many response bodies, polling
  intervals, expiry times, or scope combinations.
- Use Verus or Kani only if implementation introduces a substantive invariant
  that example-based and property-based tests cannot cover well. This roadmap
  item is expected to need `proptest`, not formal proof.
- Use the repository Makefile targets for gates. Run gate commands
  sequentially and capture output with `tee` under `/tmp`.
- Run `coderabbit review --agent` after each major implementation milestone
  only after deterministic gates for that milestone pass.
- Do not mark roadmap item `2.1.1` done until implementation, tests,
  documentation, CodeRabbit concerns, and final gates are complete.
- Keep documentation in en-GB Oxford style, except for exact API names,
  protocol fields, crate names, and source quotations.

If satisfying the objective requires violating a constraint, stop
implementation, record the conflict in `Decision Log`, and ask for direction.

## Tolerances (exception triggers)

- Scope: if implementation requires changing more than 18 files or more than
  900 net lines outside tests and documentation, stop and ask for review.
- Public interface: if a public API outside the new GitHub-authentication
  module, `RuntimePaths`, or `repovecd` wiring must change, stop and ask for
  review.
- Dependencies: if more than one runtime OAuth/GitHub client crate is needed,
  stop and present the alternatives. `octocrab` and `oauth2` must not both
  become permanent runtime dependencies without explicit approval.
- Secret storage: if encrypted token storage cannot be implemented through
  `systemd-creds` or another concrete key-management design that does not store
  the encryption key beside the ciphertext, stop and present options.
- Platform: if Rocky 10's packaged `systemd-creds` cannot encrypt and decrypt a
  credential file under `/etc/repovec/`, stop and present alternatives.
- Polling: if the implementation needs a long-running unsupervised task in
  `repovecd`, stop and define cancellation and ownership before proceeding.
- Testing: if `make check-fmt`, `make typecheck`, `make lint`, or `make test`
  still fails after two focused fix attempts in a milestone, stop and report
  the failing log path.
- CodeRabbit: if CodeRabbit reports a correctness, security, or maintainability
  concern that is still valid after local verification, address it before the
  next milestone. If the concern conflicts with a constraint, stop and ask.
- Ambiguity: if the OAuth app registration model, client ID source, or token
  storage threat model has multiple valid interpretations that materially
  change the design, stop and present options.

## Risks

- Risk: token encryption requires key management, and the repository does not
  yet have a general secret-store abstraction. Severity: high. Likelihood:
  medium. Mitigation: prefer a `systemd-creds` based adapter because the
  appliance is already systemd-managed and the upstream mechanism supports
  encrypted credentials bound to local host material. Keep a fake token store
  for tests and stop if `systemd-creds` is not available or cannot satisfy the
  `/etc/repovec/` requirement.

- Risk: `octocrab` supports GitHub device-flow initiation, but its API may not
  expose enough control for local mock endpoints, polling delays, or detailed
  error classification. Severity: medium. Likelihood: medium. Mitigation: start
  with a small dependency spike. If `octocrab` cannot target the mock server
  cleanly, use the `oauth2` crate for the device authorization grant and
  document the decision.

- Risk: `oauth2-test-server` implements RFC 8628 device-code flow, but its
  endpoints differ from GitHub's endpoint paths. Severity: medium. Likelihood:
  medium. Mitigation: hide endpoint paths behind configuration, and use an
  adapter or mock wrapper that exercises the same request/response semantics as
  GitHub.

- Risk: polling can slow tests or make them flaky if tests wait in real time.
  Severity: medium. Likelihood: high. Mitigation: introduce a clock/sleeper
  port so unit and behavioural tests advance virtual time instead of sleeping.
  Reserve real sleeping only for a small local-server integration test, and
  keep intervals short there.

- Risk: access tokens, user codes, and device codes can leak through logs or
  failing assertions. Severity: high. Likelihood: medium. Mitigation: model
  secrets with non-revealing wrappers where practical, add tests that display
  errors without token material, and never snapshot raw OAuth responses
  containing tokens.

- Risk: the roadmap asks for a "test binary", while the repository currently
  uses unit tests, integration tests, and `rstest-bdd` scenarios rather than
  many auxiliary binaries. Severity: low. Likelihood: medium. Mitigation: add
  an explicit `crates/repovecd/examples/device-flow-test.rs` or a clearly named
  integration binary only if that is the least surprising way to meet the
  roadmap. If a test target provides the same observable contract, record the
  decision.

## Progress

- [x] 2026-06-02: Approved and began roadmap item `2.1.1`.
- [x] 2026-06-25: Added the pure device-flow state model in `repovec-core`,
  including redacted token/code wrappers, additive `slow_down` handling, and
  BDD/property coverage.
- [x] 2026-06-25: Added `repovecd` runtime ports and adapters for GitHub OAuth
  HTTP, prompt presentation, polling, encrypted token storage, and
  systemd-creds command execution.
- [x] 2026-06-25: Added the `device-flow-test` example and integration tests
  that exercise the mock OAuth server, token polling, and encrypted reload.
- [x] 2026-06-25: Updated operator, developer, roadmap, and technical-design
  documentation for the device-flow client and token persistence contract.
- [x] 2026-06-26: Closed review feedback for timeout bounds, redaction,
  injected clocks, observable metrics, atomic writes, stdin-based credential
  encryption, and restart-time permission revalidation.

Retrospective execution evidence is preserved in the linked artefact:
[device-flow OAuth client retrospective](2-1-1-implement-device-flow-oauth-client-retrospective.md).

## Surprises & discoveries

- Observation: `repovecd` is currently a startup stub that validates the
  checked-in systemd unit contract and exits after collecting arguments.
  Evidence: `crates/repovecd/src/main.rs`. Impact: this task should add a small
  runtime/application surface rather than integrate with an existing daemon
  loop.

- Observation: `octocrab` has a GitHub-specific
  `Octocrab::authenticate_as_device` API and authentication types for device
  codes. Evidence: Firecrawl scrape of `docs.rs/octocrab` version `0.51.0`.
  Impact: `octocrab` is a plausible first choice, but a spike must prove it
  supports test endpoint control and the required error semantics.

- Observation: `oauth2-test-server` advertises RFC 8628 device-code support and
  in-memory operation for Rust tests. Evidence: Firecrawl scrape of the
  `oauth2-test-server` crates.io page. Impact: it is the preferred mock OAuth
  server for behavioural and integration tests unless the endpoint mismatch
  becomes costly.

- Observation: `systemd-creds` supports encrypted credentials using AES-256-GCM
  with keys derived from a local TPM2 device, `/var/lib/systemd` host material,
  or both. Evidence: Firecrawl scrape of `https://systemd.io/CREDENTIALS/`.
  Impact: it is the preferred at-rest encryption mechanism to investigate
  before adding a bespoke Rust encryption dependency.

- Observation: the implementation host has `/usr/bin/systemd-creds` from
  systemd `257` with TPM2 support enabled. Evidence: `systemd-creds --version`
  during Milestone 1. Impact: the preferred encrypted-token adapter remains
  feasible for Milestone 3.

- Observation: `octocrab` `0.51.0` hard-codes the device-code endpoint as
  `/login/device/code` and the token endpoint as `/login/oauth/access_token`.
  It can change the base URI, but not those paths. Evidence: local registry
  source at `octocrab-0.51.0/src/auth.rs`. Impact: it is awkward to run against
  `oauth2-test-server`, whose device endpoints are `/device/code` and
  `/device/token`, and it would not give enough control over endpoint-level
  error classification.

- Observation: `cargo check --all-targets --all-features` did not expose the
  missing `cap-std` `fs_utf8` feature for the token-store module before the
  example binary was run. Evidence: `make typecheck`, `make lint`, and
  `make test` passed, but `cargo run -p repovecd --example device-flow-test`
  first failed with `could not find fs_utf8 in cap_std`. Impact: keep the
  explicit example-binary run as a required gate for this item, and retain the
  workspace `cap-std` dependency feature so the adapter compiles in ordinary
  binary builds.

- Observation: `oauth2-test-server` owns a Tokio `JoinHandle`, and the blocking
  `oauth2` HTTP client must not be driven inside the async runtime context.
  Evidence: the first example-binary run panicked with
  `Cannot drop a runtime in a context where blocking is not allowed`. Impact:
  the example now starts and controls the mock server with `Runtime::block_on`,
  but performs OAuth HTTP polling and token-store work in synchronous code.

- Observation: `tempfile::NamedTempFile` creates files with `0600` mode on Unix,
  but post-creation permission changes through `std::fs::File::set_permissions`
  are rejected by the repository's `whitaker-lint` capability policy. Evidence:
  CodeRabbit requested an explicit permission set for the `systemd-creds`
  plaintext input, and `make lint` rejected the attempted `std::fs`
  implementation. Impact: keep the input as a `NamedTempFile`, record the
  owner-only creation-mode assumption in code, and avoid bypassing the
  capability policy.

## Decision log

- Decision: Keep this branch as a pre-implementation planning pull request.
  Rationale: the user explicitly required plan approval before implementation.
  The branch can carry the ExecPlan for review without starting the OAuth
  client. Date/Author: 2026-05-26, Codex.

- Decision: Model the OAuth flow as a small state machine with driven ports for
  OAuth HTTP, token storage, clock/sleep and journald logging. Rationale: this
  protects the domain/application policy from HTTP and filesystem adapters
  without forcing a large architectural transplant. Date/Author: 2026-05-26,
  Codex with Wyvern architecture brief.

- Decision: Prefer `octocrab` first, but make the first implementation
  milestone a reversible dependency spike. Rationale: the roadmap says to
  leverage `octocrab` if possible. The `oauth2` crate remains the fallback
  because it exposes generic RFC 8628 primitives and is likely easier to point
  at a mock server. Date/Author: 2026-05-26, Codex with Firecrawl research.

- Decision: Prefer `oauth2-test-server` for the mock OAuth server.
  Rationale: it is a Rust-native in-memory server with RFC 8628 support,
  whereas the named alternatives may require a less direct Rust integration.
  Date/Author: 2026-05-26, Codex with Firecrawl research.

- Decision: Treat token encryption as a first-class milestone and stop if
  `systemd-creds` cannot satisfy the contract. Rationale: encrypted-at-rest
  storage is security-critical. A design that stores a generated encryption key
  beside the ciphertext would not provide a meaningful at-rest boundary.
  Date/Author: 2026-05-26, Codex with Wyvern architecture brief.

- Decision: use `oauth2` `5.0.0` rather than `octocrab` for the runtime device
  authorization protocol adapter. Rationale: `octocrab` satisfies live GitHub's
  endpoint paths but not the configurable mock endpoint requirement in this
  plan, while `oauth2` exposes RFC 8628 device-code URLs directly and keeps
  error handling testable. Date/Author: 2026-06-02, Codex.

- Decision: store the GitHub token as an encrypted credential file named
  `github-oauth-token.cred` under the configured `/etc/repovec` root, using a
  small `CredentialEncryptor` port and a `systemd-creds` adapter. Rationale:
  the port keeps token persistence testable without shelling out in unit tests,
  while the runtime adapter avoids storing encryption key material beside the
  ciphertext. Date/Author: 2026-06-02, Codex.

- Decision: implement the roadmap "test binary" as
  `crates/repovecd/examples/device-flow-test.rs`. Rationale: an example binary
  is directly runnable with Cargo, does not alter shipped daemon entrypoints,
  and still proves the three-step flow against `oauth2-test-server`.
  Date/Author: 2026-06-02, Codex.

## Outcomes & retrospective

Roadmap item `2.1.1` is implemented. Operators can run a GitHub device-flow
login through the `repovecd` runtime boundary, present the verification URI and
redacted user code before polling, receive a bearer token from a GitHub-like
OAuth server, and persist only encrypted credential material below
`/etc/repovec/`.

The final design keeps protocol policy in `repovec-core` and runtime I/O in
`repovecd`. The orchestration uses injected OAuth, storage, sleeper, and clock
ports so polling, expiry, terminal outcomes, and adapter failures are covered
without real sleeps. Token storage writes encrypted credentials atomically,
pipes plaintext to `systemd-creds` over stdin, and emits observable tracing
events for storage and device-flow outcomes.

Validation evidence is recorded in the retrospective artefact and final review
notes. The maintained gate set includes `make check-fmt`, `make typecheck`,
`make lint`, `make test`, `make markdownlint`, `make nixie`, and the
`device-flow-test` example against the local mock OAuth server.

## Context and orientation

The repository is a Rust workspace with shared code in `crates/repovec-core`, a
control-plane daemon crate in `crates/repovecd`, and additional binary crates
for the MCP daemon, terminal UI, CLI and CI helper. The current `repovec-core`
appliance modules validate static Qdrant and systemd contracts with pure Rust
functions, typed errors, `rstest` unit coverage, and `rstest-bdd` behavioural
scenarios.

`RuntimePaths` in `crates/repovec-core/src/lib.rs` already defines the
canonical appliance roots: `/etc/repovec` for configuration and secrets, and
`/var/lib/repovec` for mutable data. This task should extend that model only
where a named GitHub-token path makes callers clearer.

The technical design defines the GitHub device flow as follows: request a
device code from GitHub, show the user code and verification URI to the
operator, poll the token endpoint at or above the server-provided interval, and
finish when GitHub returns an access token or a terminal error. The device flow
does not require `client_secret`.

Important terms:

- Device authorization grant: the OAuth 2.0 flow from RFC 8628 for devices
  that cannot open a browser locally.
- Device code: the secret code the client sends to the token endpoint while
  polling.
- User code: the short code the operator enters in a browser at GitHub's
  device-login page.
- Verification URI: the URL where the operator enters the user code.
- Driven port: a trait owned by the core/application layer that an adapter
  implements for HTTP, storage, time, or logging.

## Documentation and skill signposts

Read these repository documents before implementation:

- `docs/roadmap.md`, especially roadmap item `2.1.1`.
- `docs/repovec-appliance-technical-design.md`, especially
  "Authentication: device flow".
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/reliable-testing-in-rust-via-dependency-injection.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- `docs/ortho-config-users-guide.md`.
- `docs/rstest-bdd-users-guide.md`.
- `docs/documentation-style-guide.md`.
- `docs/users-guide.md` and `docs/developers-guide.md` for existing operator
  and maintainer guidance.

Use these skills during implementation:

- `leta` for semantic code navigation and refactoring.
- `rust-router` to select any additional Rust skills.
- `hexagonal-architecture` to keep domain policy separate from adapters.
- `arch-crate-design` for crate and module placement.
- `rust-errors` for typed error boundaries and retry classification.
- `rust-async-and-concurrency` for polling, sleeper ports and task ownership.
- `commit-message` for file-based commits.
- `pr-creation` and `en-gb-oxendict-style` for pull request updates.
- `firecrawl-mcp` only when updated external documentation is needed.

## Plan of work

### Milestone 0: re-read and confirm baseline

Confirm the worktree is on `2-1-1-implement-device-flow-oauth-client`, the plan
is approved, and there are no unexpected local changes. Re-read this ExecPlan,
`AGENTS.md`, `docs/roadmap.md`, the device-flow section of the technical design,
`Cargo.toml`, `crates/repovec-core/src/lib.rs`, and
`crates/repovecd/src/main.rs`.

Run baseline gates before code changes:

```sh
make build 2>&1 | tee "/tmp/build-repovec-appliance-$(git branch --show-current).out"
make check-fmt 2>&1 | tee "/tmp/check-fmt-repovec-appliance-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-repovec-appliance-$(git branch --show-current).out"
make lint 2>&1 | tee "/tmp/lint-repovec-appliance-$(git branch --show-current).out"
make test 2>&1 | tee "/tmp/test-repovec-appliance-$(git branch --show-current).out"
```

If baseline failures are unrelated to this task, record them in
`Surprises & Discoveries` and ask before building on a failing baseline.

### Milestone 1: dependency and protocol spike

Add the smallest possible dependency surface to prove the OAuth client
approach. Start with `octocrab` because the roadmap prefers it when possible.
The spike must prove that the implementation can:

- request device codes from configurable endpoints;
- poll a configurable token endpoint;
- classify `authorization_pending`, `slow_down`, `expired_token` and
  `access_denied`;
- run against a local mock server without GitHub network access.

If `octocrab` cannot satisfy those points cleanly, remove the spike and use
`oauth2` version `5.0.0` or newer with its RFC 8628 device authorization APIs.
Record the decision in `Decision Log`.

Add `oauth2-test-server` as a development dependency if it can drive the
behavioural tests. If its endpoint shape prevents useful GitHub-like tests, use
a focused local HTTP mock such as `wiremock` and record why the named server
was not suitable.

Validation for this milestone is a compiling minimal test or prototype that
exercises the happy-path mock flow without storing a token. Run:

```sh
make check-fmt 2>&1 | tee "/tmp/check-fmt-repovec-appliance-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-repovec-appliance-$(git branch --show-current).out"
make lint 2>&1 | tee "/tmp/lint-repovec-appliance-$(git branch --show-current).out"
make test 2>&1 | tee "/tmp/test-repovec-appliance-$(git branch --show-current).out"
coderabbit review --agent
```

Commit the accepted spike with a file-based commit message.

### Milestone 2: core domain and state machine

Add a pure GitHub OAuth module under `crates/repovec-core/src/`, either
`github_oauth/` or `appliance/github_oauth/` depending on whether the code is
general repository-lifecycle domain or appliance-asset validation. Prefer the
top-level `github_oauth` module if it will later be used by `repovecd`,
`repovec-tui` and `repovectl`.

The module should include:

```rust
pub struct DeviceFlowRequest { /* client_id and scopes */ }
pub struct DeviceAuthorization { /* code, URI, expiry, interval */ }
pub struct AccessToken { /* non-revealing secret wrapper plus scopes */ }
pub enum DeviceFlowState { /* waiting, authorized, denied, expired */ }
pub enum DeviceFlowError { /* typed terminal and retryable failures */ }
pub trait Clock { /* deterministic time source */ }
```

The exact names may change to match local style, but the final API must keep
domain state free of `reqwest`, `octocrab`, `oauth2`, filesystem paths, process
commands and systemd-specific types.

Add `rstest` unit tests for response classification and state transitions. Add
`proptest` coverage for these invariants:

- malformed OAuth responses never panic;
- the next poll instant is never earlier than the active minimum interval;
- `slow_down` monotonically increases the minimum interval;
- expiry wins over continued polling once the device code has expired.

Validation is the full code gate plus CodeRabbit:

```sh
make check-fmt 2>&1 | tee "/tmp/check-fmt-repovec-appliance-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-repovec-appliance-$(git branch --show-current).out"
make lint 2>&1 | tee "/tmp/lint-repovec-appliance-$(git branch --show-current).out"
make test 2>&1 | tee "/tmp/test-repovec-appliance-$(git branch --show-current).out"
coderabbit review --agent
```

Commit this milestone separately.

### Milestone 3: repovecd adapters and encrypted token store

Add `repovecd` modules for the driven adapters:

- `crates/repovecd/src/github_device_flow.rs` for application orchestration;
- `crates/repovecd/src/github_oauth_client.rs` for HTTP/OAuth protocol calls;
- `crates/repovecd/src/github_token_store.rs` for encrypted token persistence;
- `crates/repovecd/src/github_auth_logging.rs` if logging grows beyond a few
  simple calls.

The token store should prefer `systemd-creds`:

- write plaintext only to memory or a short-lived secure temporary file;
- run `systemd-creds encrypt --name=repovec-github-oauth-token` to create
  `/etc/repovec/github-oauth-token.cred`;
- decrypt through `systemd-creds decrypt` or service credentials on startup;
- write files atomically and restrict permissions to the `repovec` service
  user where the platform allows it;
- keep command execution behind a trait, so tests can use a fake command
  runner.

If `systemd-creds` does not satisfy the contract, stop at this milestone and
present alternatives such as an `age`-based local recipient, a kernel keyring
adapter, or deferring encrypted storage until an appliance-wide secret-store
ADR exists. Do not silently downgrade to plaintext.

Add tests that verify no token value appears in `Debug`, `Display`, error
strings or snapshots. Add filesystem tests against a temporary directory and a
fake encryptor/decryptor; avoid mutating real `/etc/repovec` in tests.

Validation:

```sh
make check-fmt 2>&1 | tee "/tmp/check-fmt-repovec-appliance-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-repovec-appliance-$(git branch --show-current).out"
make lint 2>&1 | tee "/tmp/lint-repovec-appliance-$(git branch --show-current).out"
make test 2>&1 | tee "/tmp/test-repovec-appliance-$(git branch --show-current).out"
coderabbit review --agent
```

Commit this milestone separately.

### Milestone 4: behavioural and test-binary coverage

Add `rstest-bdd` feature coverage under
`crates/repovec-core/tests/features/github_oauth.feature` and a matching
`github_oauth_bdd.rs` harness. Cover at least:

- a happy path where the user code is presented and a token is eventually
  returned;
- `slow_down` increasing the next poll delay;
- `access_denied` producing a terminal user-cancelled result;
- `expired_token` producing a terminal expired result and requiring a new
  device-code request;
- malformed server responses producing typed errors.

Add the roadmap's test binary as `crates/repovecd/examples/device-flow-test.rs`
if still needed after the test suite shape is clear. The binary should target
the local mock server, not live GitHub, and should exit successfully only after
receiving and storing a token. If a Cargo integration test target is a better
fit than a binary, record the decision and keep the observable command in this
plan.

Validation:

```sh
make check-fmt 2>&1 | tee "/tmp/check-fmt-repovec-appliance-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-repovec-appliance-$(git branch --show-current).out"
make lint 2>&1 | tee "/tmp/lint-repovec-appliance-$(git branch --show-current).out"
make test 2>&1 | tee "/tmp/test-repovec-appliance-$(git branch --show-current).out"
coderabbit review --agent
```

Commit this milestone separately.

### Milestone 5: documentation and roadmap closeout

Update:

- `docs/repovec-appliance-technical-design.md` with the chosen OAuth client
  crate, polling policy, encrypted-token storage mechanism, and any ADR link.
- `docs/users-guide.md` with operator-visible login behaviour, token storage
  location, and the meaning of denied or expired device-flow results.
- `docs/developers-guide.md` with any new internally facing auth module,
  adapter, test-binary, or secret-store convention.
- `docs/contents.md` with this ExecPlan.
- `docs/roadmap.md` to mark `2.1.1` done only after implementation, gates and
  CodeRabbit concerns are complete.

Create an ADR if the encrypted-token design establishes a reusable secret-store
policy rather than a narrow GitHub-token adapter. Follow
`docs/documentation-style-guide.md` for ADR structure and link the ADR from the
technical design.

Run documentation gates and full code gates:

```sh
make build 2>&1 | tee "/tmp/build-repovec-appliance-$(git branch --show-current).out"
make fmt 2>&1 | tee "/tmp/fmt-repovec-appliance-$(git branch --show-current).out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-repovec-appliance-$(git branch --show-current).out"
make nixie 2>&1 | tee "/tmp/nixie-repovec-appliance-$(git branch --show-current).out"
make check-fmt 2>&1 | tee "/tmp/check-fmt-repovec-appliance-$(git branch --show-current).out"
make typecheck 2>&1 | tee "/tmp/typecheck-repovec-appliance-$(git branch --show-current).out"
make lint 2>&1 | tee "/tmp/lint-repovec-appliance-$(git branch --show-current).out"
make test 2>&1 | tee "/tmp/test-repovec-appliance-$(git branch --show-current).out"
coderabbit review --agent
```

Commit documentation and roadmap closeout separately if it is not naturally
part of a tiny final implementation commit.

## Concrete steps

All commands run from the repository root:

```sh
cd /home/leynos/.lody/repos/github---leynos---repovec-appliance/worktrees/dea4daaa-5947-440f-a953-3ab34db680e6
git branch --show-current
```

Expected branch:

```plaintext
2-1-1-implement-device-flow-oauth-client
```

Before each commit, inspect the diff:

```sh
git status --short
git diff
git diff --cached
```

Use file-based commit messages:

```sh
COMMIT_MSG_DIR=$(mktemp -d)
cat > "$COMMIT_MSG_DIR/COMMIT_MSG.md" << 'ENDOFMSG'
Implement GitHub device-flow state model

Add the pure OAuth device-flow state and response classification needed by
the repovecd adapters. Keep protocol and storage I/O outside the core module.
ENDOFMSG
git commit -F "$COMMIT_MSG_DIR/COMMIT_MSG.md"
rm -rf "$COMMIT_MSG_DIR"
```

Do not use `git commit -m`.

## Validation and acceptance

The feature is accepted only when all of these are true:

- A local mock OAuth server completes the device flow and returns a bearer
  token to the appliance client.
- The client handles `authorization_pending`, `slow_down`, `expired_token`,
  `access_denied`, malformed responses and transport failures with typed
  outcomes.
- The active polling interval never drops below the server's required minimum.
- The token is stored encrypted at rest below `/etc/repovec/` in the appliance
  contract, while tests use temporary roots and fake encryptors.
- No token, device code or user code leaks through normal logs, error display,
  snapshots or command-line arguments.
- `rstest` unit tests cover state transitions and error classification.
- `rstest-bdd` behavioural tests cover happy and unhappy user-visible flows.
- Property tests cover malformed responses and polling interval invariants.
- The roadmap's "test binary" success criterion is satisfied by either an
  explicit binary target or a documented Cargo test target with equivalent
  observable behaviour.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass.
- Documentation gates pass after documentation changes:
  `make fmt`, `make markdownlint`, and `make nixie`.
- `coderabbit review --agent` reports no unresolved concerns after the final
  implementation milestone.
- `docs/roadmap.md` marks `2.1.1` done only at final closeout.

Expected final gate transcript shape:

```plaintext
make check-fmt ... exits 0
make typecheck ... exits 0
make lint ... exits 0
make test ... exits 0
make markdownlint ... exits 0
make nixie ... exits 0
```

## Idempotence and recovery

All tests must avoid writing to real `/etc/repovec`. Use temporary roots and
dependency injection for filesystem, encryption, clock and command execution.

The token-store adapter must write encrypted credentials atomically: create a
temporary file in the target directory, fsync where practical, then rename into
place. If encryption or write fails, leave the previous encrypted token in
place and return a typed storage error.

The OAuth polling loop must be restartable. If `repovecd` exits before the user
authorizes the device code, a later run may start a new flow rather than trying
to resume an old device code.

Rollback is normal Git rollback. Because commits are milestone-sized, revert
the most recent milestone commit if a later gate or review uncovers a design
problem.

## Artifacts and notes

External research used for this draft:

- GitHub Docs, "Authorizing OAuth apps": confirms the device-flow endpoints,
  no `client_secret` requirement, polling interval, `slow_down`,
  `expired_token`, `access_denied`, and other device-flow errors. URL:
  `https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps`
- `octocrab` `0.51.0` docs: shows `Octocrab::authenticate_as_device` and
  device-code authentication types. URL:
  `https://docs.rs/octocrab/latest/octocrab/struct.Octocrab.html`
- `oauth2` `5.0.0` docs: shows a generic client with an RFC 8628 device
  authorization endpoint and `exchange_device_code` support. URL:
  `https://docs.rs/oauth2/latest/oauth2/struct.Client.html`
- `oauth2-test-server` crates.io page: shows an in-memory Rust OAuth test
  server with RFC 8628 device-code support. URL:
  `https://crates.io/crates/oauth2-test-server`
- systemd credentials documentation: shows encrypted service credentials,
  `systemd-creds`, and AES-256-GCM credential encryption bound to local system
  material. URL: `https://systemd.io/CREDENTIALS/`

Wyvern planning briefs used for this draft:

- Repository layout brief: place pure domain in `repovec-core`; place runtime
  adapters in `repovecd`; extend `RuntimePaths` only where useful.
- Architecture/testing brief: use driven ports for OAuth API, token store,
  clock and logging; use typed errors; protect token storage boundaries.
- Documentation/gates brief: update the technical design, users guide,
  developers guide, documentation index, and roadmap; run Makefile gates and
  CodeRabbit at major milestones.

## Interfaces and dependencies

Final naming may change to match the implementation, but the completed feature
should expose these concepts.

In `repovec-core`:

```rust
pub struct GithubDeviceFlowConfig {
    pub client_id: GithubOAuthClientId,
    pub scopes: GithubOAuthScopes,
}

pub struct DeviceAuthorization {
    pub user_code: UserCode,
    pub verification_uri: VerificationUri,
    pub expires_at: DeviceCodeExpiry,
    pub interval: PollInterval,
}

pub enum PollOutcome {
    Pending { next_poll_after: PollInterval },
    SlowDown { next_poll_after: PollInterval },
    Authorized(StoredAccessToken),
    Expired,
    AccessDenied,
}
```

In `repovecd`:

```rust
pub trait GithubDeviceFlowApi {
    async fn request_device_code(
        &self,
        config: &GithubDeviceFlowConfig,
    ) -> Result<DeviceAuthorization, GithubAuthAdapterError>;

    async fn poll_access_token(
        &self,
        authorization: &DeviceAuthorization,
    ) -> Result<PollOutcome, GithubAuthAdapterError>;
}

pub trait GithubTokenStore {
    fn load(&self) -> Result<Option<StoredAccessToken>, GithubTokenStoreError>;
    fn save(&self, token: &StoredAccessToken) -> Result<(), GithubTokenStoreError>;
    fn delete(&self) -> Result<(), GithubTokenStoreError>;
}
```

Use `octocrab = "0.51.0"` if the spike proves it fits. Otherwise use
`oauth2 = "5.0.0"` or newer. Add `oauth2-test-server` as a dev-dependency if it
can support the local behavioural flow. All dependency versions must use
caret-compatible Cargo requirements.

## Revision note

- Initial draft created on 2026-05-26 after repository inspection, Wyvern
  planning briefs, and Firecrawl research. This draft authorizes no
  implementation until explicitly approved.
