# Define repovec target and static units

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Roadmap item `1.3.1` adds the first systemd orchestration layer for the repovec
appliance. After the approved implementation lands, an operator can install the
checked-in unit files, enable `repovec.target`, and start the appliance service
group with Qdrant, `repovecd`, `repovec-mcpd`, and `cloudflared` queued as one
managed target.

The observable behaviour is static and operational rather than interactive:
`repovec.target` names the appliance services it starts, `repovecd.service`
waits for and requires `qdrant.service`, and `repovec-mcpd.service` waits for
and requires the local services it depends on. The plan deliberately does not
implement the roadmap item until the user approves this ExecPlan.

## Constraints

- The implementation must not begin until this DRAFT plan is explicitly
  approved.
- The source-of-truth systemd assets must live under `packaging/systemd/`.
- The Qdrant Podman Quadlet remains owned by roadmap item `1.2.1` at
  `packaging/systemd/qdrant.container`. Dependants must reference the generated
  systemd unit name `qdrant.service`, not `qdrant.container` or
  `qdrant.container.service`.
- The implementation must create `repovec.target`, `repovecd.service`, and
  `repovec-mcpd.service`. It must not invent the future
  `repovec-grepai@.service` template from roadmap item `1.3.2`.
- The implementation must not create or rewrite `cloudflared.service`; that
  unit belongs to the installed `cloudflared` package or later appliance
  provisioning. The target may depend on its public unit name.
- Domain and policy validation belongs in `repovec-core`. Process execution,
  installation, and runtime systemd calls remain adapter or operator concerns
  and must not be mixed into the validator.
- Public Rust APIs added for validation must be documented with Rustdoc
  examples, and every new module must begin with a `//!` module comment.
- New validation tests must use `rstest`; behavioural packaging contract tests
  must use `rstest-bdd` where applicable.
- The feature does not introduce a broad state-space invariant or proof
  obligation. Property tests, Kani, or Verus should only be added if the
  implementation broadens beyond finite static unit-file contracts.
- Documentation must use en-GB-oxendict spelling and wrap Markdown paragraphs
  at 80 columns.
- Before committing the implementation, run the repository gates in sequence,
  not in parallel: `make fmt`, `make markdownlint`, `make nixie`,
  `make check-fmt`, `make lint`, and `make test`. At minimum, the
  user-requested `make check-fmt`, `make lint`, and `make test` must pass.

## Tolerances

- Scope: if the implementation requires more than twelve repository files or
  more than 550 net lines, stop and ask whether the task should be split.
- Interface: if an existing public Rust API signature must change, stop and
  ask for approval. Additive APIs inside `repovec-core::appliance` are within
  tolerance.
- Dependencies: if a new external crate is needed, stop and ask for approval.
  The expected implementation can use the existing workspace dependencies:
  `rstest`, `rstest-bdd`, `rstest-bdd-macros`, and `insta`.
- Systemd scope: if correct ordering requires editing or supplying
  `cloudflared.service`, stop and ask for approval because that expands beyond
  the requested static repovec units.
- Runtime scope: if automated validation requires a live systemd daemon,
  root privileges, Podman, network access, or a running Qdrant container, stop
  and replace that with static validation unless the user approves a broader
  integration environment.
- Test iterations: if `make lint` or `make test` still fails after three
  focused fix attempts, stop, document the failing command and log path, and
  ask for direction.
- Ambiguity: if multiple valid dependency graphs materially change boot or stop
  semantics, present the alternatives and wait for a decision.

## Risks

- Risk: `cloudflared.service` ordering cannot be fully controlled without
  editing the package-owned unit. Severity: medium. Likelihood: medium.
  Mitigation: make `repovec.target` want `cloudflared.service`, and consider
  adding `Before=cloudflared.service` to `repovec-mcpd.service` only if static
  validation can prove that the transaction orders the tunnel after the MCP
  daemon without owning the tunnel unit.

- Risk: `qdrant.container` may be mistaken for the unit name that other units
  depend on. Severity: medium. Likelihood: medium. Mitigation: validate that
  checked-in units reference `qdrant.service`. Document the Quadlet-generated
  unit-name convention in the design and developer guide.

- Risk: real systemd behaviour is difficult to test in Continuous Integration
  (CI). Severity: medium. Likelihood: high. Mitigation: validate the checked-in
  unit-file contract statically from Rust, and document a manual smoke test for
  disposable appliance hosts.

- Risk: the prompt includes completion criteria about interactive sessions,
  terminal resize events, and exit codes that do not correspond to roadmap item
  `1.3.1`. Severity: low. Likelihood: high. Mitigation: record this as a
  requirement mismatch and use systemd target startability, dependency
  ordering, static validation, and quality gates as the acceptance criteria for
  this feature.

## Progress

- [x] (2026-05-08T02:23:40Z) Read `AGENTS.md`, loaded the `execplans`,
  `leta`, `rust-router`, `hexagonal-architecture`, `domain-cli-and-daemons`,
  `arch-crate-design`, `commit-message`, and `pr-creation` skills, and renamed
  the branch to `1-3-1-define-repovec-target-and-static-unit-files`.
- [x] (2026-05-08T02:23:40Z) Created context pack `pk_duyt3wbs` for the Wyvern
  planning team with the roadmap item, service-layout design excerpt, and the
  existing Qdrant validation and behavioural-test patterns.
- [x] (2026-05-08T02:23:40Z) Received Wyvern input on the systemd dependency
  graph and the Quadlet naming caveat for `qdrant.service`.
- [x] (2026-05-08T02:23:40Z) Drafted this pre-implementation ExecPlan.
- [ ] Await explicit user approval before implementation begins.
- [ ] Implement static unit files and Rust validation.
- [ ] Update documentation and roadmap after the feature is implemented.
- [ ] Run all required gates and commit the approved implementation.

## Surprises & Discoveries

- Observation: the user-provided completion criteria mention interactive
  sessions, terminal resize propagation, and exit-code handling. Evidence:
  those criteria do not appear in roadmap item `1.3.1` or the service-layout
  section of `docs/repovec-appliance-technical-design.md`. Impact: this plan
  treats them as a carried-over mismatch and does not use them to accept or
  reject the systemd unit feature.

- Observation: `packaging/systemd/qdrant.container` is the source Quadlet, but
  dependent systemd units should reference `qdrant.service`. Evidence: Podman
  Quadlet generates the service unit from the file stem, and the prior `1.2.1`
  plan and users guide already describe starting `qdrant.service`. Impact: the
  validator must reject `qdrant.container` and `qdrant.container.service`
  dependency names in the new unit files.

## Decision Log

- Decision: keep this branch as a pre-implementation planning branch until the
  user approves the ExecPlan. Rationale: the `execplans` skill requires an
  approval gate before execution, and the user explicitly reminded that the
  plan must be approved before it is implemented. Date/Author:
  2026-05-08T02:23:40Z / Codex.

- Decision: model the new validation surface as a separate
  `repovec_core::appliance::systemd_units` module. Rationale:
  `docs/developers-guide.md` already says each appliance asset should get its
  own submodule with checked-in asset loading, typed errors, parser code if
  needed, and focused tests. This protects hexagonal boundaries: validation
  remains policy, while installation and systemd execution remain outside the
  domain. Date/Author: 2026-05-08T02:23:40Z / Codex.

- Decision: keep service unit files static in the repository and let
  `repovec.target` own the `Wants=` list. Rationale: roadmap item `1.3.1` asks
  for a target that wants the appliance units. The services should be started
  through the target rather than independently enabled as separate boot roots.
  Date/Author: 2026-05-08T02:23:40Z / Codex.

- Decision: do not add property tests, Kani, or Verus in the initial
  implementation. Rationale: the requested contracts are finite static
  unit-file requirements. Exhaustive mutation cases with `rstest`, behavioural
  scenarios with `rstest-bdd`, and committed diagnostic snapshots provide
  direct evidence without adding proof tooling that would merely restate the
  same fixed contract. Date/Author: 2026-05-08T02:23:40Z / Codex.

## Outcomes & Retrospective

No implementation outcome exists yet. This plan is in DRAFT state and must be
approved before code or packaging changes are made.

## Context and orientation

The repository is a Rust workspace. The shared crate `crates/repovec-core`
already contains appliance-specific validation helpers under
`crates/repovec-core/src/appliance/`. The existing `qdrant_quadlet` module
embeds `packaging/systemd/qdrant.container` with `include_str!`, parses the
small subset of Quadlet syntax needed for the contract, returns a typed error
enum, and validates the checked-in asset with unit and behavioural tests.

Roadmap item `1.3.1` in `docs/roadmap.md` asks for a `repovec.target` that
wants `qdrant.service`, `repovecd.service`, `repovec-mcpd.service`, and
`cloudflared.service`. It also asks for stub unit files for `repovecd.service`
and `repovec-mcpd.service` with correct `After=` and `Requires=` relationships.

The technical design's "Service layout" section in
`docs/repovec-appliance-technical-design.md` defines the same target-level
service set and says journald should capture all unit output. The Qdrant
section clarifies that `packaging/systemd/qdrant.container` is installed to
`/etc/containers/systemd/qdrant.container`, and that boot-target wiring remains
the responsibility of roadmap item `1.3.1`.

The following documentation should be treated as source material during
implementation:

- `docs/roadmap.md`
- `docs/repovec-appliance-technical-design.md`
- `docs/developers-guide.md`
- `docs/users-guide.md`
- `docs/rust-testing-with-rstest-fixtures.md`
- `docs/rust-doctest-dry-guide.md`
- `docs/reliable-testing-in-rust-via-dependency-injection.md`
- `docs/complexity-antipatterns-and-refactoring-strategies.md`
- `docs/ortho-config-users-guide.md`
- `docs/rstest-bdd-users-guide.md`

The relevant skills are:

- `execplans` for maintaining this living plan.
- `leta` for symbol-aware Rust navigation before modifying code.
- `rust-router`, routed to `arch-crate-design` and
  `domain-cli-and-daemons`, because this feature touches crate placement and
  supervised daemon lifecycle.
- `hexagonal-architecture` to keep static unit-file policy in the core
  validation surface and systemd runtime operations out of the domain.
- `commit-message` for file-based commit messages.
- `pr-creation` for a draft pull request that clearly links this ExecPlan.

## Plan of work

Stage A is the approval gate. Review this document and adjust the dependency
graph, tolerances, or documentation scope if needed. Do not create unit files,
Rust modules, tests, or roadmap status changes until the user explicitly
approves the plan.

Stage B adds the packaging assets. Create `packaging/systemd/repovec.target`,
`packaging/systemd/repovecd.service`, and
`packaging/systemd/repovec-mcpd.service`. The intended `repovec.target`
contract is:

```ini
[Unit]
Description=repovec appliance service group
Wants=qdrant.service repovecd.service repovec-mcpd.service cloudflared.service

[Install]
WantedBy=multi-user.target
```

The intended `repovecd.service` unit contract is:

```ini
[Unit]
Description=repovec control-plane daemon
Requires=qdrant.service
After=qdrant.service

[Service]
Type=simple
User=repovec
Group=repovec
WorkingDirectory=/var/lib/repovec
Environment=HOME=/var/lib/repovec
ExecStart=/usr/bin/repovecd
Restart=on-failure
```

The intended `repovec-mcpd.service` unit contract is:

```ini
[Unit]
Description=repovec MCP bridge daemon
Requires=qdrant.service repovecd.service
After=qdrant.service repovecd.service

[Service]
Type=simple
User=repovec
Group=repovec
WorkingDirectory=/var/lib/repovec
Environment=HOME=/var/lib/repovec
ExecStart=/usr/bin/repovec-mcpd
Restart=on-failure
```

If implementation review shows that `cloudflared.service` must be ordered after
`repovec-mcpd.service` in the same transaction, prefer adding
`Before=cloudflared.service` to `repovec-mcpd.service` over editing
`cloudflared.service`. Stop and ask for approval if that ordering is not
sufficient.

Stage C adds static contract validation in `repovec-core`. Create a new module
directory `crates/repovec-core/src/appliance/systemd_units/` with `mod.rs`,
`error.rs`, `parser.rs` if needed, and `tests.rs`. Re-export the module from
`crates/repovec-core/src/appliance/mod.rs`. Follow the extension pattern in
`docs/developers-guide.md` and the existing `qdrant_quadlet` module.

The public validation API should be additive and narrow. Use names close to the
existing pattern:

```rust
pub const CHECKED_IN_REPOVEC_TARGET_PATH: &str =
    "packaging/systemd/repovec.target";
pub const CHECKED_IN_REPOVECD_SERVICE_PATH: &str =
    "packaging/systemd/repovecd.service";
pub const CHECKED_IN_REPOVEC_MCPD_SERVICE_PATH: &str =
    "packaging/systemd/repovec-mcpd.service";

pub const fn checked_in_repovec_target() -> &'static str;
pub const fn checked_in_repovecd_service() -> &'static str;
pub const fn checked_in_repovec_mcpd_service() -> &'static str;
pub fn validate_checked_in_systemd_units() -> Result<(), SystemdUnitError>;
pub fn validate_systemd_units(
    repovec_target: &str,
    repovecd_service: &str,
    repovec_mcpd_service: &str,
) -> Result<(), SystemdUnitError>;
```

The typed error enum should implement `std::error::Error`, `fmt::Display`,
`Clone`, `Debug`, `Eq`, and `PartialEq`. Cover at least these failures: invalid
line, property before section, missing required unit file section, missing
target `Wants=`, target using `qdrant.container`, missing `repovecd.service`
`Requires=qdrant.service`, missing `repovecd.service` `After=qdrant.service`,
missing `repovec-mcpd.service` `Requires=repovecd.service`, missing
`repovec-mcpd.service` `Requires=qdrant.service`, missing
`repovec-mcpd.service` `After=repovecd.service`, missing `repovec-mcpd.service`
`After=qdrant.service`, and wrong `ExecStart=` binary names.

Stage D adds automated tests. Put `rstest` unit tests alongside the validator
in `crates/repovec-core/src/appliance/systemd_units/tests.rs`. Use fixtures for
the checked-in unit contents and parameterized mutation cases for each error
variant. Add `insta` snapshots for operator-visible error messages, using the
same style documented in `docs/developers-guide.md`.

Add `rstest-bdd` behavioural coverage under
`crates/repovec-core/tests/features/systemd_units.feature` and
`crates/repovec-core/tests/systemd_units_bdd.rs`. Behavioural scenarios should
cover the checked-in unit set being accepted, the target wanting all required
services, `repovecd` depending on Qdrant, `repovec-mcpd` depending on
`repovecd` and Qdrant, and rejection of the wrong Quadlet dependency name. Keep
these tests static; do not start systemd, Podman, Qdrant, or `cloudflared` in
automated tests.

Stage E updates documentation after the implementation is complete. Update
`docs/repovec-appliance-technical-design.md` with the final unit-file paths,
install paths, dependency graph, and any accepted ordering decision for
`cloudflared.service`. Update `docs/users-guide.md` with operator-facing
instructions for installing, enabling, and starting `repovec.target`. Update
`docs/developers-guide.md` with the new `systemd_units` validation surface.
Update `docs/contents.md` if new or renamed documentation needs to be indexed.
Only after the assets, tests, docs, and quality gates pass, mark roadmap item
`1.3.1` as done in `docs/roadmap.md`.

Stage F runs validation and commits. Run formatting first, then documentation
gates, then Rust formatting, linting, and tests. Commit only when the full gate
set passes.

## Concrete steps

1. Confirm the current branch and worktree:

   ```sh
   git branch --show-current
   git status --short
   ```

   Expected branch:

   ```plaintext
   1-3-1-define-repovec-target-and-static-unit-files
   ```

2. After approval, create the three systemd files in `packaging/systemd/`.
   Re-run `git status --short` and verify only expected files changed.

3. Add `crates/repovec-core/src/appliance/systemd_units/` and export it from
   `crates/repovec-core/src/appliance/mod.rs`.

4. Add unit tests and BDD scenarios. Run the targeted tests first:

   ```sh
   cargo test -p repovec-core systemd_units
   cargo test -p repovec-core --test systemd_units_bdd
   ```

   Expected result: both commands pass, and the new tests fail before the
   implementation when pointed at deliberately invalid unit contents.

5. Update the documentation set and roadmap entry. Keep `docs/roadmap.md`
   unchecked until every implementation gate passes.

6. Run the full gates sequentially with logs:

   ```sh
   set -o pipefail && make fmt 2>&1 | tee /tmp/fmt-repovec-1-3-1-systemd-units.out
   set -o pipefail && make markdownlint 2>&1 | tee /tmp/markdownlint-repovec-1-3-1-systemd-units.out
   set -o pipefail && make nixie 2>&1 | tee /tmp/nixie-repovec-1-3-1-systemd-units.out
   set -o pipefail && make check-fmt 2>&1 | tee /tmp/check-fmt-repovec-1-3-1-systemd-units.out
   set -o pipefail && make lint 2>&1 | tee /tmp/lint-repovec-1-3-1-systemd-units.out
   set -o pipefail && make test 2>&1 | tee /tmp/test-repovec-1-3-1-systemd-units.out
   ```

   Expected result: every command exits `0`. `make lint` may print the
   repository's known message that `whitaker` is absent and skipped; that is
   acceptable only when the target still exits `0`.

7. Review and commit using the `commit-message` skill with `git commit -F` from
   a temporary message file. Do not use `git commit -m`.

## Validation and acceptance

The feature is accepted when the repository contains all of the following
observable evidence:

- `packaging/systemd/repovec.target` exists and wants `qdrant.service`,
  `repovecd.service`, `repovec-mcpd.service`, and `cloudflared.service`.
- `packaging/systemd/repovecd.service` requires and starts after
  `qdrant.service`.
- `packaging/systemd/repovec-mcpd.service` requires and starts after
  `qdrant.service` and `repovecd.service`.
- The checked-in systemd unit set is embedded and validated from
  `repovec-core`.
- `rstest` unit tests cover the happy path, each required missing dependency,
  the wrong Quadlet unit name, wrong service binary names, parse errors, and
  diagnostic snapshots.
- `rstest-bdd` scenarios describe the appliance unit contract in behavioural
  terms.
- `docs/repovec-appliance-technical-design.md`, `docs/users-guide.md`,
  `docs/developers-guide.md`, and `docs/roadmap.md` reflect the implemented
  behaviour.
- `make check-fmt`, `make lint`, and `make test` pass. The implementation
  should also pass `make fmt`, `make markdownlint`, and `make nixie` because
  documentation changes are expected.

Manual smoke validation on a disposable appliance host can provide additional
evidence:

```sh
sudo install -m 0644 packaging/systemd/repovec.target /etc/systemd/system/repovec.target
sudo install -m 0644 packaging/systemd/repovecd.service /etc/systemd/system/repovecd.service
sudo install -m 0644 packaging/systemd/repovec-mcpd.service /etc/systemd/system/repovec-mcpd.service
sudo systemctl daemon-reload
systemctl show -p Wants repovec.target
systemctl show -p Requires -p After repovecd.service
systemctl show -p Requires -p After repovec-mcpd.service
```

Expected output includes the required service names. Starting `repovec.target`
may still fail until later roadmap items create the `repovec` system user,
directory layout, Qdrant API-key configuration, and production daemon
implementations; those later prerequisites must not block static contract
acceptance for item `1.3.1`.

## Idempotence and recovery

All planned repository edits are additive or narrow documentation updates. If
formatting changes are too broad, inspect the diff and revert unrelated
formatter churn before committing. Do not use `git reset --hard` or
`git checkout --` unless the user explicitly asks.

The automated tests are static and should be safe to repeat. If a command fails
because the sandbox cannot write build artefacts or logs, re-run the same
command with elevated permissions using the command-execution tool rather than
working around the repository's shared Cargo cache.

If manual smoke validation installs files to `/etc/systemd/system`, use only a
disposable host. Remove the copied files and run `systemctl daemon-reload` to
return the host to its prior state.

## Artifacts and notes

Planning context was shared with the Wyvern team through context pack
`pk_duyt3wbs`, named `repovec-1-3-1-systemd-plan`.

The main systemd naming note to preserve in review is:

```plaintext
packaging/systemd/qdrant.container -> qdrant.service
```

Dependants must use `qdrant.service`.

## Interfaces and dependencies

No new external dependency is expected.

The new Rust interface should live under
`repovec_core::appliance::systemd_units`. The implementation may use a small
section-aware parser for systemd INI syntax. Keep it focused on validating the
checked-in static contract rather than becoming a general-purpose systemd
parser.

The systemd asset interface is the file set under `packaging/systemd/`:

- `qdrant.container`, already implemented by roadmap item `1.2.1`.
- `repovec.target`, added by this roadmap item.
- `repovecd.service`, added by this roadmap item.
- `repovec-mcpd.service`, added by this roadmap item.

The implementation should not call `systemctl` from library code. Future CLI or
installer adapters may copy these files and invoke systemd, but this roadmap
item is limited to the checked-in unit contract and static validation.

## Revision note

Initial DRAFT created for roadmap item `1.3.1`. The plan records the
pre-implementation approval gate, the intended systemd dependency graph, the
static validation strategy, the testing approach, documentation updates, and
the branch/PR workflow needed before implementation can proceed.
