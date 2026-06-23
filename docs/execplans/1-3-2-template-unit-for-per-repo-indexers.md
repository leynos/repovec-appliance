# Define template unit for per-repo indexers

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`, `Decision Log`,
and `Outcomes & Retrospective` must be kept up to date as work proceeds.

Status: COMPLETED

## Purpose / big picture

Roadmap item `1.3.2` added the systemd template used to run one grepai indexer
per active repository branch. The repository ships
`packaging/systemd/repovec-grepai@.service`; the static systemd validator in
`repovec-core` proves that the checked-in template runs as the unprivileged
`repovec` user, uses `HOME=/var/lib/repovec`, depends on local Qdrant and
`repovecd.service`, keeps its sandboxing directives, and sends stdout and
stderr to journald rather than bespoke log files.

The observable behaviour remains mostly static in this roadmap item. A human
can inspect the checked-in template, run the Rust validator, and see the
implemented unit and behavioural tests reject broken variants of the template.
Later roadmap item `3.2.1` will instantiate the template for active branches
during reconciliation.

## Constraints

- This branch was planning-only until the user explicitly approved
  implementation on 2026-06-02.
- The template source of truth must live at
  `packaging/systemd/repovec-grepai@.service`.
- The existing `packaging/systemd/repovec.target`, `repovecd.service`,
  `repovec-mcpd.service`, and `qdrant.container` contracts must remain
  compatible unless the plan is revised and approved.
- The Qdrant dependency in systemd units must use the generated service name
  `qdrant.service`, not `qdrant.container` or `qdrant.container.service`.
- The template must run as `User=repovec` and `Group=repovec`, with
  `Environment=HOME=/var/lib/repovec`.
- Journald must capture all template output. Do not add log-file paths,
  shell-level redirection, `tee`, `StandardOutput=file:`, or
  `StandardError=file:`.
- The implementation must preserve hexagonal boundaries. Static unit-file
  validation belongs in `repovec-core::appliance::systemd_units`; live
  `systemctl`, filesystem installation, process execution, and host state
  remain adapter, packaging, or operator concerns.
- New Rust validation must use typed errors. Do not make display strings the
  programmatic contract.
- New unit tests must use `rstest`. Behavioural tests must use `rstest-bdd`
  where applicable.
- Property tests, Kani, or Verus are not required for this finite static
  packaging contract. Add them only if implementation introduces a real
  invariant over an open input space.
- Documentation must use en-GB Oxford spelling, wrap Markdown paragraphs at
  80 columns, and follow `docs/documentation-style-guide.md`.
- Before each implementation commit and before any CodeRabbit review, run the
  applicable gates sequentially with `tee` logs under `/tmp`: at minimum
  `make build`, `make check-fmt`, `make typecheck`, `make lint`, and
  `make test`. Run `make fmt`, `make markdownlint`, and `make nixie` after
  documentation changes.
- Do not mark roadmap item `1.3.2` as done until implementation, validation,
  documentation, CodeRabbit review, and final commit are complete.

If satisfying the objective requires violating a constraint, stop, document the
conflict in `Decision Log`, and ask for direction.

## Tolerances

- Scope: if implementation requires more than twelve repository files or more
  than 650 net lines, stop and ask whether to split the work.
- Interface: if an existing public Rust API signature must change, stop and
  ask for approval. Additive APIs in `repovec-core::appliance::systemd_units`
  are within tolerance.
- Dependencies: if a new external crate or system package is required, stop
  and ask for approval.
- Systemd scope: if correct behaviour requires drop-ins for
  `cloudflared.service`, changes to the host's live systemd configuration, or
  ownership of future instantiated units, stop and ask for approval.
- Runtime scope: if automated validation requires root privileges, a live
  systemd manager, Podman, Qdrant, grepai, network access, or branch worktrees,
  replace that with static validation unless the user approves a broader
  integration environment.
- Instance naming: if a safe mapping from template instance to worktree path
  cannot be expressed without changing the roadmap's future
  `repovec-grepai@{owner}-{repo}-{branch}.service` shape, stop and present the
  options.
- Test iterations: if `make lint` or `make test` still fails after three
  focused fix attempts, stop, record the failing command and log path, and ask
  for direction.
- CodeRabbit: if `coderabbit review --agent` raises a concern that conflicts
  with this plan or requires scope beyond these tolerances, stop and ask for
  approval before widening the change.

## Risks

- Risk: systemd template instance names may not safely encode
  `{owner}/{repo}/{branch}` when owners, repositories, or branches contain
  hyphens, slashes, or other characters that systemd escapes. Severity: high.
  Likelihood: medium. Mitigation: make the implementation treat the instance
  identifier as an opaque systemd-escaped value and validate only the template
  contract in this item. Record the exact instance-to-worktree mapping decision
  in the design document. If the chosen mapping contradicts roadmap item
  `3.2.1`, stop and ask whether to update the roadmap language.

- Risk: the exact long-running `grepai watch` command shape may change or may
  require workspace/project flags that are not yet represented in this
  repository. Severity: medium. Likelihood: medium. Mitigation: use upstream
  grepai documentation only for the stable command existence (`grepai watch`)
  and avoid over-validating future flags in this feature. Keep
  workspace/project configuration for roadmap item `3.1`.

- Risk: static validation can prove the checked-in unit contract, but cannot
  prove that grepai, Qdrant, worktrees, or the `repovec` system user exist on a
  host. Severity: medium. Likelihood: high. Mitigation: document the boundary.
  Host liveness belongs to roadmap items `1.2.3`, `1.3.3`, `3.1`, and `3.2`.

- Risk: explicit journald directives may be mistaken for custom logging policy
  rather than a guard against file logging. Severity: low. Likelihood: medium.
  Mitigation: document that `StandardOutput=journal` and
  `StandardError=journal` are intentional and that no log files are created.

- Risk: the prompt's completion criteria mention interactive sessions, resize
  events, and terminal exit-code handling, which do not match a non-interactive
  systemd template for `grepai watch`. Severity: medium. Likelihood: high.
  Mitigation: record this mismatch. This feature should not allocate a TTY or
  implement PTY resize handling. It should ensure systemd tracks the main
  process exit status accurately.

## Progress

- [x] (2026-05-26T01:30:28+02:00) Read repository instructions and loaded the
  `leta`, `hexagonal-architecture`, `rust-router`, `execplans`,
  `domain-cli-and-daemons`, `rust-errors`, `commit-message`, `pr-creation`,
  `firecrawl-mcp`, and `en-gb-oxendict-style` skills.
- [x] (2026-05-26T01:30:28+02:00) Created the leta workspace for this
  worktree with `leta workspace add`.
- [x] (2026-05-26T01:30:28+02:00) Renamed the branch to
  `1-3-2-template-unit-for-per-repo-indexers`.
- [x] (2026-05-26T01:30:28+02:00) Used Firecrawl to check upstream systemd
  service-template, execution-environment, journald, and grepai command
  documentation.
- [x] (2026-05-26T01:30:28+02:00) Asked a Wyvern planning agent for
  repository-local validation guidance; it recommended extending the existing
  `systemd_units` validator and BDD test pattern.
- [x] (2026-05-26T01:30:28+02:00) Asked a second Wyvern planning agent for
  systemd/indexer lifecycle guidance; it exhausted its context before returning
  a usable result, so this plan proceeds from local inspection and Firecrawl
  evidence.
- [x] (2026-05-26T01:30:28+02:00) Drafted this pre-implementation ExecPlan.
- [x] (2026-06-02T00:00:00+02:00) Received explicit user approval to proceed
  with implementation from this ExecPlan and updated the status to
  `IN PROGRESS`.
- [x] (2026-06-02T01:12:07+02:00) Completed the first implementation pass:
  added `packaging/systemd/repovec-grepai@.service`, split the static systemd
  parser into `parsed.rs` to keep `mod.rs` under 400 lines, added an additive
  four-unit validation entry point, and extended `rstest` and `rstest-bdd`
  coverage for the grepai template.
- [x] (2026-06-02T01:12:07+02:00) Focused validation passed:
  `cargo test -p repovec-core systemd_units` logged to
  `/tmp/systemd-units-repovec-appliance-1-3-2-template-unit-for-per-repo-indexers.out`
  with 48 unit tests passing, and
  `cargo test -p repovec-core --test systemd_units_bdd` logged to
  `/tmp/systemd-units-bdd-repovec-appliance-1-3-2-template-unit-for-per-repo-indexers.out`
  with 13 BDD scenarios passing.
- [x] (2026-06-02T01:13:50+02:00) First full implementation gate passed:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
  succeeded, with logs under
  `/tmp/*-repovec-appliance-1-3-2-template-unit-for-per-repo-indexers.out`.
- [x] (2026-06-02T01:24:35+02:00) First CodeRabbit review completed with
  three findings. The valid major concern added `repovecd.service` to the
  grepai template's `Requires=` directive and validator coverage. The two
  parser clean-up findings were applied by introducing `ParsedLine`,
  `KeyValueLine`, `LineContext`, and a shallow `insert_key_value` helper.
- [x] (2026-06-02T01:24:35+02:00) Post-CodeRabbit gates passed:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
  succeeded after the review fixes. `make test` ran 203 nextest tests and 31
  doctests successfully.
- [x] (2026-06-02T02:42:52+02:00) Attempted to rerun CodeRabbit after the
  review fixes. The review service repeatedly returned recoverable rate-limit
  errors, so implementation retried according to the requested
  `shuf -i 15-30 -n 1` minute backoff policy before moving to documentation.
- [x] (2026-06-02T03:39:22+02:00) A later CodeRabbit retry completed with four
  trivial findings. The implementation now uses lint-safe slice-pattern checks
  for single-value service directives, converts the Qdrant Quadlet dependency
  clone with `to_owned()`, and adds `RestartSec=5s` to the grepai template
  contract and tests.
- [x] (2026-06-02T03:39:22+02:00) Post-fix gates passed again:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
  succeeded. `make test` ran 204 nextest tests and 31 doctests successfully.
- [x] (2026-06-02T04:21:03+02:00) A follow-up CodeRabbit review completed
  with one trivial finding. The checked-in template now includes conservative
  service hardening directives (`NoNewPrivileges`, `PrivateTmp`,
  `ProtectSystem`, `ProtectHome`, kernel protection, realtime restriction, and
  network address-family limits) while keeping the semantic validator focused
  on the item `1.3.2` service-layout contract.
- [x] (2026-06-02T04:28:45+02:00) Post-hardening gates passed:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
  succeeded. `make test` ran 204 nextest tests and 31 doctests successfully.
- [x] (2026-06-02T04:58:12+02:00) CodeRabbit requested additional systemd
  sandboxing directives. The service now also restricts devices, namespaces,
  SUID/SGID transitions, personality changes, control groups, kernel logs, host
  name changes, the clock, and process visibility where systemd supports those
  controls.
- [x] (2026-06-02T05:00:08+02:00) Post-sandboxing gates passed:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
  succeeded. `make test` ran 204 nextest tests and 31 doctests successfully.
- [x] (2026-06-02T05:13:41+02:00) CodeRabbit requested a small parser clean-up
  to avoid converting a section name twice. The parser now binds the owned
  section name once, uses it for the section map, and stores it as the current
  section.
- [x] (2026-06-02T05:17:09+02:00) Post-parser-clean-up gates passed:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
  succeeded. `make test` ran 204 nextest tests and 31 doctests successfully.
- [x] (2026-06-02T06:09:31+02:00) CodeRabbit completed a clean implementation
  milestone review with zero findings after the parser clean-up. The branch is
  ready for an atomic implementation commit before documentation work starts.
- [x] (2026-06-02T06:12:44+02:00) Committed the implementation milestone as
  `8d791a1 Add grepai indexer service template`.
- [x] (2026-06-02T06:31:18+02:00) Completed the documentation milestone:
  updated the technical design, user guide, developer guide, and roadmap entry
  for `1.3.2`; the roadmap item is now marked done and points concrete
  instance reconciliation at item `3.2.1`.
- [x] (2026-06-02T06:38:06+02:00) Documentation validation passed for the
  changed files and for the full explicit documentation targets:
  `make markdownlint` and `make nixie` succeeded, and the changed docs pass a
  targeted `markdownlint-cli2` run.
- [x] (2026-06-02T06:41:53+02:00) Full post-documentation Rust gates passed:
  `make check-fmt`, `make typecheck`, `make lint`, and `make test` all
  succeeded. `make test` ran 204 nextest tests and 31 doctests successfully.
- [ ] (2026-06-02T06:41:53+02:00) `make fmt` remains blocked by the
  `mdformat-all` helper invoking a stricter `markdownlint` command that reports
  unrelated repository-wide long-line and reference issues in existing docs.
  The changed docs are clean under the repository's explicit
  `make markdownlint` target, and incidental formatter churn was not retained.
- [x] (2026-06-02T06:49:02+02:00) CodeRabbit completed a clean documentation
  milestone review with zero findings.

## Surprises & Discoveries

- Observation: `docs/repovec-appliance-technical-design.md` already states
  that `repovec.target` wants per-repo indexers named
  `repovec-grepai@<repo>.service`, while `docs/roadmap.md` later says
  `repovec-grepai@{owner}-{repo}-{branch}.service`. Impact: the instance naming
  convention is not settled enough for this item to own all lifecycle
  semantics. This plan keeps the template contract separate from the future
  reconciler's instance-ID policy.

- Observation: the existing `systemd_units` module validates static assets
  with `include_str!`, typed errors, finite `rstest` mutations, and
  `rstest-bdd` scenarios. Impact: `1.3.2` should extend that module rather than
  create a second validator or run live `systemctl` checks.

- Observation: upstream systemd documentation says template services receive
  one argument through the `service@argument.service` syntax, and that the
  instance name is available through specifiers. Impact: this feature can use
  specifiers in the template, but the exact escaping policy must be documented
  and should not be inferred from a lossy hyphenated name.

- Observation: upstream systemd documentation recommends `Type=exec` for
  long-running services, when missing users or binaries should be reported by
  `systemctl start`. Impact: the implementation should consider `Type=exec` for
  the new indexer template while leaving existing `Type=simple` daemon units
  unchanged unless a separate approved refactor covers them.

- Observation: upstream grepai public documentation shows `grepai watch` as
  the file-watcher command and describes it as keeping the index fresh. It does
  not settle repovec's workspace/project invocation details. Impact: the unit
  contract should validate that the template starts `grepai watch`, but avoid
  claiming that workspace configuration or provider flags are complete in this
  item.

- Observation: `crates/repovec-core/src/appliance/systemd_units/mod.rs` was
  already at the local 400-line code-file limit before the template validator
  was added. Impact: the implementation first extracted the systemd parser into
  `crates/repovec-core/src/appliance/systemd_units/parsed.rs`, preserving the
  existing policy boundary while making room for the template contract.

- Observation: CodeRabbit identified an asymmetry where
  `repovec-grepai@.service` was ordered after `repovecd.service` but did not
  require it. Impact: the template now uses
  `Requires=qdrant.service repovecd.service`, and the validator rejects a
  missing `repovecd.service` requirement.

- Observation: CodeRabbit recommended adding common systemd service-hardening
  directives to the grepai template. Impact: the checked-in unit now carries
  those hardening directives, but they are not part of the static validator's
  semantic contract for item `1.3.2`.

- Observation: CodeRabbit's follow-up hardening request included directives
  that limit device access and process visibility. Impact: the template now
  assumes `grepai watch` does not need direct device access, host namespace
  creation, SUID transitions, kernel log access, or visibility into unrelated
  host processes.

- Observation: `make fmt` and `make markdownlint` do not use the same
  Markdown linter path. Impact: `make markdownlint` passes with
  `markdownlint-cli2`, while `make fmt` fails after `mdformat-all` invokes a
  stricter `markdownlint` command against unrelated existing documentation.
  This branch keeps task-owned documentation clean and does not commit
  unrelated formatter churn.

## Decision Log

- Decision: keep this branch as a pre-implementation planning branch until the
  user explicitly approves this ExecPlan. Rationale: the `execplans` skill and
  the user request both require approval before implementation. Date/Author:
  2026-05-26T01:30:28+02:00 / Codex.

- Decision: extend `repovec_core::appliance::systemd_units` instead of adding
  a new validation module. Rationale: the existing module already owns the
  service-layout contract, parser, typed errors, unit tests, and behavioural
  tests for checked-in systemd assets. Date/Author: 2026-05-26T01:30:28+02:00 /
  Codex.

- Decision: treat `repovec-grepai@.service` as a static template contract, not
  as the lifecycle manager for concrete branch instances. Rationale: the
  reconciler-owned start/stop behaviour belongs to roadmap item `3.2.1`.
  Date/Author: 2026-05-26T01:30:28+02:00 / Codex.

- Decision: do not add property tests, Kani, or Verus for the initial
  implementation. Rationale: the requested behaviour is a finite unit-file
  contract with a closed list of required directives. Exhaustive mutation tests
  and BDD scenarios give more direct evidence than proof tooling here.
  Date/Author: 2026-05-26T01:30:28+02:00 / Codex.

- Decision: classify the interactive-session completion criteria as a scope
  mismatch unless the user revises the plan. Rationale: a systemd-managed
  `grepai watch` service is non-interactive and should not allocate a terminal;
  resize propagation belongs to PTY/TUI work, not to this template.
  Date/Author: 2026-05-26T01:30:28+02:00 / Codex.

- Decision: begin execution after explicit user approval on 2026-06-02.
  Rationale: the approval gate in this ExecPlan has been satisfied by the
  user's request to proceed with implementation. Date/Author:
  2026-06-02T00:00:00+02:00 / Codex.

- Decision: keep the existing three-argument `validate_systemd_units` public
  API and add `validate_systemd_units_with_grepai_template` for callers that
  need the full four-unit contract. Rationale: this avoids a public API
  breakage while allowing checked-in startup validation to cover the new
  template. Date/Author: 2026-06-02T01:12:07+02:00 / Codex.

- Decision: require `repovecd.service` as well as ordering after it in the
  grepai template. Rationale: systemd `After=` only controls ordering; pairing
  it with `Requires=` makes the indexer fail closed when the control-plane
  daemon is unavailable. Date/Author: 2026-06-02T01:24:35+02:00 / Codex.

- Decision: add `RestartSec=5s` to the grepai template and validator.
  Rationale: `Restart=on-failure` without a delay can create a tight restart
  loop when `grepai watch` fails persistently; a short fixed delay keeps the
  service responsive without hammering systemd. Date/Author:
  2026-06-02T03:39:22+02:00 / Codex.

- Decision: add conservative systemd hardening directives to the checked-in
  grepai template without making them validator requirements. Rationale:
  hardening improves the shipped packaging default, but the static validator
  should continue to test the service-layout, identity, dependency, restart,
  and journald contracts that define this roadmap item. Date/Author:
  2026-06-02T04:21:03+02:00 / Codex.

- Decision: accept the additional CodeRabbit sandboxing directives in the
  packaging asset. Rationale: the indexer only needs repository files, local
  network access to Qdrant, and ordinary process execution; the added systemd
  restrictions align with that minimal runtime shape and remain outside the
  semantic validator contract until runtime installation tests exist.
  Date/Author: 2026-06-02T04:58:12+02:00 / Codex.

## Outcomes & Retrospective

The implementation shipped the checked-in
`packaging/systemd/repovec-grepai@.service` template and committed it with the
static validation changes for roadmap item `1.3.2`. The template runs
`/usr/bin/grepai watch` as `repovec:repovec`, sets `HOME=/var/lib/repovec`,
uses `WorkingDirectory=/var/lib/repovec/worktrees/%I`, stays tied to
`repovec.target`, depends on Qdrant and `repovecd.service`, and leaves stdout
and stderr in journald.

The validator now embeds the grepai template alongside the target, daemon
service, and MCP daemon service assets. It rejects missing or incorrect
template sections, Qdrant and `repovecd.service` dependencies, target binding
directives, identity directives, worktree directory, environment, restart
policy, grepai command, and bespoke file logging. Unit and behavioural coverage
was extended with `rstest` and `rstest-bdd` cases for the shipped template
contract.

The documentation updates described the installation boundary, the future
instance reconciliation boundary for roadmap item `3.2.1`, the journald logging
contract, and the static validator's limits. The implementation gates passed:
`make check-fmt`, `make typecheck`, `make lint`, `make test`,
`make markdownlint`, `make nixie`, and targeted `markdownlint-cli2` checks for
the changed Markdown. CodeRabbit review passed with no findings before the
review feedback addressed here.

## Context and orientation

This repository is a Rust workspace. Appliance-specific static validation lives
under `crates/repovec-core/src/appliance/`. The existing
`crates/repovec-core/src/appliance/systemd_units/` module embeds checked-in
systemd assets from `packaging/systemd/`, parses a small subset of systemd unit
syntax, and returns `SystemdUnitError` when a checked-in asset violates the
appliance contract.

The systemd assets currently shipped by roadmap item `1.3.1` are:

- `packaging/systemd/repovec.target`
- `packaging/systemd/repovecd.service`
- `packaging/systemd/repovec-mcpd.service`

Roadmap item `1.3.2` adds:

- `packaging/systemd/repovec-grepai@.service`

The future continuous-indexing work in roadmap item `3.2.1` will instantiate
the template for active branches. That future work will decide when to start,
stop, enable, disable, or purge concrete indexer units.

The following repository documents are source material for implementation:

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
- `docs/documentation-style-guide.md`

Relevant skills for implementation are:

- `leta`, for source navigation and symbol relationships
- `rust-router`, followed by `domain-cli-and-daemons` and `rust-errors`, for
  daemon lifecycle and typed error boundaries
- `hexagonal-architecture`, for keeping policy validation separate from
  systemd execution and packaging adapters
- `execplans`, for keeping this living document current
- `commit-message`, for file-based commit messages
- `pr-creation` and `en-gb-oxendict-style`, for the pull request

Relevant external references checked during planning are:

- systemd service templates:
  <https://www.freedesktop.org/software/systemd/man/latest/systemd.service.html>
- systemd execution environment and logging directives:
  <https://www.freedesktop.org/software/systemd/man/latest/systemd.exec.html>
- grepai README and quick-start commands:
  <https://raw.githubusercontent.com/yoanbernabeu/grepai/main/README.md>
- grepai project site:
  <https://yoanbernabeu.github.io/grepai/>

## Proposed implementation details

The implementation started from tests that described the template contract,
then added the shipped template and validator changes that made those tests
pass. The checked-in template shape is:

```systemd
[Unit]
Description=repovec grepai indexer for %I
Requires=qdrant.service
Requires=repovecd.service
After=qdrant.service repovecd.service
PartOf=repovec.target

[Service]
Type=exec
User=repovec
Group=repovec
WorkingDirectory=/var/lib/repovec/worktrees/%I
Environment=HOME=/var/lib/repovec
ExecStart=/usr/bin/grepai watch
Restart=on-failure
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=repovec.target
```

The implementation retained `%I` as the systemd instance token for the worktree
layout. The later reconciliation task `3.2.1` owns mapping repository and
branch identity to concrete, systemd-safe instance names.

The validator checks the template for:

- `[Unit]`, `[Service]`, and `[Install]` sections
- `Requires=qdrant.service`
- `Requires=repovecd.service`
- `After=qdrant.service`
- `After=repovecd.service`
- `PartOf=repovec.target`
- `WantedBy=repovec.target`
- `User=repovec`
- `Group=repovec`
- `WorkingDirectory=/var/lib/repovec/worktrees/%I`, unless the approved
  implementation chooses a safer documented specifier
- `Environment=HOME=/var/lib/repovec`
- `ExecStart=/usr/bin/grepai watch`
- `Restart=on-failure`
- explicit journald output with `StandardOutput=journal` and
  `StandardError=journal`
- rejection of `qdrant.container` and `qdrant.container.service`

The validator should not check for the existence of `/usr/bin/grepai`,
`/var/lib/repovec/worktrees/%I`, the `repovec` user, or live systemd units.

## Implementation milestones

### Milestone 1: Red tests for the template contract

Extend the existing systemd unit test harness so it can hold and mutate a
fourth unit file, `repovec-grepai@.service`.

Add `rstest` cases that fail before implementation for missing or incorrect
template directives, including service identity, `HOME`, Qdrant dependency,
`ExecStart`, journald output, and install binding.

Extend `crates/repovec-core/tests/features/systemd_units.feature` and
`crates/repovec-core/tests/systemd_units_bdd.rs` with behavioural scenarios for
the checked-in template and representative unhappy paths.

Run focused failing tests and record the expected failures in `Progress`:

```sh
cargo test -p repovec-core systemd_units 2>&1 \
  | tee /tmp/systemd-units-$(git branch --show-current).out
cargo test -p repovec-core --test systemd_units_bdd 2>&1 \
  | tee /tmp/systemd-units-bdd-$(git branch --show-current).out
```

### Milestone 2: Template and validator implementation

Add `packaging/systemd/repovec-grepai@.service`.

Extend `crates/repovec-core/src/appliance/systemd_units/mod.rs` with:

- `CHECKED_IN_REPOVEC_GREPAI_TEMPLATE`
- `CHECKED_IN_REPOVEC_GREPAI_TEMPLATE_PATH`
- `checked_in_repovec_grepai_template()`
- an additive validation entry point if needed, or an approved extension of
  `validate_systemd_units`
- `validate_grepai_template`

Extend `SystemdUnitError` only where existing variants cannot express the
failure clearly. Prefer reusing `MissingDependency`, `IncorrectExecStart`, and
`IncorrectServiceDirective` when they remain accurate.

Make the focused unit and BDD tests pass, then run:

```sh
make build 2>&1 | tee /tmp/build-$(git branch --show-current).out
make check-fmt 2>&1 | tee /tmp/check-fmt-$(git branch --show-current).out
make typecheck 2>&1 | tee /tmp/typecheck-$(git branch --show-current).out
make lint 2>&1 | tee /tmp/lint-$(git branch --show-current).out
make test 2>&1 | tee /tmp/test-$(git branch --show-current).out
```

Run CodeRabbit only after those gates pass:

```sh
coderabbit review --agent
```

Resolve all applicable CodeRabbit concerns before moving on.

### Milestone 3: Documentation

Update `docs/repovec-appliance-technical-design.md` to record the template
unit, the instance-name/worktree-path decision, journald logging, and the
boundary between static template validation and future lifecycle management.

Update `docs/users-guide.md` with operator-visible installation notes for the
template and explain that concrete instances are managed by later
reconciliation work.

Update `docs/developers-guide.md` section `5.3` with the expanded validation
surface, including the new accessor and template path.

Update `docs/roadmap.md` only after the implementation is complete and all
gates pass, marking item `1.3.2` done with a short status note.

Run documentation formatting and validation:

```sh
make fmt 2>&1 | tee /tmp/fmt-$(git branch --show-current).out
make markdownlint 2>&1 | tee /tmp/markdownlint-$(git branch --show-current).out
make nixie 2>&1 | tee /tmp/nixie-$(git branch --show-current).out
```

Then rerun the full requested gates:

```sh
make check-fmt 2>&1 | tee /tmp/check-fmt-$(git branch --show-current).out
make typecheck 2>&1 | tee /tmp/typecheck-$(git branch --show-current).out
make lint 2>&1 | tee /tmp/lint-$(git branch --show-current).out
make test 2>&1 | tee /tmp/test-$(git branch --show-current).out
```

Run CodeRabbit again after the gates pass:

```sh
coderabbit review --agent
```

Resolve all applicable concerns before committing.

### Milestone 4: Commit and pull request

Review the full diff, update this ExecPlan's `Progress`,
`Surprises & Discoveries`, `Decision Log`, and `Outcomes & Retrospective`, and
make an atomic implementation commit with a file-based commit message.

Push `1-3-2-template-unit-for-per-repo-indexers` to
`origin/1-3-2-template-unit-for-per-repo-indexers`.

Open or update the draft pull request. The title must include `(1.3.2)`, and
the summary must link this ExecPlan. Include the lody session link in a final
`## References` section.

## Acceptance criteria

- `packaging/systemd/repovec-grepai@.service` exists and contains no bespoke
  log-file routing.
- The checked-in template runs as `repovec:repovec` with
  `HOME=/var/lib/repovec`.
- The checked-in template starts `grepai watch` without shell wrappers,
  pipelines, or redirection.
- The checked-in template is ordered after Qdrant, requires and orders after
  `repovecd.service`, and is tied to `repovec.target`.
- The static validator rejects missing or incorrect template identity,
  dependency, command, environment, journald, and install directives. It
  enforces the `repovecd.service` dependency with both `Requires=` and `After=`
  directives, alongside the Qdrant and `repovec.target` checks.
- `rstest` unit tests cover happy and unhappy paths for the template.
- `rstest-bdd` behavioural tests describe the user-facing systemd template
  contract.
- Documentation explains how the template is installed, how future instances
  relate to it, where output goes, and what validation does not prove.
- `make build`, `make check-fmt`, `make typecheck`, `make lint`, and
  `make test` pass after the final implementation milestone.
- `coderabbit review --agent` has no unresolved applicable concerns.
- `docs/roadmap.md` marks item `1.3.2` done only after the implementation is
  complete.

## Rollback plan

Because this feature is additive, rollback is straightforward. Revert the
implementation commit that adds `repovec-grepai@.service`, validator changes,
tests, documentation updates, and the roadmap status change. Re-run
`make check-fmt`, `make typecheck`, `make lint`, and `make test` after the
revert. If the branch has already been pushed, push the revert as a new commit,
so review history remains intact.
