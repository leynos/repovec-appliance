# Developers guide

This guide is for maintainers and contributors working on repovec-appliance. It
describes the repository-level build, test, lint, and continuous integration
(CI) workflow that must remain true as the project evolves.

## Spelling policy

Run `make spelling` to enforce en-GB-oxendict prose spelling. The generated
`typos.toml` starts from the shared Oxford dictionary and applies the narrow
repository policy in `typos.local.toml`. Edit the local policy, then run
`make spelling-config` rather than changing generated entries by hand. The
focused shared config builder refreshes its untracked dictionary cache only
when the authoritative copy is newer. The consumer checker enforces exact
phrase corrections that the token-based Typos scanner cannot represent.

## Normative references

- [Documentation contents](contents.md) if present
- [repovec-appliance technical design](repovec-appliance-technical-design.md)
- [Roadmap](roadmap.md)
- [Execution plan for 1.1.3: CI gating pipeline](execplans/1-1-3-ci-gating-pipeline.md)

## 1. Local quality gates

Before proposing a code change, run the repository gate targets:

```sh
set -o pipefail
make build 2>&1 | tee /tmp/repovec-make-build.log
set -o pipefail
make check-fmt 2>&1 | tee /tmp/repovec-make-check-fmt.log
set -o pipefail
make lint 2>&1 | tee /tmp/repovec-make-lint.log
set -o pipefail
make test 2>&1 | tee /tmp/repovec-make-test.log
set -o pipefail
make spelling 2>&1 | tee /tmp/repovec-make-spelling.log
```

When a change touches Markdown or documentation-tooling configuration, also run:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/repovec-make-fmt.log
set -o pipefail
make markdownlint 2>&1 | tee /tmp/repovec-make-markdownlint.log
set -o pipefail
make nixie 2>&1 | tee /tmp/repovec-make-nixie.log
```

These Make targets are the source of truth for local validation and for CI. Do
not duplicate or partially reimplement them in workflow YAML.

The provisioning helper integration suite has its own opt-in targets that are
deliberately kept out of `make test`:

- `make integration-command-test` runs the fast command-contract suite that
  uses `cmd-mox` shims; it needs only the Python harness dependencies.
- `make integration-test` runs the full lifecycle suite inside a privileged
  Fedora container managed by `testcontainers-python`; it needs a
  Docker-compatible runtime and the ability to launch privileged nested
  rootful Podman.

Both targets gate on phony prerequisite helpers (`_check-python`,
`_check-integration-prereqs`, `_check-command-test-prereqs`) that exit
non-zero when their checks fail, so missing prerequisites abort the chain
with an actionable skip message rather than letting `pytest` produce a
second misleading error on top. See [Section 6](#6-provisioning-integration-tests)
for the full prerequisite and execution contract.

## 2. GitHub Actions gate set

The repository CI workflow exposes seven stable, required job names:

- `spelling`
- `build`
- `check-fmt`
- `lint`
- `test`
- `docs-gate`
- `systemd-gate`

The first five jobs run on pull request updates, pushes to `main`, and manual
workflow dispatch.

`docs-gate` always reports a result so it can be configured as a required
check. It runs `make markdownlint` and `make nixie` only when the changed-file
set includes a documentation input. Markdown inputs use one of these extensions:

- `.md`
- `.markdown`
- `.mdx`

Documentation-tooling configuration changes also count as documentation input:

- `.markdownlint-cli2.jsonc`

When the changed-file list is unavailable, the workflow runs the documentation
gate conservatively instead of risking a skipped validation. That fallback
requires both `make markdownlint` and `make nixie`.

`make nixie` is narrower than `make markdownlint`: Mermaid validation runs only
when one of the changed Markdown files contains a Mermaid diagram, or when a
documentation-tooling configuration change or missing changed-file input
requires the conservative path. The user-visible flow is documented in
[users-guide.md](users-guide.md).

`systemd-gate` always reports a result so it can be configured as a required
check. It runs `make validate-systemd`, which builds the `repovec-ci` helper
and calls `repovec-ci systemd-gate` to verify that the checked-in systemd unit
files still satisfy the appliance service-layout contract. A non-zero exit from
the gate is a fatal CI failure.

Run `make validate-systemd` locally before proposing a change to any file under
`packaging/systemd/` or `crates/repovec-core/src/appliance/systemd_units/`.

### 2.1 Workflow pins and Dependabot

Dependabot owns the upgrade of GitHub Actions and reusable workflows,
including calls into `leynos/shared-actions`. Contract tests that assert a
caller's exact commit SHA create a lockstep dependency: every time Dependabot
opens a bump PR, the test fails until a human edits the pinned constant to
match. That defeats the purpose of automated dependency updates and turns a
routine bump into a manual chore.

Contract tests may still verify the *shape* of a reusable-workflow caller.
They must not verify the specific SHA value.

- Do assert the workflow references the correct reusable workflow path.
- Do assert the ref is pinned to a full 40-character commit SHA, not a
  mutable branch such as `main` or `rolling`.
- Do assert the expected `on:` triggers, least-privilege `permissions:`, and
  the inputs the caller relies on.
- Do not hard-code the current SHA value as an expected string. Match it with
  a pattern instead.
- Do not fail a test purely because Dependabot bumped the pinned SHA.

```python
import re

SHA_RE = re.compile(r"^[0-9a-f]{40}$")


def test_uses_pinned_full_sha(caller_step):
    ref = caller_step["uses"].split("@")[-1]
    assert SHA_RE.match(ref), f"expected a 40-hex commit SHA, got {ref!r}"
```

If a workflow's behaviour genuinely depends on a feature only present from a
particular commit onwards, express that as a comment or a changelog note, not
as a test assertion on the SHA string.

## 3. CI policy helper

### 3.1 Public API

```rust
/// Reason the documentation gate should or should not run.
pub enum DocsGateReason {
    DocumentationChanged,
    MissingChangedFiles,
    NoDocumentationChanges,
}
impl DocsGateReason { pub const fn as_str(self) -> &'static str }

/// Computed plan for the documentation gate.
pub struct DocsGatePlan { /* opaque */ }
impl DocsGatePlan {
    pub const fn should_run(&self) -> bool;
    pub const fn docs_gate_required(&self) -> bool;
    pub const fn nixie_required(&self) -> bool;
    pub const fn reason(&self) -> DocsGateReason;
    pub fn matched_files(&self) -> &[String];
    pub fn conservative_fallback_files(&self) -> &[String];
}

/// Whether a Markdown file contains a Mermaid diagram (or could not be read).
pub enum MermaidDetection { Present, Absent, Unknown }

/// Evaluates the docs-gate policy using the real file system.
pub fn evaluate_docs_gate_in<I, S>(
    root: &cap_std::fs_utf8::Dir,
    changed_files: I,
) -> DocsGatePlan
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>;

/// Evaluates the docs-gate policy with an injected Mermaid detector.
/// Use this in tests to avoid real file I/O.
pub fn evaluate_docs_gate_with<I, S, F>(
    changed_files: I,
    path_contains_mermaid: F,
) -> DocsGatePlan
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
    F: FnMut(&str) -> MermaidDetection;
```

`DocsGateReason` records why the documentation gate should run or be skipped.
`DocsGatePlan` is the computed result that the workflow consumes.
`MermaidDetection` captures whether a Markdown file contains Mermaid content or
falls back to the conservative unreadable-file path. `evaluate_docs_gate_in`
evaluates the policy with the real file system, and `evaluate_docs_gate_with`
evaluates the same policy with an injected detector.

`evaluate_docs_gate_in` uses `cap_std::fs_utf8::Dir` to read file contents.
`evaluate_docs_gate_with` accepts an injected closure, making the policy fully
testable without any file I/O.

### 3.2 `cap-std` dependency rationale

`cap-std` (with the `fs_utf8` feature) is used instead of `std::fs` because it
provides capability-safe, UTF-8-native file-system access; this makes the
file-reading boundary explicit and keeps the core policy logic
(`evaluate_docs_gate_with`) free of ambient authority, making it
straightforward to test with a stub closure.

### 3.3 `repovec-ci` binary

```text
USAGE:
    repovec-ci [docs-gate] [--changed-file <path> [--changed-file <path>]...] [--help]
    repovec-ci [docs-gate] --stdin
    repovec-ci systemd-gate
```

| Subcommand / Flag       | Description                                                                                                         |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------- |
| *(no subcommand)*       | Equivalent to `docs-gate`; retained for backwards compatibility.                                                    |
| `docs-gate`             | Evaluate the documentation-gate policy and print `key=value` lines for `$GITHUB_OUTPUT`.                            |
| `systemd-gate`          | Validate the checked-in systemd unit files against the appliance contract; exit non-zero on any contract violation. |
| `--changed-file <path>` | (docs-gate only) Treat `<path>` as a changed file; repeatable; mutually exclusive with `--stdin`.                   |
| `--stdin`               | (docs-gate only) Read newline-delimited changed-file paths from stdin; mutually exclusive with `--changed-file`.    |
| `-h`, `--help`          | Print usage text and exit.                                                                                          |

In `docs-gate` mode the binary writes `key=value` lines to stdout for use with
`$GITHUB_OUTPUT` and is invoked by the `docs-gate` CI job. The output keys are:

- `should_run`
- `docs_gate_required`
- `nixie_required`
- `reason`
- `matched_files_count`
- `matched_files`
- `conservative_fallback_files_count`
- `conservative_fallback_files`

`systemd-gate` writes a single confirmation line to stdout on success:

```text
checked-in systemd units satisfy the appliance contract
```

On failure it writes a human-readable error to stderr and exits with status 1.

## 4. Required-check enforcement

The desired repository ruleset is versioned in
[`/.github/rulesets/main-ci-gating.json`](../.github/rulesets/main-ci-gating.json).
 Apply that payload only after the workflow changes that produce the required
checks are available on the default branch.

Example application command:

```sh
gh api \
  --method POST \
  repos/leynos/repovec-appliance/rulesets \
  --input .github/rulesets/main-ci-gating.json
```

If a ruleset named `main-ci-gating` already exists, update it instead:

```sh
RULESET_ID="$(
  gh api repos/leynos/repovec-appliance/rulesets \
    --jq '.[] | select(.name == "main-ci-gating") | .id'
)"
gh api \
  --method PUT \
  "repos/leynos/repovec-appliance/rulesets/${RULESET_ID}" \
  --input .github/rulesets/main-ci-gating.json
```

Keep the workflow job names and the ruleset payload synchronized. Changing a
required job name without updating the ruleset will block merges.

## 5. Appliance module

### 5.1 Appliance module overview

`crates/repovec-core/src/appliance/` contains appliance-specific validation
helpers. Keep these helpers close to the assets they validate and avoid placing
appliance policy in unrelated core modules.

Each appliance asset gets its own submodule under `appliance/`. The submodule
owns the checked-in asset contract, any local parsing needed for that asset,
the typed validation error, and focused tests for the contract.

### 5.2 `qdrant_quadlet` validation surface

The `qdrant_quadlet` module exposes the public validation surface for the
checked-in Qdrant Podman Quadlet:

- `checked_in_qdrant_quadlet() -> &'static str` returns the repository's
  embedded Quadlet source.
- `validate_checked_in_qdrant_quadlet(...)` validates the embedded Quadlet
  asset.
- `validate_qdrant_quadlet(...)` validates caller-provided Quadlet contents
  against the same appliance contract.
- `QdrantQuadletObserver` is the telemetry boundary for validation events. Pass
  `TracingQdrantQuadletObserver` when events should be emitted through
  `tracing`.
- `QdrantQuadletError` is the typed error enum for validation failures. See
  `crates/repovec-core/src/appliance/qdrant_quadlet/error.rs` for the full
  variant list.

Keep the Qdrant contract split along the same boundary as the module Rustdoc:

- domain invariants belong in `qdrant_quadlet/mod.rs`;
- API-key contract checks belong in `qdrant_quadlet/api_key.rs`;
- appliance host bindings belong in `qdrant_quadlet/platform_bindings/`.

The domain invariants describe what Qdrant itself requires: the supported image
reference, the in-container storage target, and the REST and gRPC container
ports. The platform binding adapter describes how the appliance satisfies that
contract on the host: loopback `PublishPort=` values, the storage source path,
the SELinux relabel option, and the Podman auto-update policy. Do not add host
paths, SELinux rules, or other appliance infrastructure constants directly to
the domain validation body in `mod.rs`; route them through the adapter and keep
the module Rustdoc synchronized with the boundary.

The validation entry points make telemetry dependencies explicit:

```rust
pub fn validate_checked_in_qdrant_quadlet(
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError>;

pub fn validate_qdrant_quadlet(
    contents: &str,
    observer: &dyn QdrantQuadletObserver,
) -> Result<(), QdrantQuadletError>;
```

The validator also enforces the Qdrant API-key authentication contract. A valid
Quadlet must require and start after `repovec-qdrant-api-key.service`, expose
the Podman secret `repovec-qdrant-api-key` as `QDRANT__SERVICE__API_KEY`, and
reject any inline `Environment=QDRANT__SERVICE__API_KEY=...` value. Keep this
validation pure: it parses strings and reports policy errors, but it must not
call Podman, systemd, or the filesystem.

The provisioning assets live beside other appliance packaging inputs:

- `packaging/systemd/repovec-qdrant-api-key.service`
- `packaging/libexec/repovec-qdrant-api-key`
- `packaging/sysusers.d/repovec.conf`

The helper is host-facing packaging code. It owns filesystem, user, permission
and Podman-secret operations; `repovec-core` only validates the static contract
that those assets expose.


### 5.3 `qdrant_liveness` runtime surface

The `qdrant_liveness` module exposes the public runtime validation surface for
the local Qdrant service:

- `QdrantLivenessConfig` carries the gRPC endpoint, API-key file path, and
  timeout. The appliance defaults are `http://127.0.0.1:6334`,
  `/etc/repovec/qdrant-api-key`, and a short bounded probe timeout.
- `load_qdrant_api_key(...) -> Result<QdrantApiKey, QdrantLivenessError>`
  reads and validates raw key material without logging or returning the secret
  value.
- `check_qdrant_liveness(config).await -> Result<QdrantLivenessReport, QdrantLivenessError>`
  connects to Qdrant over gRPC with the stored key and returns non-secret
  server metadata when Qdrant is ready.
- `QdrantLivenessError` is the typed error enum for missing key files,
  unreadable key files, invalid key material, invalid endpoints, connection
  failures, timeouts, authentication failures, and non-ready replies.

Keep this module split from `qdrant_quadlet`. `qdrant_quadlet` validates static
Quadlet text; `qdrant_liveness` performs runtime filesystem and network I/O.
The liveness policy currently requires both `health_check()` and an
authenticated read-only `list_collections()` request, because the health
endpoint alone does not prove API-key validity.

Daemon binaries call `check_qdrant_liveness()` at startup through injectable
helpers. Unit tests for those binaries must inject async success and failure
closures rather than opening sockets. This keeps daemon tests focused on
startup orchestration, exit-code mapping, and structured logging while the
live Qdrant integration test proves the real network and authentication
contract in `repovec-core`. The live Qdrant integration test is
ignored by default and can be run explicitly on a host with Podman:

```sh
cargo test -p repovec-core --test qdrant_liveness_integration -- --ignored
```


### 5.4 `systemd_units` validation surface

The `systemd_units` module exposes the public validation surface for the
checked-in repovec target, daemon service files, and grepai indexer template:

- `checked_in_repovec_target() -> &'static str` returns the embedded
  `packaging/systemd/repovec.target` source.
- `checked_in_repovecd_service() -> &'static str` returns the embedded
  `packaging/systemd/repovecd.service` source.
- `checked_in_repovec_mcpd_service() -> &'static str` returns the embedded
  `packaging/systemd/repovec-mcpd.service` source.
- `checked_in_repovec_grepai_template() -> &'static str` returns the embedded
  `packaging/systemd/repovec-grepai@.service` source.
- `CHECKED_IN_REPOVEC_GREPAI_TEMPLATE_PATH` names the repository path for the
  checked-in `packaging/systemd/repovec-grepai@.service` template.
- `validate_checked_in_systemd_units() -> Result<(), SystemdUnitError>`
  validates the embedded unit set.
- `validate_systemd_units(target, repovecd, mcpd) -> Result<(), SystemdUnitError>`
  validates caller-provided target and daemon unit contents against the
  pre-indexer appliance contract.
- `validate_systemd_units_with_grepai_template(target, repovecd, mcpd, grepai)`
  validates caller-provided target, daemon, and grepai template contents
  against the full service-layout contract.
- `validate_and_trace_checked_in_units() -> Result<(), SystemdUnitError>`
  verifies the embedded unit set and emits a `tracing::trace!` event on
  success. It serves as the entry point daemon binaries call during startup.
- `run_startup_validation(validator) -> Result<(), i32>` runs an injected
  systemd-unit validator at a daemon startup boundary, emits structured
  `tracing::error!` diagnostics with `unit` and `error` fields on failure,
  emits a `tracing::debug!` confirmation on success, and maps validation
  failures to process exit code `1`.
- `SystemdUnitError` is the typed error enum for validation failures. See
  `crates/repovec-core/src/appliance/systemd_units/error.rs` for the full
  variant list.
- `SystemdUnitError::unit() -> &str` returns the logical systemd unit name
  associated with the validation failure. Use this field when emitting
  structured log events, so operators can identify the failing unit without
  parsing the display string.

This module validates static unit-file policy only. It must not call
`systemctl`, start processes, read `/etc/systemd/system`, or otherwise perform
runtime installation work.

Keep the three-argument `validate_systemd_units` API for callers that only need
the base target and daemon contract. New callers that need the complete
checked-in appliance layout should use
`validate_systemd_units_with_grepai_template`.

Daemon binaries call the systemd startup adapter near the start of `main()`,
before Qdrant liveness validation or other long-running work. Treat
`SystemdUnitError` as fatal at that process boundary: log `error.unit()` and
`error` as structured fields and exit non-zero, so systemd reports a failed
startup rather than running under a broken checked-in unit contract.

#### Observability

`repovec-core` uses `tracing 0.1` as its logging facade. Library crates depend
only on the facade crate. Binary crates and test harnesses choose and configure
the subscriber, such as `tracing-subscriber`, at the application boundary.

Qdrant Quadlet validation reports telemetry through `QdrantQuadletObserver`.
The validation pipeline receives the observer explicitly, making logging
side effects visible at call sites. `TracingQdrantQuadletObserver` is the
production adapter that maps observer callbacks to `tracing` events.
The unit type `()` implements `QdrantQuadletObserver` as a no-op sink for test
and silent paths that should validate without telemetry overhead.

Qdrant Quadlet observer adapters follow these instrumentation conventions:

- Define a module `LOG_TARGET` constant, such as
  `repovec_core::qdrant_quadlet`, and pass it as the `target:` argument from
  the tracing observer. This keeps per-module filtering predictable with
  `RUST_LOG`.
- Emit `info!` from observer callbacks at validation entry and success
  boundaries.
- Emit `warn!` from observer callbacks for each contract-violation return
  point, with at least one structured field carrying the offending value, the
  expected value, or both.
- Emit `error!` from observer callbacks for malformed input parse failures,
  including `line_number` and redacted line-content fields.
- Redact sensitive values, including API key literals, before placing them in
  any log field.

When adding instrumentation:

- Define or reuse the module's `LOG_TARGET` constant in the tracing observer.
- Add a `QdrantQuadletObserver` callback for the validation boundary, and call
  it from the validator before returning.
- Emit at the operator-facing level in the tracing observer; do not use
  `debug!` or `trace!` for contract violations operators must act on.
- Carry structured fields instead of embedding variable values in the message
  string.
- Redact secrets before logging.
- Add a test that fails if the observer callback or tracing adapter event is
  removed.

#### Mutual exclusion in the provisioning helper

##### `LOCK_FILE`

The helper uses `/etc/repovec/repovec-qdrant-api-key.lock` as its lock file.
It opens the file on descriptor 9 with `exec 9>"${LOCK_FILE}"` and acquires an
exclusive lock via `flock 9`.  The lock file is placed in `/etc/repovec`
rather than `/var/lock` because `/etc/repovec` is a root-owned directory with
mode `0750` that is not world-writable.  This prevents unprivileged users from
substituting the lock file in a way that could interfere with the mutual
exclusion protocol.

##### Critical section boundaries

The lock is acquired after the initial `install -d -o root -g root` bootstrap
step.  That bootstrap runs unlocked so the lock file's parent directory already
exists when the helper attempts to open it.  Once held, the lock covers
`ensure_repovec_user`, the group-ownership `install -d`, secret inspection,
secret removal, key generation, and secret creation.  The lock is released via
the explicit `release_lock` helper, which calls `flock -u 9` and emits a debug
log.  Signal handlers on `EXIT`, `HUP`, `INT`, and `TERM` also call
`release_lock` before cleaning up temporary files, ensuring the lock is not
held by a signal-interrupted process for longer than necessary.

##### Fail-closed invariant

Unexpected failures from `podman secret rm` (anything other than "in use")
cause the helper to exit non-zero.  This fail-closed behaviour ensures the
caller observes a failure rather than silently proceeding with the existing
secret, which might be stale or compromised.  The "in use" case exits zero
because the existing secret remains valid and usable.

##### Debug logging

Setting `REPOVEC_DEBUG=1` causes the helper to emit debug-level log lines to
stderr via the internal `debug_log()` function.  When enabled, the helper logs
when it is waiting to acquire the lock, when it has acquired the lock, and when
it has released the lock.  `REPOVEC_DEBUG` is intended for local
troubleshooting only and must not be set in production service units.

The `debug_log()` function is internal to the provisioning helper.  It writes a
line to stderr when `REPOVEC_DEBUG=1` is set; under the systemd service, the
journal timestamps that stderr line.  It is not a public API, and callers
outside the provisioning helper must not rely on its output format.  Every lock
lifecycle event (waiting to acquire, acquired, and released) is instrumented
through this function, and its output is suppressed when `REPOVEC_DEBUG` is
unset or set to any value other than `1`.

### 5.5 Daemon startup test helpers

The `repovec-test-helpers` crate owns the shared daemon startup test harness
used by `repovec-core`, `repovecd`, and `repovec-mcpd`. It exposes a generic
`capture_logs(action)` helper that captures formatted `tracing` output in
memory for the duration of an injected closure, plus the `ensure` and
`ensure_log_line_contains` assertion primitives. On top of these it provides
the binary-facing `assert_startup_*` wrappers that the daemon crates call.

Use the `assert_startup_*` wrappers for binary-level daemon startup tests
whenever the behaviour is the same across daemons and only the unit name
differs. Keep unit tests for `run_startup_validation()` itself in
`repovec-core`; those tests reuse `capture_logs` (via a dev-dependency on
`repovec-test-helpers`) and should assert the core adapter emits the expected
`TRACE`, `DEBUG`, and `ERROR` events so the logging contract cannot disappear
while return-code tests still pass.

Snapshot helpers in `repovec-test-helpers` are behind its `snapshots` feature
because `insta` is only needed by daemon test targets. Daemon crates enable
that feature in `[dev-dependencies]` and commit the generated snapshots under
`crates/repovec-test-helpers/src/snapshots/`.

### 5.6 Extension pattern

To add validation for a new appliance asset:

1. Create a submodule directory under `appliance/` with `mod.rs`, `error.rs`,
   `parser.rs` if a custom parser is needed, and `tests.rs` when tests would
   otherwise make the module too large.
2. Re-export the submodule from `appliance/mod.rs`.
3. Embed the checked-in asset with `include_str!` and expose a
   `checked_in_*()` function.
4. Write `validate_*()` returning a typed error enum that implements
   `std::error::Error` and `fmt::Display`.
5. Cover all error variants in `tests.rs` using `rstest` fixtures and add BDD
   scenarios under `crates/repovec-core/tests/features/`.

### 5.7 Test patterns

The appliance validation modules use `rstest` for unit tests and `rstest-bdd`
with `rstest-bdd-macros` for behavioural tests. Unit tests should exercise each
typed error variant directly. Behavioural tests should describe the appliance
contract in feature files and keep the executable scenarios thin.

**Display and message snapshots (`insta`).** Operator-visible `fmt::Display`
strings for typed appliance errors (for example every [`QdrantQuadletError`][])
should be derived from **`validate_*` failures**, not from errors constructed
solely in tests. Mutate an embed or copy of the checked-in asset (or compose a
deliberate invalid parse input), invoke the real validator, assert the canonical
`PartialEq/Eq` typed error variant, then compare `error.to_string()` against a
committed YAML snapshot from the [`insta`][] crate (workspace-pinned under
`[workspace.dependencies]` and pulled in via `[dev-dependencies]`). Direct
literal assertions for `error.to_string()` are acceptable when committing one
snapshot file per diagnostic would exceed the active ExecPlan's file-count
tolerance. Prefer one `#[rstest]` harness with cases that enumerate
scenario-specific mutations alongside their stable snapshot labels, colocated
under the module `snapshots/` directory (see
`crates/repovec-core/src/appliance/qdrant_quadlet/`). Duplicate labels across
distinct cases remain valid whenever the reachable diagnostic matches the same
operator-facing wording (for instance two malformed `PublishPort=` inputs that
both surface `MissingGrpcPort`). Update snapshots deliberately via
`cargo insta` (or `INSTA_UPDATE=…`) when message wording changes.

[`QdrantQuadletError`]: ../crates/repovec-core/src/appliance/qdrant_quadlet/error.rs
[`insta`]: https://docs.rs/insta

Property-based tests use `proptest` (workspace dev-dependency). `proptest` is
appropriate for invariants that must hold across arbitrary inputs, as a
complement to example-based `rstest` unit tests.  When writing property tests,
`prop_assume!` filters must not be used to exclude cases that the domain code
under test must handle — filters are reserved for excluding inputs that are
structurally invalid for the strategy, not for narrowing the test's coverage of
the invariant.  See
`crates/repovec-core/src/appliance/qdrant_quadlet/tests_proptest.rs` for a
worked example.

See [rstest BDD users guide](rstest-bdd-users-guide.md) and
[Rust testing with rstest fixtures](rust-testing-with-rstest-fixtures.md) for
the project-local testing guidance.

### 5.6 Extension pattern

To add validation for a new appliance asset:

1. Create a submodule directory under `appliance/` with `mod.rs`, `error.rs`,
   `parser.rs` if a custom parser is needed, and `tests.rs` when tests would
   otherwise make the module too large.
2. Re-export the submodule from `appliance/mod.rs`.
3. Embed the checked-in asset with `include_str!` and expose a
   `checked_in_*()` function.
4. Write `validate_*()` returning a typed error enum that implements
   `std::error::Error` and `fmt::Display`.
5. Cover all error variants in `tests.rs` using `rstest` fixtures and add BDD
   scenarios under `crates/repovec-core/tests/features/`.

### 5.7 Test patterns

The appliance validation modules use `rstest` for unit tests and `rstest-bdd`
with `rstest-bdd-macros` for behavioural tests. Unit tests should exercise each
typed error variant directly. Behavioural tests should describe the appliance
contract in feature files and keep the executable scenarios thin.

**Display and message snapshots (`insta`).** Operator-visible `fmt::Display`
strings for typed appliance errors (for example every [`QdrantQuadletError`][])
should be derived from **`validate_*` failures**, not from errors constructed
solely in tests. Mutate an embed or copy of the checked-in asset (or compose a
deliberate invalid parse input), invoke the real validator, assert the canonical
`PartialEq/Eq` typed error variant, then compare `error.to_string()` against a
committed YAML snapshot from the [`insta`][] crate (workspace-pinned under
`[workspace.dependencies]` and pulled in via `[dev-dependencies]`). Direct
literal assertions for `error.to_string()` are acceptable when committing one
snapshot file per diagnostic would exceed the active ExecPlan's file-count
tolerance. Prefer one `#[rstest]` harness with cases that enumerate
scenario-specific mutations alongside their stable snapshot labels, colocated
under the module `snapshots/` directory (see
`crates/repovec-core/src/appliance/qdrant_quadlet/`). Duplicate labels across
distinct cases remain valid whenever the reachable diagnostic matches the same
operator-facing wording (for instance two malformed `PublishPort=` inputs that
both surface `MissingGrpcPort`). Update snapshots deliberately via
`cargo insta` (or `INSTA_UPDATE=…`) when message wording changes.

[`QdrantQuadletError`]: ../crates/repovec-core/src/appliance/qdrant_quadlet/error.rs
[`insta`]: https://docs.rs/insta

Property-based tests use `proptest` (workspace dev-dependency). `proptest` is
appropriate for invariants that must hold across arbitrary inputs, as a
complement to example-based `rstest` unit tests.  When writing property tests,
`prop_assume!` filters must not be used to exclude cases that the domain code
under test must handle — filters are reserved for excluding inputs that are
structurally invalid for the strategy, not for narrowing the test's coverage of
the invariant.  See
`crates/repovec-core/src/appliance/qdrant_quadlet/tests_proptest.rs` for a
worked example.

See [rstest BDD users guide](rstest-bdd-users-guide.md) and
[Rust testing with rstest fixtures](rust-testing-with-rstest-fixtures.md) for
the project-local testing guidance.

### 5.5 Daemon startup test helpers

The `repovec-test-helpers` crate owns the shared daemon startup test harness
used by `repovec-core`, `repovecd`, and `repovec-mcpd`. It exposes a generic
`capture_logs(action)` helper that captures formatted `tracing` output in
memory for the duration of an injected closure, plus the `ensure` and
`ensure_log_line_contains` assertion primitives. On top of these it provides
the binary-facing `assert_startup_*` wrappers that the daemon crates call.

Use the `assert_startup_*` wrappers for binary-level daemon startup tests
whenever the behaviour is the same across daemons and only the unit name
differs. Keep unit tests for `run_startup_validation()` itself in
`repovec-core`; those tests reuse `capture_logs` (via a dev-dependency on
`repovec-test-helpers`) and should assert the core adapter emits the expected
`TRACE`, `DEBUG`, and `ERROR` events so the logging contract cannot disappear
while return-code tests still pass.

Snapshot helpers in `repovec-test-helpers` are behind its `snapshots` feature
because `insta` is only needed by daemon test targets. Daemon crates enable
that feature in `[dev-dependencies]` and commit the generated snapshots under
`crates/repovec-test-helpers/src/snapshots/`.

## 6. Provisioning integration tests

The Rust workspace's unit and behavioural tests cover the helper's contract on
paper. End-to-end behaviour of the `repovec-qdrant-api-key` service — system
user creation, key-file mode and ownership, rootful Podman secret lifecycle —
is exercised by a Python-based integration harness rooted at
`integration-tests/`. The harness is opt-in: it is **not** part of `make test`
and does not run in default CI, because its lifecycle suite requires
privileges that CI runners typically do not grant.

### 6.1 Suite layout

| Suite                                          | Marker          | Runtime requirement                                                       |
| ---------------------------------------------- | --------------- | ------------------------------------------------------------------------- |
| `provisioning/test_qdrant_api_key_cmd_mox.py`  | `cmd_mox`       | Python and the harness dependencies. No container runtime needed.         |
| `provisioning/test_qdrant_api_key.py`          | `integration`   | A Docker-compatible runtime able to host privileged Podman-in-Podman.     |

The `cmd_mox` suite exercises the helper's external command orchestration by
running the real shell script through a `PATH` populated with `cmd-mox`
shims. The `integration` suite runs the real helper inside a privileged
Fedora container managed by `testcontainers-python`, and uses rootful nested
Podman to validate the full secret lifecycle.

### 6.2 Prerequisites

- Python 3.13 (managed by [`uv`](https://github.com/astral-sh/uv)).
- The harness dependencies, installed via `cd integration-tests && uv sync`.
  This matches the contract the `integration-test` and
  `integration-command-test` Makefile targets advertise in their
  skip-message hint, so a `uv sync` here is sufficient to satisfy
  their prerequisite checks. It brings in `pytest`,
  `testcontainers-python`, `cuprum`, `cmd-mox`, and the `docker` SDK
  that `testcontainers` uses to talk to the runtime.
- For the `integration` suite only: a reachable Docker API socket and the
  ability to launch privileged containers. On Linux, the canonical
  configuration is rootless Podman exposing its socket via
  `podman system service`; see `integration-tests/README.md` for the exact
  environment variables (`DOCKER_HOST`, `TESTCONTAINERS_RYUK_DISABLED`).

### 6.3 Make targets

```sh
set -o pipefail
PYTHON=$(pwd)/integration-tests/.venv/bin/python \
    make integration-command-test 2>&1 \
    | tee /tmp/repovec-integration-command-test.log
set -o pipefail
PYTHON=$(pwd)/integration-tests/.venv/bin/python \
    make integration-test 2>&1 \
    | tee /tmp/repovec-integration-test.log
```

Set `PYTHON` to the path of an interpreter whose environment has the harness
dependencies installed (typically the `uv`-managed `.venv`). Pass extra
options to pytest via `PYTEST_FLAGS`, for example
`PYTEST_FLAGS=--no-skip-on-missing-runtime` to convert any "no runtime"
skips inside the harness into hard failures — useful for CI jobs that must
guarantee the suite ran.

### 6.4 Internal harness APIs

The harness exposes a small surface intended for use by tests, not by the
appliance itself. None of these symbols ship in any wheel.

- `lib.container.ContainerSession` — argv-first wrapper around the running
  `DockerContainer`. Tests prefer `session.must_run("getent", "passwd",
  "repovec")` over hand-rolled `subprocess` calls, so failures surface
  rendered stdout/stderr/exit-code rather than opaque traceback fragments.
- `lib.commands.run_host` — `cuprum`-backed host-side command runner with a
  curated allowlist (`python3`, `podman`, `git`, `gh`, `coderabbit`, `sh`).
  Anything off the allowlist raises `UnknownProgramError`; that is the safety
  rail against ad hoc shell-outs leaking into test code.
- `lib.assertions` — domain-level assertion helpers (`assert_repovec_user`,
  `assert_key_file_contract`, `assert_podman_secret_exists`) that hide raw
  container shell-outs behind named predicates, so failures point at the
  contract being violated rather than the shell incantation.
- `integration_container` / `container_session` pytest fixtures — session-
  scoped privileged container plus a per-test wrapper whose `cleanup_state`
  runs both before and after every test, isolating lifecycle scenarios
  without paying for a fresh container build per case.
- `patched_helper` / `helper_env` pytest fixtures — `cmd-mox` analogue that
  copies the real helper script into `tmp_path`, rewrites `CONFIG_DIR`,
  `KEY_FILE`, and `LOCK_FILE` to point at the tmp tree, and supplies a
  minimal environment overlay (`HOME` + `LANG` only; `PATH` is sourced live
  from `os.environ` at invocation time so `cmd-mox` shims are always
  picked up).

`integration-tests/README.md` is the operator-level entry point for running
the suite; this section is the developer-level reference for extending it.

## 7. GitHub OAuth device-flow implementation

Device-flow authentication follows the repository's boundary rule:
`repovec-core` owns protocol policy and `repovecd` owns runtime adapters.

The pure policy module is `repovec_core::github_oauth`. Keep device-flow error
classification, poll-interval decisions, terminal errors, and redacted secret
wrappers in this module. This code must remain independent of HTTP, clocks,
sleeping, `systemd-creds`, and the filesystem.

The runtime modules live in `repovecd`:

- `github_device_flow`: application orchestration and ports for the OAuth API,
  token store, and sleeper.
- `github_oauth_client`: blocking HTTP adapter for the device-code and token
  endpoints.
- `github_token_store`: encrypted credential persistence under
  `/etc/repovec/`.

Device-flow expiry and polling deadlines must use the `monotony`
`MonotonicClock` port over `std::time::Instant`; do not use wall-clock time
(`SystemTime`, UTC, local time, or chrono timestamps) for elapsed-time
decisions. The runtime adapter should inject the clock alongside the sleeper so
tests can drive deterministic instants without sleeping. Test clocks should use
`monotony`'s `test-util` helpers and be queued or otherwise explicitly advanced
across `now()` calls, and expiry tests should fail if production code bypasses
the injected clock by calling `Instant::now()` directly.

Do not pass GitHub access tokens on command lines or include them in formatted
diagnostics. The token-store adapter encrypts through `systemd-creds` with
`--name=repovec-github-oauth-token`, writes `github-oauth-token.cred`
atomically, and redacts known GitHub token prefixes in captured stderr. Persist
only the bearer token secret; scopes returned by the OAuth server are response
metadata and are not restored when a token is loaded from disk.

Use dependency injection at every network, persistence, and time boundary.
Unit tests should use `rstest` fixtures. Behavioural tests should use
`rstest-bdd` for policy scenarios. The `repovecd` example binary
`device-flow-test` is the externally observable success criterion: it starts
`oauth2-test-server`, completes a local device-flow exchange, stores the token
through the encrypted-store boundary, reloads it, and verifies the same token
secret is recovered.
