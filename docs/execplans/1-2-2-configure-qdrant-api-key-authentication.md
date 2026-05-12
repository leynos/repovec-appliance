# Configure Qdrant API-key authentication

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: COMPLETE

## Purpose / big picture

Roadmap item `1.2.2` completes the security half of the local Qdrant service
contract. After this work is approved and implemented, a fresh appliance boot
creates one random Qdrant API key, stores the raw key at
`/etc/repovec/qdrant-api-key`, exposes that key to the Qdrant container as
`QDRANT__SERVICE__API_KEY`, and starts Qdrant with API-key authentication
enabled. A local process that calls Qdrant without the `api-key` header is
rejected; a local process that supplies the stored key succeeds.

Implementation is complete. The progress log records the approval,
implementation, validation, review follow-ups, and rebase outcomes for this
roadmap item.

## Constraints

- Implementation was approved before code changes began; this completed plan
  now records the final state and validation evidence.
- Keep the scope to roadmap item `1.2.2`. Do not implement Qdrant health checks
  from `1.2.3`, the full `repovec.target` and daemon unit layout from `1.3.1`,
  or the full data-directory layout from `1.3.3`.
- The checked-in Qdrant Quadlet remains
  `packaging/systemd/qdrant.container`, and the installed rootful Quadlet path
  remains `/etc/containers/systemd/qdrant.container`.
- The generated key file path is exactly `/etc/repovec/qdrant-api-key`.
- The key file stores the raw key, not a shell-style assignment, so future
  callers can read the key without parsing an environment file.
- The Qdrant container receives the key through Qdrant's supported environment
  variable, `QDRANT__SERVICE__API_KEY`.
- Secrets must not be committed, logged, embedded in the Quadlet, or included
  in test snapshots.
- The `repovec` system user must be the only non-root account able to read the
  key file. If this conflicts with platform behaviour, stop and document the
  conflict before changing the permission model.
- Domain and policy validation belongs in `repovec-core`; filesystem,
  systemd, Podman and operating-system user management are adapters or
  packaging concerns. Do not make pure validation code perform I/O.
- Use `rstest` for unit tests and `rstest-bdd` for behavioural tests where the
  implementation adds or changes test coverage.
- Use the repository Makefile targets for validation. Run `make check-fmt`,
  `make lint`, and `make test` before any implementation commit. Because this
  task changes documentation, also run `make fmt`, `make markdownlint`, and
  `make nixie` as relevant documentation gates.
- Run gate commands sequentially and capture output with `tee` logs under
  `/tmp`.
- Keep Rust source files under the repository's 400-line guidance. If extending
  `qdrant_quadlet/mod.rs` or `tests.rs` would exceed that, split cohesive logic
  into small sibling modules.
- Use en-GB Oxford spelling in documentation, except for exact API names such
  as `QDRANT__SERVICE__API_KEY`.

## Tolerances (exception triggers)

- Scope: if implementation requires changing more than 12 files or more than
  650 net lines outside tests and documentation, stop and ask for review.
- Interface: if a public Rust API outside
  `repovec_core::appliance::qdrant_quadlet` or a new appliance-secret module
  must change, stop and ask for review.
- Dependencies: if a new runtime crate dependency is required for key
  generation or system interaction, stop and ask for approval. Development
  dependencies already present in the workspace, such as `rstest` and
  `rstest-bdd`, do not trigger this tolerance.
- Platform: if Podman Quadlet on the target Rocky/systemd version cannot use
  `Secret=...,type=env,target=QDRANT__SERVICE__API_KEY`, stop and present
  alternatives.
- Security: if the implementation cannot keep the raw key out of logs, process
  arguments, committed files and test output, stop and redesign.
- Permissions: if systemd or Podman cannot read the Podman secret while the raw
  key file is readable only by `root` and `repovec`, stop and document the
  least-privilege alternatives.
- Testing: if `make check-fmt`, `make lint`, or `make test` still fails after
  two focused fix attempts, stop and report the failing logs.
- Ambiguity: if the prompt's terminal-handling completion criteria appear to be
  intended for this roadmap item after all, stop. Those criteria do not match
  Qdrant API-key authentication.

## Risks

- Risk: roadmap item `1.2.2` needs a `repovec` system user, but roadmap item
  `1.3.3` separately creates that user and the wider directory layout.
  Severity: medium. Likelihood: high. Mitigation: pull forward only the minimal
  user prerequisite needed for the key file, document that `1.3.3` still owns
  the full runtime directory layout, and keep the provisioning idempotent if
  the user already exists.

- Risk: using a plain `EnvironmentFile=` would force
  `/etc/repovec/qdrant-api-key` to contain `QDRANT__SERVICE__API_KEY=<key>`
  rather than the raw key requested by the roadmap. Severity: medium.
  Likelihood: medium. Mitigation: use a Podman secret backed by the raw key
  file and expose that secret to the container as the
  `QDRANT__SERVICE__API_KEY` environment variable.

- Risk: Podman secret creation is host-runtime behaviour, while CI is unlikely
  to provide a writable systemd and Podman environment. Severity: medium.
  Likelihood: high. Mitigation: cover static contracts in Rust unit and BDD
  tests, and document a manual smoke test for Podman/systemd hosts.

- Risk: API-key authentication proves itself only against a running Qdrant
  process. Severity: medium. Likelihood: high. Mitigation: keep static CI tests
  for the Quadlet/provisioning contract and add an end-to-end smoke procedure
  that starts Qdrant and verifies unauthenticated rejection and authenticated
  success.

- Risk: key rotation is operationally important but not requested by roadmap
  item `1.2.2`. Severity: medium. Likelihood: medium. Mitigation: make the
  first-boot provisioning idempotent and avoid hardening choices such as
  `chattr +i` that would block later rotation work. Document rotation as future
  work.

- Risk: local root can inspect container environment and Podman secret state.
  Severity: low. Likelihood: high. Mitigation: define the threat model as local
  unprivileged-process isolation. The appliance is single-owner; root
  compromise is outwith the scope of this roadmap item.

## Progress

- [x] (2026-05-08T01:52:56Z) Read `AGENTS.md`, the `execplans`, `leta`,
  `rust-router`, `hexagonal-architecture`, `commit-message`, `pr-creation`, and
  `en-gb-oxendict-style` guidance relevant to planning, validation,
  documentation and PR creation.
- [x] (2026-05-08T01:52:56Z) Confirmed the current branch was
  `feat/qdrant-api-key-plan` and the worktree was clean before writing the plan.
- [x] (2026-05-08T01:52:56Z) Inspected `docs/roadmap.md`,
  `docs/repovec-appliance-technical-design.md`, `docs/developers-guide.md`,
  `docs/users-guide.md`, `packaging/systemd/qdrant.container`, and the existing
  `qdrant_quadlet` Rust validation module.
- [x] (2026-05-08T01:52:56Z) Created context pack `pk_ra2vbuju` for the
  planning team with the roadmap, technical design, developer-guide, and
  Quadlet references.
- [x] (2026-05-08T01:52:56Z) Used three Wyvern agents for read-only planning
  review of systemd/Podman design, Rust validation/testing, and
  documentation/delivery requirements.
- [x] (2026-05-08T01:52:56Z) Drafted this pre-implementation ExecPlan.
- [x] (2026-05-08T10:14:00+02:00) Received explicit user approval to
  implement this ExecPlan.
- [x] (2026-05-08T10:17:00+02:00) Re-read the plan, branch state, current
  Quadlet, existing `qdrant_quadlet` validator, behavioural tests and packaging
  directory before editing.
- [x] (2026-05-08T10:42:00+02:00) Added
  `packaging/systemd/repovec-qdrant-api-key.service`,
  `packaging/libexec/repovec-qdrant-api-key`, and
  `packaging/sysusers.d/repovec.conf` for first-boot key provisioning.
- [x] (2026-05-08T10:44:00+02:00) Updated
  `packaging/systemd/qdrant.container` so `qdrant.service` requires and starts
  after the provisioning unit and receives the Podman secret as
  `QDRANT__SERVICE__API_KEY`.
- [x] (2026-05-08T10:48:00+02:00) Extended
  `repovec_core::appliance::qdrant_quadlet` with typed validation for
  provisioning dependencies, Podman secret injection, and inline API-key
  rejection.
- [x] (2026-05-08T10:51:00+02:00) Added `rstest` unit coverage for the new
  Quadlet authentication contract and static provisioning-asset checks.
- [x] (2026-05-08T10:53:00+02:00) Added `rstest-bdd` scenarios for accepted
  secret injection, missing API-key secret rejection, and inline API-key
  rejection.
- [x] (2026-05-08T10:55:00+02:00) Updated the technical design, users guide,
  developers guide, documentation index, and roadmap entry for `1.2.2`.
- [x] (2026-05-08T11:04:00+02:00) Ran the required gates and captured logs
  under `/tmp`.
- [x] (2026-05-08T11:06:00+02:00) Marked roadmap item `1.2.2` done after
  implementation, validation, and documentation updates were complete.
- [x] (2026-05-08T11:16:00+02:00) Received user approval to proceed with the
  cohesive change set despite the file-count tolerance exception.
- [x] (2026-05-10T00:00:00+02:00) Verified follow-up review findings against
  current code and applied the still-valid fixes for `Environment=`
  tokenization, duplicate BDD coverage, sysusers documentation, safe curl
  documentation, newline-free key generation and Podman-secret removal
  rationale.
- [x] (2026-05-10T00:00:00+02:00) Addressed additional review comments by
  centralizing packaging asset inclusion in tests, narrowing the no-leak helper
  assertion, strengthening the canonical key-path assertion, and adding wrong
  `Secret=...,type=...` coverage.
- [x] (2026-05-10T00:00:00+02:00) Addressed architecture and observability
  checks by simplifying the API-key test helper, making API-key contract
  constants private, adding Display snapshot coverage, and logging safe
  provisioning decisions to journald.
- [x] (2026-05-11T00:00:00+02:00) Addressed final review findings by redacting
  inline API-key Display output, adding wrong `After=` coverage, hardening
  helper key generation, and removing argv secret exposure from the ExecPlan
  smoke command.

## Surprises & Discoveries

- Observation: the repository already has a focused static validator for the
  Qdrant Quadlet, plus `rstest` and `rstest-bdd` coverage. Evidence:
  `crates/repovec-core/src/appliance/qdrant_quadlet/` and
  `crates/repovec-core/tests/qdrant_quadlet_bdd.rs`. Impact: `1.2.2` should
  extend that validation surface rather than creating a parallel contract
  checker.

- Observation: `1.2.2` requires permissions for a `repovec` system user before
  the roadmap's full user/directory task in `1.3.3`. Evidence:
  `docs/roadmap.md` lists API-key permissions in `1.2.2` and the complete
  user/directory layout in `1.3.3`. Impact: implementation should pull forward
  only the minimal user prerequisite required for the key file and document the
  hand-off to `1.3.3`.

- Observation: current Qdrant documentation confirms that environment
  variables override file-based configuration and that
  `QDRANT__SERVICE__API_KEY` maps to `service.api_key`. Evidence: Qdrant
  configuration and security documentation. Impact: the Quadlet should pass
  exactly that environment variable.

- Observation: current Podman documentation supports passing secrets to
  containers as environment variables with `secret,type=env,target=<ENV_NAME>`.
  Evidence: Podman `--secret` and Quadlet documentation. Impact: a Podman
  secret lets the file remain a raw key while Qdrant receives the required
  environment variable.

- Observation: the repository has no package-install manifest yet; the only
  existing packaging asset before this item was
  `packaging/systemd/qdrant.container`. Evidence:
  `find packaging -maxdepth 3 -type f`. Impact: this item can add the source
  assets and static validation, but install placement remains a later packaging
  concern unless the roadmap introduces an installer.

- Observation: `shellcheck` is documented as available, but it is not installed
  in this worktree environment. Evidence: running
  `shellcheck packaging/libexec/repovec-qdrant-api-key` returned
  `/bin/bash: line 1: shellcheck: command not found`. Impact: shell syntax was
  reviewed through static Rust asset tests and the repository gates, but a
  dedicated shellcheck run could not be used here.

- Observation: the local development environment is not a disposable
  systemd/Podman appliance host. Evidence: this worktree runs inside the coding
  sandbox and the implementation only has source packaging assets, not an
  installed rootful Quadlet environment. Impact: the end-to-end Qdrant HTTP
  smoke test remains a reviewer/manual appliance-host check using the commands
  in this plan.

- Observation: follow-up review found that `Environment=` values may contain
  quoted values or multiple assignments, and the validator only checked each
  raw Quadlet value as a whole. Evidence:
  `validate_no_inline_api_key_environment` previously used
  `environment.starts_with(...)`. Impact: validation now tokenizes
  `Environment=` values before checking for inline Qdrant API-key assignments.

## Decision Log

- Decision: This plan uses a raw key file plus Podman secret environment
  injection, not `EnvironmentFile=/etc/repovec/qdrant-api-key`. Rationale: the
  roadmap names `/etc/repovec/qdrant-api-key` as the key store. A raw key file
  satisfies that directly, and Podman can expose the same secret as
  `QDRANT__SERVICE__API_KEY` inside the container without hard-coding the
  secret in the Quadlet. Date/Author: 2026-05-08, Codex with Wyvern planning
  input.

- Decision: Add a one-shot provisioning unit or helper owned by packaging, not
  a long-running daemon feature. Rationale: this is first-boot appliance setup
  for a systemd-managed container. It must run before `qdrant.service` and
  remain idempotent on reboot; it does not need the future reconciler.
  Date/Author: 2026-05-08, Codex with Wyvern planning input.

- Decision: Pull forward only the minimal `repovec` user prerequisite required
  for the Qdrant key. Rationale: the roadmap requires the key file to be
  restricted to `repovec`. Full data-root and service layout remain `1.3.3`
  work, but the key cannot be permissioned to a user that does not exist.
  Date/Author: 2026-05-08, Codex.

- Decision: Treat the prompt's terminal-handling completion criteria as a
  copied mismatch unless the user clarifies otherwise. Rationale: interactive
  terminal sessions, resize propagation and exit-code reporting do not map to
  Qdrant API-key authentication. Date/Author: 2026-05-08, Codex.

- Decision: Do not require property tests, Kani, or Verus for this item.
  Rationale: this change introduces fixed static contracts and idempotent
  first-boot state, not a broad invariant over large input spaces or a new
  formal business axiom. Focused `rstest`, `rstest-bdd`, and manual system
  smoke checks provide better value here. Date/Author: 2026-05-08, Codex with
  Wyvern planning input.

- Decision: Add `packaging/sysusers.d/repovec.conf` and keep a fallback
  `useradd --system` path in the provisioning helper. Rationale: the sysusers
  asset is the declarative package-owned source for the minimal user
  prerequisite, while the helper fallback keeps first-boot provisioning
  idempotent if the sysusers file has not yet been installed or processed.
  Date/Author: 2026-05-08, Codex.

- Decision: If the Podman secret already exists and cannot be removed, the
  provisioning helper exits successfully instead of failing a running system.
  Rationale: normal boot and restart paths refresh the secret before Qdrant
  starts. If an operator reruns the helper while Qdrant still uses the secret,
  preserving the existing secret is safer than breaking the running service or
  logging secret material. Date/Author: 2026-05-08, Codex.

- Decision: Use a small quote-aware splitter for Quadlet `Environment=` values
  rather than introducing a new runtime dependency. Rationale: the validator
  needs only enough shell-like tokenization to distinguish quoted assignments
  and whitespace-separated assignments; keeping this local preserves the pure
  validation boundary and avoids a dependency solely for a static asset check.
  Date/Author: 2026-05-10, Codex.

- Decision: Keep packaging asset includes in a small test-local macro rather
  than duplicating
  `concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/...")` at each include
  site. Rationale: `include_str!` still requires compile-time literals, but
  centralizing the manifest-relative prefix leaves one place to update if the
  crate moves. Date/Author: 2026-05-10, Codex.

- Decision: Implementation reached the ExecPlan scope tolerance for file count.
  Rationale: the plan set an exception trigger at more than 12 changed files.
  This implementation needs separate files for the existing Rust validator
  modules, unit tests, BDD feature and step definitions, systemd unit, helper
  script, sysusers asset, and required documentation updates. Options are:
  approve the larger but cohesive change set, remove optional discoverability
  and sysusers assets to reduce the count slightly, or split the work into two
  commits/PRs. Trade-off: splitting or removing assets lowers file count but
  makes the feature less self-contained. Date/Author: 2026-05-08, Codex.

## Outcomes & Retrospective

Implemented roadmap item `1.2.2`. The repository now ships:

- `packaging/libexec/repovec-qdrant-api-key`, a one-shot helper that creates
  the minimal `repovec` user if needed, creates `/etc/repovec`, generates a raw
  random key at `/etc/repovec/qdrant-api-key` only when missing, locks the file
  to `repovec:repovec` mode `0400`, and creates the rootful Podman secret
  `repovec-qdrant-api-key`.
- `packaging/systemd/repovec-qdrant-api-key.service`, a systemd oneshot unit
  that runs the helper before Qdrant.
- `packaging/sysusers.d/repovec.conf`, the declarative minimal system-user
  prerequisite.
- Updated `packaging/systemd/qdrant.container` wiring so Qdrant requires the
  provisioning unit and receives the Podman secret as
  `QDRANT__SERVICE__API_KEY`.
- Extended pure Rust validation and tests in
  `crates/repovec-core/src/appliance/qdrant_quadlet/` and
  `crates/repovec-core/tests/`.
- Updated `docs/repovec-appliance-technical-design.md`, `docs/users-guide.md`,
  `docs/developers-guide.md`, `docs/contents.md`, and `docs/roadmap.md`.

Validation evidence:

- `make fmt` passed; log:
  `/tmp/fmt-repovec-1-2-2-qdrant-api-key-impl.out`.
- `make markdownlint` passed; log:
  `/tmp/markdownlint-repovec-1-2-2-qdrant-api-key-impl.out`.
- `make nixie` passed; log:
  `/tmp/nixie-repovec-1-2-2-qdrant-api-key-impl.out`.
- `make check-fmt` passed; log:
  `/tmp/check-fmt-repovec-1-2-2-qdrant-api-key-impl.out`.
- `make lint` passed after fixing local Clippy findings; log:
  `/tmp/lint-repovec-1-2-2-qdrant-api-key-impl.out`.
- `make test` passed with 71 nextest tests and 24 doctests; log:
  `/tmp/test-repovec-1-2-2-qdrant-api-key-impl.out`.

The manual systemd/Podman smoke test was not run in this coding environment.
Use the smoke commands in this ExecPlan on a disposable appliance-style host to
verify the live Qdrant behaviour: unauthenticated `curl` should return `401` or
`403`, and authenticated `curl` with the stored key should return `200`.

## Context and orientation

The repository is a Rust workspace for a VM appliance. `repovec-core` contains
shared types and appliance validation helpers. The existing Qdrant work from
roadmap item `1.2.1` added:

- `packaging/systemd/qdrant.container`, the checked-in Quadlet asset.
- `crates/repovec-core/src/appliance/qdrant_quadlet/`, a pure Rust validator
  that parses the Quadlet and checks image, loopback ports, storage mount and
  auto-update policy.
- `crates/repovec-core/src/appliance/qdrant_quadlet/tests.rs`, `rstest` unit
  coverage for the validator.
- `crates/repovec-core/tests/qdrant_quadlet_bdd.rs` and
  `crates/repovec-core/tests/features/qdrant_quadlet.feature`, `rstest-bdd`
  behavioural coverage for the checked-in Quadlet contract.
- `docs/users-guide.md` and
  `docs/repovec-appliance-technical-design.md`, which currently document the
  Qdrant network and storage contract and mention that API-key authentication
  remains future work.

Qdrant accepts an API key through configuration key `service.api_key`.[^1]
Environment variables with the `QDRANT__` prefix and double-underscore nested
keys override file configuration, so `QDRANT__SERVICE__API_KEY=<secret>`
enables API-key authentication for the service.[^1] Qdrant REST clients send
the key with the `api-key` HTTP header.[^2]

Podman secrets can be exposed either as files or as environment variables. In
Quadlet, the container file can use the `Secret=` key.[^3][^4] For this task,
the intended shape is:

```plaintext
Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY
```

The one-shot provisioning service creates or refreshes the Podman secret from
`/etc/repovec/qdrant-api-key` before Qdrant starts. The raw key file is
persistent and restricted; the Podman secret is the container-injection
mechanism.

## Plan of work

### Stage A: verify the static contract before edits

Start by reading the current state rather than assuming this draft is still
complete. Re-check `docs/roadmap.md`, `packaging/systemd/qdrant.container`,
`docs/repovec-appliance-technical-design.md`, `docs/users-guide.md`, and the
`qdrant_quadlet` validator files.

Confirm that no implementation for `1.2.2` already landed in another branch. If
it did, update this plan before editing code.

### Stage B: add first-boot provisioning assets

Add the minimal packaging asset that creates the Qdrant key and Podman secret
idempotently. The preferred shape is a root-owned one-shot unit:

```plaintext
packaging/systemd/repovec-qdrant-api-key.service
```

The service should:

- ensure the `repovec` system user exists, or depend on a minimal checked-in
  `sysusers.d` asset that creates it before the service runs;
- ensure `/etc/repovec` exists with restrictive permissions;
- if `/etc/repovec/qdrant-api-key` is missing, generate a new high-entropy
  random key using the platform cryptographic random source;
- write the key atomically by creating a temporary file in `/etc/repovec`,
  setting owner and mode before rename where possible, then renaming it to the
  final path;
- set owner `repovec:repovec` and mode `0400` on the final raw key file;
- create or replace the rootful Podman secret named `repovec-qdrant-api-key`
  from that file without printing the key;
- do nothing destructive when the key file already exists.

Prefer a small shell script only if it remains readable and testable by static
contract checks. If a Rust helper is needed, keep policy types in
`repovec-core` and operating-system I/O in the appropriate binary or packaging
adapter. Do not add a broad secret-management framework for this single Qdrant
key.

### Stage C: wire Qdrant Quadlet authentication

Update `packaging/systemd/qdrant.container` to depend on the provisioning unit
and to pass the Podman secret to Qdrant as an environment variable.[^3] The
intended Quadlet contract is:

```plaintext
[Unit]
Requires=repovec-qdrant-api-key.service
After=repovec-qdrant-api-key.service

[Container]
Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY
```

Keep the existing image, `AutoUpdate=registry`, loopback ports and storage
mount. Do not add an `[Install]` section in this item; boot-target ownership
remains `1.3.1`.

### Stage D: extend pure Rust validation

Extend `crates/repovec-core/src/appliance/qdrant_quadlet/` so the checked-in
Quadlet fails validation unless the authentication contract is present.

Add constants for the canonical service name, secret name, secret target and
key file path where useful. Add `QdrantQuadletError` variants for missing or
wrong authentication wiring. Suggested variants are:

```rust
MissingApiKeySecret
IncorrectApiKeySecret { secret: String }
MissingApiKeyProvisioningDependency
IncorrectApiKeyProvisioningDependency { dependency: String }
InlineApiKeyEnvironmentDisallowed { environment: String }
```

The exact names may change during implementation, but the errors should remain
semantic and caller-inspectable. Validation should reject:

- missing `Secret=...target=QDRANT__SERVICE__API_KEY`;
- wrong secret name;
- wrong environment target;
- missing `Requires=repovec-qdrant-api-key.service`;
- missing `After=repovec-qdrant-api-key.service`;
- inline `Environment=QDRANT__SERVICE__API_KEY=...` values in the checked-in
  asset.

Keep this validation I/O-free. It parses strings and enforces appliance policy.
It does not read `/etc/repovec`, call Podman, call systemd, or generate keys.

### Stage E: add unit and behavioural tests

Extend `crates/repovec-core/src/appliance/qdrant_quadlet/tests.rs` with
`rstest` fixtures derived from the checked-in Quadlet. Cover at least:

- `checked_in_qdrant_quadlet_remains_valid` still passes;
- removing the `Secret=` entry fails;
- changing the secret name fails;
- changing `target=QDRANT__SERVICE__API_KEY` fails;
- removing `Requires=repovec-qdrant-api-key.service` fails;
- removing `After=repovec-qdrant-api-key.service` fails;
- adding inline `Environment=QDRANT__SERVICE__API_KEY=not-secret` fails;
- keeping other valid existing Quadlet fields unchanged still passes.

Extend `crates/repovec-core/tests/features/qdrant_quadlet.feature` and
`crates/repovec-core/tests/qdrant_quadlet_bdd.rs` with behavioural scenarios
such as:

```gherkin
Scenario: The checked-in Quadlet supplies the Qdrant API key from a Podman secret
  Given the checked-in Qdrant Quadlet
  When the Quadlet is validated
  Then the Quadlet is accepted

Scenario: The Qdrant API key secret must be present
  Given the checked-in Qdrant Quadlet
  And the API key secret is removed
  When the Quadlet is validated
  Then the validation fails because the API key secret is missing

Scenario: Inline Qdrant API keys are rejected
  Given the checked-in Qdrant Quadlet
  And the API key is inlined as an environment variable
  When the Quadlet is validated
  Then the validation fails because inline API keys are not allowed
```

If a provisioning script or unit validator is added, include focused tests for
the static contract: expected path, mode, owner, idempotence guard, no raw key
literal in checked-in files, and Podman secret target.

### Stage F: update documentation

Update `docs/repovec-appliance-technical-design.md` under the Qdrant
Podman/systemd section with the final authentication design:

- raw key path `/etc/repovec/qdrant-api-key`;
- owner/mode policy;
- Podman secret name;
- Qdrant environment variable;
- one-shot provisioning before `qdrant.service`;
- unauthenticated and authenticated request behaviour.

Update `docs/users-guide.md` by replacing the current placeholder that says
`1.2.2` is future work. Document how operators can inspect service state
without printing the key, and how local clients authenticate by reading the key
as the `repovec` user and sending `api-key: <key>`.

Update `docs/developers-guide.md` with the new validation surface and any
provisioning asset conventions.

Update `docs/contents.md` if the new ExecPlan should be discoverable from the
documentation index.

Update `docs/roadmap.md` only after implementation and validation are complete:
mark item `1.2.2` as done and add a short status note. Do not mark it done in
the pre-implementation planning commit.

### Stage G: manual system smoke validation

On a disposable host with Podman, systemd and permissions matching the target
appliance, install the generated assets and verify the externally observable
contract:

1. `systemctl daemon-reload` succeeds.
2. Starting `qdrant.service` starts or requires the key provisioning service.
3. `/etc/repovec/qdrant-api-key` exists, has stable contents across restart,
   is owned by `repovec:repovec`, and is not world-readable.
4. The Podman secret `repovec-qdrant-api-key` exists.
5. `curl http://127.0.0.1:6333/collections` without `api-key` returns an
   authentication failure status, expected `401` or `403`.
6. `curl -H "api-key: <stored key>" http://127.0.0.1:6333/collections`
   returns success, expected `200`.

If a real Podman/systemd smoke test cannot be run in the development
environment, record that explicitly in the implementation PR and include the
exact manual procedure for a reviewer to run.

## Concrete steps

Run these commands from the repository root:

```bash
git branch --show-current
git status --short
```

Expected transcript:

```plaintext
1-2-2-configure-qdrant-api-key-authentication
```

No output from `git status --short` means the worktree is clean. If the branch
name is still different during implementation, rename it before committing:

```bash
git branch -m 1-2-2-configure-qdrant-api-key-authentication
```

Inspect the relevant project context:

```bash
sed -n '37,70p' docs/roadmap.md
sed -n '337,371p' docs/repovec-appliance-technical-design.md
sed -n '60,100p' docs/users-guide.md
sed -n '1,80p' packaging/systemd/qdrant.container
```

After implementation edits, run formatting and documentation gates first:

```bash
set -o pipefail && make fmt 2>&1 | tee /tmp/fmt-repovec-1-2-2-qdrant-api-key.out
set -o pipefail && make markdownlint 2>&1 | tee /tmp/markdownlint-repovec-1-2-2-qdrant-api-key.out
set -o pipefail && make nixie 2>&1 | tee /tmp/nixie-repovec-1-2-2-qdrant-api-key.out
```

Then run Rust gates sequentially:

```bash
set -o pipefail && make check-fmt 2>&1 | tee /tmp/check-fmt-repovec-1-2-2-qdrant-api-key.out
set -o pipefail && make lint 2>&1 | tee /tmp/lint-repovec-1-2-2-qdrant-api-key.out
set -o pipefail && make test 2>&1 | tee /tmp/test-repovec-1-2-2-qdrant-api-key.out
```

A successful gate exits with status `0`. `make lint` may print that Whitaker is
not installed and skip that optional sub-check; that is acceptable only if the
Makefile target still exits successfully.

Use this manual smoke sequence on a disposable systemd/Podman host after
installing the generated assets:

```bash
sudo systemctl daemon-reload
sudo systemctl start qdrant.service
sudo stat -c '%U:%G %a %n' /etc/repovec/qdrant-api-key
sudo -u repovec test -r /etc/repovec/qdrant-api-key
curl -sS -o /tmp/qdrant-unauthenticated.out -w '%{http_code}\n' \
  http://127.0.0.1:6333/collections
sudo -u repovec sh -c \
  'awk "{ print \"api-key: \" \$0 }" /etc/repovec/qdrant-api-key | \
  curl -sS -o /tmp/qdrant-authenticated.out -w "%{http_code}\n" --header @- \
  http://127.0.0.1:6333/collections'
```

Expected observations:

```plaintext
repovec:repovec 400 /etc/repovec/qdrant-api-key
401
200
```

Qdrant may use `403` rather than `401` for unauthenticated rejection. Treat
either as success if authenticated access returns `200`.

## Validation and acceptance

This feature is done when all of the following are true:

- A fresh boot creates `/etc/repovec/qdrant-api-key` exactly once, with a random
  raw key, owner `repovec:repovec`, and mode `0400`.
- Re-running the provisioning service does not change an existing key.
- Qdrant starts only after the provisioning service has run successfully.
- The checked-in Quadlet passes the Rust validator only when it depends on the
  provisioning service and exposes Podman secret `repovec-qdrant-api-key` as
  `QDRANT__SERVICE__API_KEY`.
- The checked-in Quadlet fails validation if the API-key secret is removed,
  renamed, targeted at the wrong environment variable, or replaced by an inline
  `Environment=QDRANT__SERVICE__API_KEY=...` secret.
- `rstest` unit tests cover the happy path, unhappy paths and edge cases listed
  in this plan.
- `rstest-bdd` behavioural scenarios describe the API-key Quadlet contract.
- Documentation records the design decisions and operator-visible behaviour.
- `docs/roadmap.md` marks `1.2.2` done only after implementation and
  validation are complete.
- `make check-fmt`, `make lint`, and `make test` pass.
- Documentation gates pass for the Markdown changes: `make fmt`,
  `make markdownlint`, and `make nixie`.
- A Podman/systemd smoke test proves that unauthenticated requests to Qdrant
  are rejected and authenticated requests using the stored key succeed, or the
  implementation PR records why that smoke test could not be run locally and
  gives exact reviewer commands.

## Idempotence and recovery

The provisioning service must be safe to rerun. If the key file already exists,
the service validates ownership and mode, ensures the Podman secret exists or
is refreshed from the same key, and leaves the key value unchanged.

If provisioning fails before the final rename, only a temporary file in
`/etc/repovec` may remain. A retry should either reuse no partial data or clean
that temporary file before generating a new key. Never overwrite an existing
final key unless a future explicit rotation command is introduced.

If the Podman secret becomes inconsistent with the raw key file, rerunning the
provisioning service should recreate the Podman secret from
`/etc/repovec/qdrant-api-key` without generating a new raw key.

If Qdrant starts without authentication during development, stop the service,
inspect the generated unit and Podman secret, fix the contract, and rerun the
manual smoke test. Do not mark the roadmap item done until unauthenticated
requests are rejected.

## Artifacts and notes

Primary local documents and code paths:

- `docs/roadmap.md`
- `docs/repovec-appliance-technical-design.md`
- `docs/users-guide.md`
- `docs/developers-guide.md`
- `packaging/systemd/qdrant.container`
- `packaging/systemd/repovec-qdrant-api-key.service`
- `crates/repovec-core/src/appliance/qdrant_quadlet/mod.rs`
- `crates/repovec-core/src/appliance/qdrant_quadlet/error.rs`
- `crates/repovec-core/src/appliance/qdrant_quadlet/tests.rs`
- `crates/repovec-core/tests/features/qdrant_quadlet.feature`
- `crates/repovec-core/tests/qdrant_quadlet_bdd.rs`

## Interfaces and dependencies

The planned static Quadlet contract is built around Quadlet `Secret=` support
and standard `[Unit]` dependency pass-through:[^4]

```plaintext
[Unit]
Requires=repovec-qdrant-api-key.service
After=repovec-qdrant-api-key.service

[Container]
Image=docker.io/qdrant/qdrant:v1
AutoUpdate=registry
Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY
PublishPort=127.0.0.1:6333:6333
PublishPort=127.0.0.1:6334:6334
Volume=/var/lib/repovec/qdrant-storage:/qdrant/storage:Z
```

The planned key file contract is:

```plaintext
Path: /etc/repovec/qdrant-api-key
Contents: raw random API key, no trailing metadata
Owner: repovec
Group: repovec
Mode: 0400
```

The planned Podman secret contract is:

```plaintext
Name: repovec-qdrant-api-key
Source: /etc/repovec/qdrant-api-key
Container target: QDRANT__SERVICE__API_KEY
Container exposure type: environment variable
```

The existing public validation functions remain:

```rust
pub const fn checked_in_qdrant_quadlet() -> &'static str;
pub fn validate_checked_in_qdrant_quadlet() -> Result<(), QdrantQuadletError>;
pub fn validate_qdrant_quadlet(contents: &str) -> Result<(), QdrantQuadletError>;
```

If implementation adds a separate provisioning-asset validator, keep the same
style: pure functions that accept string contents and return semantic error
enums. Do not expose opaque `eyre::Report` from library APIs.

## Revision note

This ExecPlan was created on 2026-05-08 for roadmap item `1.2.2`. The completed
implementation resolved the raw key file versus environment-variable injection
choice by selecting a Podman secret, and implementation remained blocked until
the user explicitly approved the plan.

[^1]: Qdrant configuration documentation: environment variables use the
    `QDRANT__` prefix and nested keys separated by double underscores;
    `QDRANT__SERVICE__API_KEY` configures `service.api_key`.
[^2]: Qdrant security documentation: API-key authentication requires the
    `api-key` header in REST and gRPC clients.
[^3]: Podman secret documentation: `type=env,target=<name>` exposes a secret as
    an environment variable.
[^4]: Podman Quadlet documentation: `.container` units support `Secret=` and
    pass standard `[Unit]` dependencies through to the generated service.
