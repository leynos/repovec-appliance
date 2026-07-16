# Validate Qdrant liveness from Rust

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item `1.2.3` completes the runtime half of the local Qdrant contract.
Roadmap items `1.2.1` and `1.2.2` already prove, through static validation,
that the checked-in Quadlet binds Qdrant to loopback and injects the API key.
This work adds the runtime proof: a Rust health-check function in
`repovec-core` connects to Qdrant's gRPC endpoint at `127.0.0.1:6334`, supplies
the stored API key from `/etc/repovec/qdrant-api-key`, and confirms Qdrant is
ready before dependent appliance services proceed.

After implementation, a maintainer can observe success by starting a Qdrant
container with the packaged configuration and running the new integration test.
The test starts Qdrant, calls the Rust liveness check with the generated key,
and fails if the service is unreachable, unauthenticated, or not ready.

## Constraints

- Implementation was approved by the user on `2026-06-02T00:12:58+02:00`.
  Continue milestone-by-milestone within the tolerances below.
- Keep scope to roadmap item `1.2.3` only. Do not implement repository
  discovery, indexing, full service status APIs, terminal-session handling,
  or resize-event propagation. Generic interactive-session exit-code reporting
  is out of scope, but daemon startup failures should still exit with status
  `1`.
- The prompt's completion criteria about interactive sessions, terminal
  resize events, and exit codes do not match roadmap item `1.2.3`. Treat them
  as contradictory boilerplate unless the user explicitly re-scopes this task.
- Preserve the existing static validators:
  `repovec_core::appliance::qdrant_quadlet` remains a pure Quadlet text
  contract validator, and `repovec_core::appliance::systemd_units` remains a
  pure systemd unit text contract validator.
- Add runtime Qdrant probing as a sibling appliance module, not by expanding
  `QdrantQuadletError` or making static validators perform I/O.
- The default Qdrant endpoint remains `http://127.0.0.1:6334`, matching the
  checked-in Quadlet gRPC loopback binding.
- The default API-key path remains `/etc/repovec/qdrant-api-key`. The key file
  stores the raw key, not an environment assignment.
- Secrets must not be logged, included in snapshots, passed as process
  arguments, committed to the repository, or exposed in assertion messages.
- Public errors must be semantic Rust error enums. Do not expose `anyhow` or
  `eyre` from library APIs.
- Keep domain and policy logic separate from adapters. The health policy
  should describe what counts as ready; filesystem reads, Qdrant gRPC calls,
  and container startup belong at the adapter boundary.
- Use `rstest` for unit and integration fixtures, and use `rstest-bdd` when a
  behavioural scenario expresses externally observable acceptance better than a
  unit test.
- Use property tests only when the implementation introduces an invariant over
  a range of values or state transitions. Do not add Kani or Verus proofs for
  simple protocol orchestration.
- Follow repository documentation guidance: update
  `docs/repovec-appliance-technical-design.md`, `docs/users-guide.md`,
  `docs/developers-guide.md`, `docs/contents.md`, and `docs/roadmap.md` when
  the implementation lands.
- Use the repository Makefile gates. Run `make check-fmt`, `make typecheck`,
  `make lint`, and `make test` sequentially after each major implementation
  milestone, capturing output with `tee` under `/tmp`.
- For documentation changes, run `make fmt`, `make markdownlint`, and
  `make nixie` where relevant, also sequentially with `tee` logs.
- Run `coderabbit review --agent` only after deterministic local gates pass
  for the relevant milestone, and resolve all applicable concerns before
  continuing.
- Commit each completed, gated milestone. Use a file-based commit message with
  `git commit -F`; do not use `git commit -m`.

## Tolerances (exception triggers)

- Scope: if implementation requires changing more than 14 files or more than
  850 net lines outside tests and documentation, stop and ask for review.
- Interface: if a public API outside `repovec_core::appliance` or the daemon
  startup wrappers must change, stop and ask for review.
- Dependencies: if implementation needs a runtime dependency other than
  `qdrant-client`, `tokio`, or a small semantic error helper such as
  `thiserror`, stop and ask for review. Development dependencies already in the
  workspace, including `rstest`, `rstest-bdd`, `insta`, and `proptest`, do not
  trigger this tolerance.
- Protocol: if Qdrant's Rust client cannot authenticate the gRPC
  `HealthCheck` call with the `api-key` metadata header, stop and present the
  alternatives rather than falling back silently to REST.
- Platform: if a local Podman integration test cannot bind to
  `127.0.0.1:6334` because a host Qdrant instance is already listening, use a
  dynamic loopback host port for the test.
- Security: if a test or diagnostic would expose the API key, stop and
  redesign that path.
- Testing: if `make check-fmt`, `make typecheck`, `make lint`, or `make test`
  still fails after two focused fix attempts for a milestone, stop and report
  the failing log files.
- Review: if CodeRabbit raises a correctness, security, or maintainability
  concern that conflicts with this plan, update the plan's `Decision Log` and
  ask for direction before continuing.
- Ambiguity: if multiple valid interpretations materially affect whether
  daemons should fail closed or merely report degraded Qdrant status, stop and
  present the trade-offs.

## Risks

Risk: the integration test needs a working container runtime, but CI or a
developer workstation may not allow Podman container startup. Severity: medium.
Likelihood: high. Mitigation: make the live Qdrant test explicitly opt-in or
ignored by default, document the required environment, and keep deterministic
unit and behavioural coverage in the default test set.

Risk: adding `qdrant-client` pulls in `tonic`, `prost`, and async runtime
dependencies, increasing compile cost for a crate that currently performs
mostly static validation. Severity: medium. Likelihood: high. Mitigation: add
the dependency at the workspace level with an explicit caret version, consider
`default-features = false` if `health_check` works without optional features,
and avoid adding parallel direct dependencies unless code uses them directly.

Risk: `qdrant-client` performs a compatibility check when building a client,
which can obscure the explicit liveness call or produce less precise errors.
Severity: medium. Likelihood: medium. Mitigation: use
`Qdrant::from_url(...).api_key(...).skip_compatibility_check().build()` and
make the explicit `health_check().await` the single readiness signal.

Risk: daemon startup could fail before Qdrant is ready even though systemd is
about to finish starting it. Severity: medium. Likelihood: medium. Mitigation:
implement bounded retry or timeout policy around the health check, document the
timeout, and keep it short enough to let systemd restart policy handle
persistent failures.

Risk: a malformed or empty API-key file can turn into an authentication or
metadata error that exposes the key in the error string. Severity: high.
Likelihood: medium. Mitigation: validate the key before building the Qdrant
client and map client errors into redacted semantic variants.

Risk: the prompt asks for `rstest-bdd` and end-to-end coverage "where
applicable", but a BDD wrapper around low-level gRPC errors could duplicate
unit tests without improving confidence. Severity: low. Likelihood: medium.
Mitigation: use BDD for user-observable health semantics only, such as "a
dependent daemon refuses to start when Qdrant is unavailable"; keep protocol
edge cases as unit or integration tests.

Risk: a future implementation may be tempted to use REST `/healthz` because
Qdrant documents that endpoint clearly. Severity: medium. Likelihood: medium.
Mitigation: record that the roadmap requires gRPC at `127.0.0.1:6334`; REST
`/healthz` is useful prior art but is not the implementation target unless the
user approves a scope change.

## Relevant documentation and skills

Before implementation, load these skills and keep them active for their
specific parts of the work:

- `leta`: use LSP-backed navigation for Rust symbols, references, and module
  relationships.
- `rust-router`: route follow-on Rust questions to the smallest relevant
  Rust skill.
- `rust-errors`: use for the public `QdrantLivenessError` enum and error
  mapping.
- `rust-async-and-concurrency`: use for async runtime, timeout, retry, and
  daemon startup blocking boundaries.
- `domain-cli-and-daemons`: use if wiring liveness into `repovecd` and
  `repovec-mcpd` startup semantics.
- `hexagonal-architecture`: use to keep policy, ports, and adapters separated
  without imposing a larger pattern than the repository needs.
- `nextest`: use only if debugging `cargo nextest` behaviour from
  `make test`.
- `commit-message` and `pr-creation`: use for milestone commits and pull
  request updates.

Repository references to read before editing:

- `docs/roadmap.md`, item `1.2.3`.
- `docs/repovec-appliance-technical-design.md`, especially "Qdrant under
  Podman + systemd".
- `docs/users-guide.md`, "Qdrant service".
- `docs/developers-guide.md`, "Appliance module".
- `docs/rust-testing-with-rstest-fixtures.md`.
- `docs/rust-doctest-dry-guide.md`.
- `docs/reliable-testing-in-rust-via-dependency-injection.md`.
- `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- `docs/ortho-config-users-guide.md`.
- `docs/rstest-bdd-users-guide.md`.
- `crates/repovec-core/src/appliance/qdrant_quadlet/`.
- `crates/repovec-core/tests/qdrant_quadlet_bdd.rs`.
- `packaging/systemd/qdrant.container`.
- `packaging/libexec/repovec-qdrant-api-key`.

External source evidence gathered during planning:

- Qdrant security documentation states that API-key authentication uses the
  `api-key` header and may be configured through `QDRANT__SERVICE__API_KEY`.
- Qdrant API documentation exposes REST `GET /healthz` with `api-key`
  authentication, but this plan keeps the roadmap's gRPC requirement.
- `qdrant-client` `1.18.0` is the current crates.io Rust client version
  reported by `cargo search qdrant-client --limit 3` on `2026-05-26`.
- The downloaded `qdrant-client` `1.18.0` source includes
  `qdrant.Qdrant/HealthCheck`, `Qdrant::from_url("http://127.0.0.1:6334")`,
  `QdrantBuilder::api_key(...)`, `skip_compatibility_check()`, and
  `Qdrant::health_check().await`.

## Implementation plan

### Milestone 1: Shape the liveness API and dependency boundary

Add a new sibling module under `crates/repovec-core/src/appliance/`, for example
`qdrant_liveness`, and export it from
`crates/repovec-core/src/appliance/mod.rs`. Keep it separate from
`qdrant_quadlet` so static asset validation and runtime service probing remain
distinct responsibilities.

Define small public types:

- `QdrantLivenessConfig`, carrying the endpoint, API-key file path and timeout
  policy. The default endpoint is `http://127.0.0.1:6334`, and the default key
  path is `/etc/repovec/qdrant-api-key`.
- `QdrantLivenessReport`, carrying the positive readiness evidence returned by
  Qdrant, such as title, version and optional commit. Do not store the key.
- `QdrantLivenessError`, a semantic error enum covering missing key file,
  unreadable key file, empty key, malformed key metadata, invalid endpoint,
  connection or timeout failure, authentication failure, and non-ready or
  incompatible Qdrant responses.

Add only the dependencies the code actually uses. The likely baseline is:

```toml
qdrant-client = { version = "1.18.0", default-features = false }
thiserror = "2.0.17"
tokio = { version = "1.48.0", features = ["rt-multi-thread", "time"] }
```

Confirm exact versions with `cargo info` before implementation. If `thiserror`
is unnecessary because a manual `std::error::Error` implementation stays small
and clear, omit it.

Acceptance for this milestone:

- `repovec_core::appliance::qdrant_liveness` compiles.
- The new public types have Rustdoc examples that do not require a live
  Qdrant instance.
- No existing public static validator API changes.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass.
- `coderabbit review --agent` reports no unresolved applicable concerns.

### Milestone 2: Implement key loading and unit tests

Implement key-file loading as an adapter around a pure validation function. The
pure function should accept key bytes or a borrowed string and return a
redacted domain value such as `QdrantApiKey`. It should reject empty keys, keys
containing newlines, and values that cannot be used as gRPC metadata.

Use `rstest` cases for happy and unhappy paths:

- valid generated-style hex key succeeds;
- missing file maps to a missing-key or read error;
- empty file is rejected;
- newline-suffixed key is rejected rather than trimmed silently;
- invalid metadata characters are rejected and redacted in `Display`;
- errors never include the secret value.

Acceptance for this milestone:

- Unit tests fail before the key loader exists and pass after implementation.
- Public error `Display` strings are stable, useful, and redacted.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass.
- `coderabbit review --agent` reports no unresolved applicable concerns.

### Milestone 3: Implement the gRPC health check

Build a `qdrant-client` client with the configured endpoint and API key. Use
`skip_compatibility_check()` so client construction does not perform an
implicit network probe before the explicit liveness check. Apply the configured
timeout around `client.health_check().await`.

Map Qdrant and tonic errors into `QdrantLivenessError` without leaking the key.
Authentication failures should be distinct from connection failures because
operators need different remediation: regenerate or repair the key for auth
failures, and inspect `qdrant.service` for connection failures.

Acceptance for this milestone:

- A function such as `check_qdrant_liveness(config).await` connects by gRPC
  and returns `QdrantLivenessReport` on success.
- The report includes non-secret server information from Qdrant's health
  reply.
- Wrong key, closed port and timeout paths are mapped to distinct semantic
  errors where the upstream status allows it.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass.
- `coderabbit review --agent` reports no unresolved applicable concerns.

### Milestone 4: Add live Qdrant integration coverage

Add an opt-in integration test under `crates/repovec-core/tests/`, for example
`qdrant_liveness_integration.rs`. Keep helper setup fallible and scoped to the
test body so default test discovery can skip the live cases cleanly.

The test should start `docker.io/qdrant/qdrant:v1.18.1` with Podman, set
`QDRANT__SERVICE__API_KEY` from a temporary key value, bind the container gRPC
port to loopback, wait for bounded readiness, call the Rust liveness function,
and then clean up the container. The default path should skip these tests with
`#[ignore]`; developers run them explicitly with `-- --ignored`.

Prefer a dynamic host port if implementation discovers that fixed
`127.0.0.1:6334` would make the test flaky on developer machines. If the test
uses a dynamic host port, keep the production default at `127.0.0.1:6334` and
document the test-only override.

Cover at least:

- happy path with the correct key;
- wrong API key;
- service unavailable or closed port;
- timeout while waiting for startup.

Acceptance for this milestone:

- The default `make test` path remains deterministic in environments without
  Podman.
- Running the opt-in test on a host with Podman starts Qdrant, proves the
  health check, and removes its container afterwards.
- The test logs do not print the API key.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass.
- The opt-in integration command is documented with expected success output.
- `coderabbit review --agent` reports no unresolved applicable concerns.

### Milestone 5: Wire startup behaviour where it is observable

Decide whether `repovecd`, `repovec-mcpd`, or both should call the liveness
check at startup. The current services already validate the checked-in systemd
unit contract before doing other work. The likely implementation is to make
`repovecd` fail closed when Qdrant is not live, and either have `repovec-mcpd`
rely on `repovecd.service` ordering or perform the same check if it will
directly depend on Qdrant.

Keep this logic testable by injecting the health-check function into a small
startup helper. Do not make unit tests open network sockets. Use fake success
and failure closures to assert exit decision and logging path.

Acceptance for this milestone:

- Dependent daemons do not proceed silently when Qdrant is unavailable.
- Startup tests verify success and failure decisions through injected
  health-check functions.
- The process exit code remains `1` for fatal startup contract failures.
- `make check-fmt`, `make typecheck`, `make lint`, and `make test` pass.
- `coderabbit review --agent` reports no unresolved applicable concerns.

### Milestone 6: Update documentation and roadmap state

Update documentation only after the runtime behaviour and tests are in place:

- `docs/repovec-appliance-technical-design.md`: describe the gRPC liveness
  check, startup sequencing, timeout policy and failure handling.
- `docs/users-guide.md`: tell operators how to inspect Qdrant liveness
  failures without printing the key.
- `docs/developers-guide.md`: document the new runtime health module and the
  opt-in Qdrant integration test.
- `docs/contents.md`: link this ExecPlan.
- `docs/roadmap.md`: mark `1.2.3` done only after implementation, validation,
  documentation, and review are complete.

Run documentation gates:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/fmt-repovec-appliance-1-2-3-validate-qdrant-liveness-from-rust.out
set -o pipefail
make markdownlint 2>&1 | tee /tmp/markdownlint-repovec-appliance-1-2-3-validate-qdrant-liveness-from-rust.out
set -o pipefail
make nixie 2>&1 | tee /tmp/nixie-repovec-appliance-1-2-3-validate-qdrant-liveness-from-rust.out
```

Acceptance for this milestone:

- The design, user, developer and roadmap documentation match the code.
- Roadmap item `1.2.3` is marked done.
- `make check-fmt`, `make typecheck`, `make lint`, `make test`,
  `make markdownlint`, and `make nixie` pass.
- `coderabbit review --agent` reports no unresolved applicable concerns.

## Validation commands

For each implementation milestone, run these gates sequentially:

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/check-fmt-repovec-appliance-1-2-3-validate-qdrant-liveness-from-rust.out
set -o pipefail
make typecheck 2>&1 | tee /tmp/typecheck-repovec-appliance-1-2-3-validate-qdrant-liveness-from-rust.out
set -o pipefail
make lint 2>&1 | tee /tmp/lint-repovec-appliance-1-2-3-validate-qdrant-liveness-from-rust.out
set -o pipefail
make test 2>&1 | tee /tmp/test-repovec-appliance-1-2-3-validate-qdrant-liveness-from-rust.out
```

For the opt-in live Qdrant integration test, run the documented command after
the default gates pass. The exact command should be finalized during
implementation, but it should follow this shape:

```sh
set -o pipefail
cargo test -p repovec-core --test qdrant_liveness_integration -- --ignored 2>&1 \
  | tee /tmp/qdrant-integration-repovec-appliance-1-2-3-validate-qdrant-liveness-from-rust.out
```

For documentation milestones, also run:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/fmt-repovec-appliance-1-2-3-validate-qdrant-liveness-from-rust.out
set -o pipefail
make markdownlint 2>&1 | tee /tmp/markdownlint-repovec-appliance-1-2-3-validate-qdrant-liveness-from-rust.out
set -o pipefail
make nixie 2>&1 | tee /tmp/nixie-repovec-appliance-1-2-3-validate-qdrant-liveness-from-rust.out
```

After deterministic gates pass for each major milestone, run:

```sh
coderabbit review --agent
```

Resolve all applicable CodeRabbit concerns before moving to the next milestone.

## Progress

- [x] (2026-05-26T01:25:49+02:00) Loaded the requested `leta`,
  `hexagonal-architecture`, and `rust-router` skills, plus `execplans`,
  `firecrawl-mcp`, `commit-message`, `pr-creation`, and `en-gb-oxendict-style`
  for this planning and PR task.
- [x] (2026-05-26T01:25:49+02:00) Created the leta workspace for this
  worktree with `leta workspace add`.
- [x] (2026-05-26T01:25:49+02:00) Confirmed the starting branch was
  `feat/plan-qdrant-rust-health`, the worktree was clean, and the branch was not
  `main`.
- [x] (2026-05-26T01:25:49+02:00) Renamed the branch to
  `1-2-3-validate-qdrant-liveness-from-rust` before creating any commits.
- [x] (2026-05-26T01:25:49+02:00) Read `docs/roadmap.md`,
  `docs/repovec-appliance-technical-design.md`, `docs/users-guide.md`,
  `docs/developers-guide.md`, `Makefile`, `Cargo.toml`, `crates/repovec-core`,
  and existing Qdrant-related execplans.
- [x] (2026-05-26T01:25:49+02:00) Used two Wyvern agents for read-only
  planning review: one for repository and documentation scope, and one for Rust
  API and testing strategy.
- [x] (2026-05-26T01:25:49+02:00) Used Firecrawl to verify Qdrant security
  documentation, Qdrant health endpoint prior art, and Rust client
  documentation.
- [x] (2026-05-26T01:25:49+02:00) Checked local `cargo search` and downloaded
  `qdrant-client` `1.18.0` metadata/source to verify the current Rust client,
  gRPC health method, API-key metadata header, and builder methods.
- [x] (2026-05-26T01:25:49+02:00) Drafted this pre-implementation ExecPlan.
- [x] (2026-05-26T01:35:00+02:00) Added this ExecPlan to
  `docs/contents.md` for documentation discoverability.
- [x] (2026-05-26T01:42:00+02:00) Ran `make check-fmt`, `make typecheck`,
  `make lint`, and `make test`; all passed with logs under `/tmp`.
- [x] (2026-05-26T01:43:00+02:00) Ran targeted `markdownlint-cli2` on
  `docs/contents.md` and this ExecPlan; it passed with zero errors.
- [x] (2026-05-26T01:43:00+02:00) Ran `make markdownlint` and `make nixie`;
  both passed.
- [x] (2026-06-02T00:12:58+02:00) Received explicit approval to implement the
  planned functionality.
- [x] (2026-06-02T00:15:34+02:00) Milestone 1 complete: added the
  `repovec_core::appliance::qdrant_liveness` public API, dependency
  declarations, and deterministic unit coverage for non-I/O domain values.
- [x] (2026-06-02T00:15:34+02:00) Milestone 1 gates passed:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test`. The test
  run executed 195 nextest tests, 13 `repovec-ci` doctests, and 20
  `repovec-core` doctests.
- [x] (2026-06-02T00:27:29+02:00) Resolved Milestone 1 CodeRabbit findings:
  simplify duplicate API-key byte validation logic, add property tests for the
  printable-ASCII API-key invariant, and add report accessor unit tests.
- [x] (2026-06-02T00:27:29+02:00) Re-ran Milestone 1 gates after the
  CodeRabbit fixes. `make check-fmt`, `make typecheck`, `make lint`, and
  `make test` passed. The test run executed 199 nextest tests, 13 `repovec-ci`
  doctests, and 20 `repovec-core` doctests.
- [x] (2026-06-02T00:36:47+02:00) Re-ran CodeRabbit for Milestone 1 after
  fixes; it completed with zero findings.
- [x] (2026-06-02T00:42:57+02:00) Milestone 2 complete: implemented
  capability-oriented API-key file loading, preserving distinct missing-file,
  unreadable-file, empty-key and invalid-key errors.
- [x] (2026-06-02T00:42:57+02:00) Milestone 2 gates passed:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test`. The test
  run executed 204 nextest tests, 13 `repovec-ci` doctests, and 21
  `repovec-core` doctests.
- [x] (2026-06-02T01:32:13+02:00) CodeRabbit completed Milestone 2 review
  with zero findings after two recoverable rate-limit backoffs.
- [x] (2026-06-02T01:35:12+02:00) Milestone 3 complete: implemented the
  async gRPC liveness check with `qdrant-client`, explicit timeout handling,
  invalid endpoint mapping, authentication classification, and health-reply
  conversion.
- [x] (2026-06-02T01:35:12+02:00) Milestone 3 gates passed:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test`. The test
  run executed 211 nextest tests, 13 `repovec-ci` doctests, and 22
  `repovec-core` doctests.
- [x] (2026-06-02T01:53:41+02:00) CodeRabbit completed Milestone 3 review
  with zero findings after one recoverable rate-limit backoff.
- [x] (2026-06-02T02:11:08+02:00) Milestone 4 complete: added an
  ignored, opt-in live Qdrant integration test that starts a rootless Podman
  container, uses a dynamic loopback gRPC port, validates success and failure
  paths, and skips Podman startup in default `make test`.
- [ ] (2026-06-02T02:25:36+02:00) Live integration testing found that
  Qdrant's gRPC health endpoint does not reject a wrong API key. The liveness
  check now performs `health_check()` for readiness and `list_collections()` as
  a lightweight authenticated gRPC operation before reporting success.
- [x] (2026-06-02T02:30:44+02:00) Milestone 4 live integration command passed:
  `cargo test -p repovec-core --test qdrant_liveness_integration -- --ignored`.
  It ran four ignored scenarios: correct API key, wrong API key, closed port,
  and bounded readiness timeout. No `repovec-qdrant-liveness-*` containers
  remained afterwards.
- [x] (2026-06-02T02:32:03+02:00) Milestone 4 gates passed:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test`. The test
  run executed 214 nextest tests with four live-Qdrant scenarios skipped by
  default, 13 `repovec-ci` doctests, and 22 `repovec-core` doctests.
- [x] (2026-06-02T03:12:21+02:00) CodeRabbit completed the Milestone 4
  review after one 21-minute rate-limit backoff and raised five findings. The
  fixes made the readiness wait use `tokio::time::sleep`, changed temporary
  API-key files to use `std::env::temp_dir()`, reported the actual readiness
  wait timeout, and made container/test setup helpers return fallible results
  to the test body.
- [x] (2026-06-02T03:17:02+02:00) Re-ran Milestone 4 validation after the
  CodeRabbit fixes. `make check-fmt`, `make typecheck`, `make lint`,
  `make test`, the opt-in live Qdrant integration command, and
  `make markdownlint` all passed. The default test run again executed 214
  nextest tests with four live-Qdrant scenarios skipped, 13 `repovec-ci`
  doctests, and 22 `repovec-core` doctests.
- [x] (2026-06-02T03:35:18+02:00) CodeRabbit follow-up review raised four
  remaining integration-test concerns. The accepted fixes removed the runtime
  environment guard in favour of unconditional ignored tests, kept default
  `make test` deterministic under the repository's `--all-features` test
  policy, and parsed the first non-empty `podman port` binding for dual-stack
  hosts.
- [x] (2026-06-02T03:39:04+02:00) Re-ran Milestone 4 validation after the
  follow-up fixes. `make check-fmt`, `make typecheck`, `make lint`,
  `make test`, and
  `cargo test -p repovec-core --test qdrant_liveness_integration -- --ignored`
  passed. The default test run executed 214 nextest tests with four live-Qdrant
  scenarios skipped, 13 `repovec-ci` doctests, and 22 `repovec-core` doctests.
- [x] (2026-06-02T04:29:08+02:00) CodeRabbit follow-up review was
  rate-limited twice, with mandated random backoffs of 24 minutes and 17
  minutes. The completed retry raised two valid findings: pin the Qdrant
  integration image and avoid oversleeping past the readiness deadline.
- [x] (2026-06-02T04:33:27+02:00) Re-ran Milestone 4 validation after pinning
  the integration image to `docker.io/qdrant/qdrant:v1.18.1` and bounding each
  poll sleep to the remaining deadline. `make check-fmt`, `make typecheck`,
  `make lint`, `make test`, and
  `cargo test -p repovec-core --test qdrant_liveness_integration -- --ignored`
  passed. The default test run executed 214 nextest tests with four live-Qdrant
  scenarios skipped, 13 `repovec-ci` doctests, and 22 `repovec-core` doctests.
- [x] (2026-06-02T04:45:00+02:00) CodeRabbit follow-up review raised three
  integration-test concerns. The accepted fixes changed the polling match arm
  to ignore retry errors explicitly, added visible best-effort cleanup
  diagnostics for temporary files and containers, and imported `Future`
  explicitly for the async helper bound.
- [x] (2026-06-02T04:51:00+02:00) Re-ran Milestone 4 validation after the
  cleanup-diagnostic fixes. `make check-fmt`, `make typecheck`, `make lint`,
  `make test`, and
  `cargo test -p repovec-core --test qdrant_liveness_integration -- --ignored`
  passed. The default test run executed 214 nextest tests with four live-Qdrant
  scenarios skipped, 13 `repovec-ci` doctests, and 22 `repovec-core` doctests.
  The live command ran four tests and left no `repovec-qdrant-liveness-*`
  containers behind.
- [x] (2026-06-02T05:05:00+02:00) CodeRabbit follow-up review raised one
  remaining trivial integration-test concern. The readiness retry loop now
  captures the current instant once after each liveness probe and reuses it for
  both deadline comparison and remaining-sleep calculation.
- [x] (2026-06-02T05:10:00+02:00) Re-ran Milestone 4 validation after the
  single-instant polling fix. `make check-fmt`, `make typecheck`, `make lint`,
  `make test`,
  `cargo test -p repovec-core --test qdrant_liveness_integration -- --ignored`,
  and `make markdownlint` passed. The default test run executed 214 nextest
  tests with four live-Qdrant scenarios skipped, 13 `repovec-ci` doctests, and
  22 `repovec-core` doctests. The live command ran four tests and left no
  `repovec-qdrant-liveness-*` containers behind.
- [x] (2026-06-02T05:25:00+02:00) CodeRabbit follow-up review raised one
  final trivial simplification. The readiness retry loop now relies on the
  explicit `now >= deadline` guard and sleeps for the bounded remaining
  duration without a redundant zero-duration check.
- [x] (2026-06-02T05:30:00+02:00) Re-ran Milestone 4 validation after the
  redundant-guard removal. `make check-fmt`, `make typecheck`, `make lint`,
  `make test`,
  `cargo test -p repovec-core --test qdrant_liveness_integration -- --ignored`,
  and `make markdownlint` passed. The default test run executed 214 nextest
  tests with four live-Qdrant scenarios skipped, 13 `repovec-ci` doctests, and
  22 `repovec-core` doctests. The live command ran four tests and left no
  `repovec-qdrant-liveness-*` containers behind.
- [x] (2026-06-02T05:55:00+02:00) CodeRabbit follow-up review was
  rate-limited once, with the mandated random backoff of 21 minutes. The
  completed retry raised one trivial readability concern: import
  `QdrantLivenessReport` in the integration test instead of using a fully
  qualified return type.
- [x] (2026-06-02T06:00:00+02:00) Re-ran Milestone 4 validation after the
  `QdrantLivenessReport` import simplification. `make check-fmt`,
  `make typecheck`, `make lint`, `make test`,
  `cargo test -p repovec-core --test qdrant_liveness_integration -- --ignored`,
  and `make markdownlint` passed. The default test run executed 214 nextest
  tests with four live-Qdrant scenarios skipped, 13 `repovec-ci` doctests, and
  22 `repovec-core` doctests. The live command ran four tests and left no
  `repovec-qdrant-liveness-*` containers behind.
- [x] (2026-06-02T06:18:00+02:00) CodeRabbit follow-up review completed
  with zero findings. Milestone 4 is ready to commit.
- [x] (2026-06-02T06:20:00+02:00) Committed Milestone 4 as `b31930d`
  (`Add Qdrant liveness integration test`).
- [x] (2026-06-02T06:27:00+02:00) Milestone 5 complete: wired both
  `repovecd` and `repovec-mcpd` to fail closed when the Qdrant liveness check
  fails, because both checked-in systemd units directly require and order after
  Qdrant. Startup tests use injected async health-check closures and do not
  open network sockets.
- [x] (2026-06-02T06:31:00+02:00) Early Milestone 5 typecheck passed after
  removing unused single-purpose systemd wrapper functions: `make typecheck`.
- [x] (2026-06-02T06:36:00+02:00) Milestone 5 gates passed:
  `make check-fmt`, `make typecheck`, `make lint`, `make test`,
  `make markdownlint`, and the opt-in live Qdrant integration command. The
  default test run executed 220 nextest tests with four live-Qdrant scenarios
  skipped, 13 `repovec-ci` doctests, and 22 `repovec-core` doctests. The live
  command ran four tests and left no `repovec-qdrant-liveness-*` containers
  behind.
- [x] (2026-06-02T06:44:00+02:00) CodeRabbit completed Milestone 5 review
  with zero findings.
- [x] (2026-06-02T06:45:00+02:00) Committed Milestone 5 as `d0c65b3`
  (`Validate Qdrant during daemon startup`).
- [x] (2026-06-02T06:50:00+02:00) Milestone 6 complete: updated the
  technical design, users guide, developers guide, contents index, and roadmap
  to describe authenticated Qdrant liveness, daemon startup failure behaviour,
  the opt-in live integration command, and completion of roadmap item `1.2.3`.
- [x] (2026-06-02T07:02:00+02:00) Milestone 6 gates passed except for
  `make fmt`, which still fails in the repository-wide `markdownlint --fix`
  phase on pre-existing documentation issues outside this feature. The
  applicable gates passed: `make check-fmt`, `make typecheck`, `make lint`,
  `make test`, `make markdownlint`, `make nixie`, and the opt-in live Qdrant
  integration command. The default test run executed 220 nextest tests with
  four live-Qdrant scenarios skipped, 13 `repovec-ci` doctests, and 22
  `repovec-core` doctests. The live command ran four tests and left no
  `repovec-qdrant-liveness-*` containers behind.
- [x] (2026-06-02T07:13:00+02:00) CodeRabbit completed Milestone 6 review
  with zero findings.

## Surprises & Discoveries

- The prompt includes completion criteria about interactive terminal sessions,
  resize propagation and exit codes. Those criteria do not match roadmap item
  `1.2.3`, the Qdrant roadmap text, or the surrounding Qdrant documentation.
  This plan treats them as contradictory and out of scope.
- Firecrawl's crates.io page extraction reported an old `qdrant-client`
  version, but `cargo search qdrant-client --limit 3` reported `1.18.0`. The
  plan uses Cargo's registry metadata as the dependency source of truth.
- The `qdrant-client` `1.18.0` crate contains a first-class
  `Qdrant::health_check().await` method backed by `/qdrant.Qdrant/HealthCheck`,
  so the implementation does not need to hand roll a generated gRPC client.
- The context-pack MCP server was discoverable, but this session exposed only
  list/get output operations and no create/update operation. No existing
  context packs were available for this branch, so the Wyvern agents received
  explicit read-only task prompts instead of a generated pack.
- `make fmt` failed in its `mdformat-all` phase because the local
  `markdownlint --fix` invocation reported existing documentation issues in
  files outside this plan. The resulting unrelated formatter churn was
  reverted, and the deterministic validation gates plus `make markdownlint`
  passed afterwards.
- `cargo info qdrant-client` on `2026-06-02` still reports `qdrant-client`
  `1.18.0`, so the implementation can use the planned client version without
  revising dependency constraints.
- CodeRabbit's Milestone 1 review found that the API-key byte predicate
  duplicated newline, carriage-return and tab checks already excluded by the
  printable-ASCII range. It also correctly identified that the printable-ASCII
  invariant deserves property coverage, so property tests are warranted for
  this small but security-relevant validation boundary.
- `cap-std` was already present in the repository through `repovec-ci`, but not
  centralized in `[workspace.dependencies]`. Milestone 2 moved that existing
  dependency into the workspace table and reused it from `repovec-core` so
  API-key file loading follows the repository's capability-oriented filesystem
  convention.
- CodeRabbit rate-limited the first two Milestone 2 review attempts. The
  mandated random backoffs were 25 minutes and 16 minutes. The third attempt
  completed normally with zero findings.
- CodeRabbit rate-limited the first Milestone 3 review attempt. The mandated
  random backoff was 16 minutes. The second attempt completed normally with
  zero findings.
- The live Qdrant test showed that `Qdrant::health_check().await` proves
  process readiness but not API-key validity: a wrong key still receives a
  health reply. `Qdrant::list_collections().await` is cheap, read-only and
  authenticated, so the implementation uses it to close the authentication gap.
- CodeRabbit's Milestone 4 review caught two test-portability issues: the
  temporary API-key helper was hard-coded to `/tmp`, and the readiness polling
  loop used blocking `std::thread::sleep` inside an async function. Both were
  valid findings and were fixed before proceeding.
- The repository's `make test` target runs with `--all-features`, so a Cargo
  feature cannot be used as the opt-in live-test gate. The live integration
  scenarios are therefore unconditionally `#[ignore]`d and run only through the
  documented `-- --ignored` command.
- Pinning the live integration image matters even when the packaged Quadlet
  tracks Qdrant's `v1` stream: the test is a behavioural contract for this
  implementation branch, not the appliance update policy.
- The repository denies direct stderr-printing macros in tests through Clippy.
  Cleanup diagnostics therefore use a small `stderr().write_fmt(...)` helper so
  Drop failures are visible without weakening lint policy.
- CodeRabbit caught a tiny timing consistency issue in the readiness polling
  loop after the deadline-sleep fix. Capturing the instant once per completed
  probe makes the comparison and remaining-duration calculation use the same
  observation of time.
- Adding runtime liveness to the daemon binaries made the previous
  no-argument systemd validation wrapper functions dead code. The combined
  startup helper is now the canonical no-argument entry point, while the
  individual systemd helper remains injectable for unit tests.
- `make fmt` still cannot be used as a clean acceptance gate in this worktree:
  its `markdownlint --fix` phase reports unrelated pre-existing line-length
  and missing-reference issues in documentation files outside this feature.
  The accidental formatter churn from running it was restored, while
  `make markdownlint` and `make nixie` pass for the final tree.

## Decision Log

- Decision: keep runtime Qdrant liveness in a new appliance module rather than
  extending `qdrant_quadlet`. Rationale: `qdrant_quadlet` is intentionally a
  pure static validator over checked-in Quadlet text; mixing live gRPC I/O into
  it would blur the repository's current boundary.

- Decision: prefer the official `qdrant-client` crate for the gRPC health
  call. Rationale: the crate already exposes `Qdrant::health_check().await`,
  handles the `api-key` metadata header, and targets the same `127.0.0.1:6334`
  gRPC endpoint required by the roadmap. This is lower risk than generating and
  maintaining local protobuf bindings.

- Decision: make live Qdrant integration coverage opt-in while keeping unit
  and behavioural coverage in the default test set. Rationale: the roadmap
  requires an integration test that starts Qdrant, but ordinary CI and
  developer environments may not provide Podman. The test should exist and be
  runnable on suitable hosts without making every default `make test`
  invocation depend on a container runtime.

- Decision: do not plan Kani or Verus proof work for this roadmap item.
  Rationale: the change is protocol and I/O orchestration. It does not
  introduce a safety-critical invariant, unbounded pure logic, unsafe code, or
  state-machine property that would justify bounded model checking or deductive
  proof.

- Decision: defer marking roadmap item `1.2.3` done until implementation is
  complete. Rationale: this branch initially carries only the
  pre-implementation plan. Marking the item done before code, tests, docs and
  review land would make the roadmap inaccurate.

- Decision: model the API key as a redacted `QdrantApiKey` newtype rather than
  passing a raw `String` through the public liveness API. Rationale: the
  liveness adapter must eventually expose the secret to `qdrant-client`, but
  callers, debug output, and errors should not accidentally print it.

- Decision: use `cap_std::fs_utf8::Dir` for the API-key file adapter rather
  than `std::fs::read_to_string`. Rationale: `AGENTS.md` directs Rust code in
  this repository toward capability-oriented filesystem APIs, and the
  dependency was already in use by `repovec-ci`.

- Decision: keep gRPC error mapping dependent only on `qdrant-client`'s public
  API. Rationale: `qdrant-client` exposes status information through
  `QdrantError`; adding a direct `tonic` dependency solely for enum matching
  would widen this milestone's dependency surface without improving the runtime
  contract.

- Decision: use a dynamic loopback host port for the live integration test
  rather than fixed `127.0.0.1:6334`. Rationale: the production default remains
  `127.0.0.1:6334`, but tests should not fail merely because a developer has a
  local Qdrant already bound to the appliance port.

- Decision: define liveness as health-readiness plus a read-only authenticated
  collection-list request. Rationale: Qdrant's gRPC health endpoint is not
  sufficient to validate the stored API key, while `list_collections()` fails
  for wrong credentials without mutating Qdrant state.

- Decision: pin the live integration test image to
  `docker.io/qdrant/qdrant:v1.18.1`. Rationale: the test should be reproducible
  while the checked-in Quadlet can continue to express the appliance's registry
  update policy separately.

- Decision: emit best-effort cleanup warnings from live-test Drop handlers.
  Rationale: temporary-file and container cleanup cannot fail the already
  unwinding test safely, but hidden cleanup failures can leave confusing local
  state. The warning path redacts secrets and avoids Clippy-denied printing
  macros.

- Decision: make both `repovecd` and `repovec-mcpd` validate Qdrant liveness
  at startup. Rationale: the checked-in service units for both daemons directly
  require Qdrant and start after it, so both process boundaries should fail
  closed instead of relying on only one daemon to enforce the runtime contract.

## Outcomes & Retrospective

Roadmap item `1.2.3` now has a documented Rust liveness check that proves
Qdrant is reachable over authenticated local gRPC and prevents dependent
services from silently starting against an unavailable vector store.

The final public API lives in
`repovec_core::appliance::qdrant_liveness`:

- `QdrantLivenessConfig`
- `QdrantApiKey`
- `QdrantLivenessReport`
- `QdrantLivenessError`
- `load_qdrant_api_key(...)`
- `check_qdrant_liveness(...)`

The implementation uses `qdrant-client` `1.18.0` with default features
disabled, `tokio` `1.48.0` with `rt-multi-thread` and `time`, and
`cap-std`/`camino` for capability-oriented API-key file access. `repovec-core`
owns the async liveness startup policy and runtime, while daemon binaries
delegate to `daemon_startup`.

The live integration test uses a dynamic loopback host port instead of fixed
`127.0.0.1:6334`, while the production default remains
`http://127.0.0.1:6334`. The test pins the Qdrant image to
`docker.io/qdrant/qdrant:v1.18.1` for reproducibility.

The most important implementation discovery was that Qdrant's gRPC
`health_check()` can succeed with the wrong API key. The final liveness policy
therefore requires both `health_check()` and authenticated
`list_collections()` before reporting success.

Documentation now describes the liveness policy and operator diagnostics in
`docs/repovec-appliance-technical-design.md`, `docs/users-guide.md`, and
`docs/developers-guide.md`. `docs/contents.md` links this ExecPlan as a
delivery record, and `docs/roadmap.md` marks item `1.2.3` complete.

All deterministic gates passed for the final tree except `make fmt`, which
still fails because its repository-wide `markdownlint --fix` phase reports
pre-existing documentation issues outside this feature. The clean final gates
were `make check-fmt`, `make typecheck`, `make lint`, `make test`,
`make markdownlint`, `make nixie`, and the opt-in live Qdrant integration
command. CodeRabbit completed the final milestone review with zero findings.
