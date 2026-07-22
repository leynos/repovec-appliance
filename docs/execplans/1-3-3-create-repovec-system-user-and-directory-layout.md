# Create the `repovec` system user and directory layout (1.3.3)

This ExecPlan (execution plan) is a living document. The sections
`Constraints`, `Tolerances`, `Risks`, `Progress`, `Surprises & Discoveries`,
`Decision Log`, and `Outcomes & Retrospective` must be kept up to date as work
proceeds.

Status: DRAFT

## Purpose / big picture

Roadmap item 1.3.3 (`docs/roadmap.md`, lines 116-121) requires the appliance to
own its on-disk foundation: the `repovec` system user, that user's home
directory `/var/lib/repovec`, the data subdirectories `git-mirrors/`,
`worktrees/`, and `.grepai/`, and the secrets directory `/etc/repovec/` with
restricted permissions. The success criterion is that
`systemctl start repovec.target` succeeds and Qdrant becomes reachable.

After this change an operator installing the appliance packaging assets on a
systemd host obtains, deterministically and without a reboot, a `repovec`
service account and a correctly owned, tightly permissioned directory tree. The
daemons (`repovecd`, `repovec-mcpd`) whose units already declare
`WorkingDirectory=/var/lib/repovec` and `HOME=/var/lib/repovec` can start
because that directory now exists; the Qdrant container can mount
`/var/lib/repovec/qdrant-storage`; and Qdrant answers an authenticated gRPC
liveness probe on loopback.

The observable outcome a reviewer can check:

1. The repository ships a checked-in `systemd-tmpfiles` asset that declares the
   `/var/lib/repovec` tree with the exact owner and mode of each directory, an
   install-time provisioning unit that applies the `sysusers.d` and `tmpfiles.d`
   assets so the tree exists whether or not the host has rebooted, and the
   existing `systemd-sysusers` asset that declares the user.
2. A new pure-Rust validator in `repovec-core` proves that the checked-in
   `tmpfiles.d` and `sysusers.d` assets satisfy a per-directory contract, with a
   typed error enum whose operator-facing `Display` messages are under snapshot
   control, plus a bidirectional drift guard so the asset and the contract
   cannot diverge silently.
3. A minimal live-ownership pre-flight in the daemons fails closed if the real
   on-disk tree is misowned or over-permissive, because a static text validator
   cannot detect a misprovisioned host.
4. The opt-in privileged integration harness demonstrates the success criterion
   on a real systemd host under SELinux enforcing: after the assets are applied,
   the tree exists with the expected ownership and modes, `qdrant.service`
   reaches `active`, and Qdrant answers on loopback gRPC.

This plan is in DRAFT. Do not begin implementation until the user approves it.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

- The `directory_layout` validator added to `repovec-core` must remain pure and
  I/O-free. It may only parse embedded asset text (`include_str!`) and compare it
  against an in-memory contract. It must not call `systemctl`,
  `systemd-tmpfiles`, `systemd-sysusers`, `useradd`, `podman`, read
  `/etc/passwd`, or touch the live filesystem. The separate live-ownership
  pre-flight (Milestone 4) is an explicitly carved-out adapter, not part of the
  pure validator.
- Security invariant SI-1 (data-tree confidentiality). Every directory in the
  `/var/lib/repovec` tree — the data root, `git-mirrors/`, `worktrees/`, and
  `.grepai/` — must be owned `repovec:repovec` with mode `0700` (no group or
  other access). Contents are private third-party source and derived embeddings;
  the sole legitimate reader is the `repovec` service account, which holds owner
  access.
- Security invariant SI-2 (qdrant-storage isolation). `/var/lib/repovec/qdrant-storage`
  must be owned `root:root` with mode `0700`. Qdrant runs under rootful Podman as
  uid 0 and is the only filesystem accessor; the `repovec` daemons reach Qdrant
  only over loopback gRPC and must not have filesystem access to the vector
  store.
- Security invariant SI-3 (secrets-dir authority). `/etc/repovec/` must be owned
  `root`, group `repovec`, mode `0750` (non-world, non-group-write). Root
  ownership of the directory is mandatory so `repovec` can read but cannot
  `chmod`/`chown` it or replace its secret files. Its secret files remain
  `repovec:repovec` mode `0400`. These are established by task 1.2.2's
  `packaging/libexec/repovec-qdrant-api-key` and asserted by `integration-tests/`;
  this plan must not weaken them.
- Security invariant SI-4 (single authority for `/etc/repovec`). Exactly one
  mechanism provisions `/etc/repovec`. The libexec helper remains authoritative;
  the new `tmpfiles.d` asset must not declare `/etc/repovec`, and the validator
  must reject a `tmpfiles.d` asset that does.
- Security invariant SI-5 (explicit modes). The `tmpfiles.d` validator must
  reject the `-` default token for mode, owner, or group on any managed
  directory, because the `systemd-tmpfiles` `d` default mode is world-readable
  `0755`.
- `RuntimePaths` in `crates/repovec-core/src/lib.rs` is the single source of
  truth for appliance *paths*. New path accessors go on `RuntimePaths` (with a
  runnable `# Examples` doctest, matching the existing accessors). The
  *policy* (which directories are required and how each is owned and
  permissioned) lives in the `directory_layout` module as a spec table, not on
  `RuntimePaths`, which must stay a pure path type free of modes and owners.
- The existing `packaging/sysusers.d/repovec.conf` declaration
  (`u repovec - "repovec appliance service user" /var/lib/repovec /usr/sbin/nologin`)
  must not be broken. The `repovec` user's home must stay `/var/lib/repovec` and
  its shell `/usr/sbin/nologin`.
- No numeric uid/gid may be pinned. The design defers allocation to
  `systemd-sysusers`; the validator must not assert a fixed numeric id, and
  ownership must be checked by name.
- Do not modify unrelated appliance modules
  (`appliance::qdrant_quadlet`, `appliance::qdrant_liveness`,
  `appliance::systemd_units`) beyond, at most, a single additive re-export line
  in `crates/repovec-core/src/appliance/mod.rs`.
- All commit gates (`make check-fmt`, `make typecheck`, `make lint`,
  `make test`, and `make validate-systemd`) must pass before each CodeRabbit
  review and before each commit.

## Tolerances (exception triggers)

Thresholds that trigger escalation when breached. These bound autonomous action;
they are not quality targets.

- Scope: if the implementation requires touching more than 24 non-snapshot files,
  or more than roughly 1000 net lines of code and asset text (excluding committed
  `.snap` snapshot files, generated fixtures, and prose documentation), stop and
  escalate. Committed `.snap` files count against neither the line nor the file
  budget; the plan prefers inline snapshots to keep the file count low.
- Interface: if any existing public API in `repovec-core` must change signature
  (as opposed to additive new functions), stop and escalate.
- Dependencies: if a new external crate dependency beyond those already in
  `[workspace.dependencies]` is required, stop and escalate. Adding
  `tempfile.workspace = true` to `crates/repovec-core` `[dev-dependencies]`, if
  needed for a filesystem-touching test, is pre-approved and does not count.
- Iterations: if a focused test still fails after 4 fix attempts, stop and
  record the blocker in `Decision Log`.
- Ambiguity: if the SELinux relabel interaction (Decision D-3) or the live
  pre-flight scope (Decision D-6) proves to materially change the contract, stop
  and present options.
- CodeRabbit: if `coderabbit review --agent` raises a concern that conflicts
  with this plan or requires scope beyond these tolerances, stop and ask for
  approval before acting on it.

## Risks

- Risk R-1: install-time provisioning. `systemd-tmpfiles` and `systemd-sysusers`
  process new snippets at boot; on a freshly installed but un-rebooted host they
  have not run, so `/var/lib/repovec` would not exist and
  `systemctl start repovec.target` would fail. Severity: high. Likelihood: high
  without mitigation.
  Mitigation: ship `packaging/systemd/repovec-provision.service`, a oneshot that
  runs `systemd-sysusers` then `systemd-tmpfiles --create` for the repovec
  assets, ordered `Before=` the daemons and `qdrant.service` and
  `WantedBy=repovec.target`, so the tree is provisioned when the target starts
  regardless of reboot state (Decision D-7). If the `repovec` user does not exist
  when tmpfiles runs, tmpfiles skips the chown and leaves the directory
  root-owned; the live pre-flight (Milestone 4, R-6 mitigation) detects this and
  fails closed.
- Risk R-2: boot ordering for the chown-by-name. The `tmpfiles.d` asset chowns to
  `repovec:repovec`, which requires the user to exist first. Severity: medium.
  Likelihood: low.
  Mitigation: verified on a Rocky 10 host that `systemd-tmpfiles-setup.service`
  ships `After=systemd-sysusers.service`, and `repovec-provision.service` runs
  `systemd-sysusers` before `systemd-tmpfiles --create` in the same unit. State
  "sysusers before tmpfiles" as an explicit packaging and image-build (roadmap
  6.3.1) requirement, not merely a stock-systemd fact. The integration test
  asserts ownership resolves to the `repovec` user by name.
- Risk R-3: grepai indexer sandbox. The `repovec-grepai@.service` template (task
  1.3.2) applies `ProtectSystem=full` and `ProtectHome=read-only`. Severity: low.
  Likelihood: low.
  Analysis (to be confirmed in Stage A): `ProtectHome=read-only` affects only
  `/home`, `/root`, and `/run/user`; it does not touch `/var/lib/repovec`.
  `ProtectSystem=full` makes `/usr`, `/boot`, and `/etc` read-only but leaves
  `/var` writable. Therefore the indexer can write `worktrees/%I` and `.grepai/`
  today and there is no conflict with this plan. The only latent risk is a future
  hardening to `ProtectSystem=strict`, which would make `/var` read-only and then
  require `ReadWritePaths=/var/lib/repovec/worktrees/%I /var/lib/repovec/.grepai`
  on the template. Record this as a note for 1.3.2 / 3.2.1; do not expand this
  plan. (An earlier draft misattributed the risk to `ProtectHome`; corrected
  after the expert review.)
- Risk R-4: the privileged integration harness requires rootful nested Podman and
  systemd-as-PID-1 inside a container under SELinux enforcing, which CI runners
  typically cannot grant. Severity: low. Likelihood: high (expected).
  Mitigation: keep the E2E assertions in the opt-in `integration-tests/` suite
  (not part of `make test`), mirroring the 1.2.2 precedent; verify systemd is
  PID 1 as a first step; provide the exact command to run on a capable host.
- Risk R-5: SELinux relabel churn on `qdrant-storage`. `systemd-tmpfiles` applies
  the default `/var/lib` file context, while the Quadlet's `:Z` relabels the bind
  source to a private MCS `container_file_t` label at container start. Severity:
  low. Likelihood: medium.
  Mitigation: this is self-healing because `:Z` re-runs on each container start
  (Decision D-3). The integration test runs under SELinux enforcing so it proves
  Qdrant actually writes; otherwise the E2E proves nothing about the Rocky 10
  target.
- Risk R-6: static validation cannot detect live misprovisioning. A correct
  asset does not guarantee a correct host (a user-missing race, tampering, a
  backup restore with wrong modes, or a pre-created misowned child, since `d`
  does not recurse). Severity: high for a private-data appliance. Likelihood:
  low per event.
  Mitigation: the live pre-flight adapter (Milestone 4) stats the tree at daemon
  startup and refuses to start on any ownership or confidentiality mismatch.
- Risk R-7: the tmpfiles/sysusers parser is more permissive than the real
  `systemd-tmpfiles`/`systemd-sysusers` parsers, and the sysusers GECOS field is
  quoted and contains spaces. Severity: medium. Likelihood: medium.
  Mitigation: the tokenizer honours the quoted GECOS field so column indices are
  correct; the tmpfiles view accepts only the `d` type this asset uses and
  rejects anything else with `MalformedLine`; a robustness property test asserts
  the tokenizer never panics and never returns a partially populated entry.

## Progress

- [ ] Stage A: approval gate — this DRAFT is presented and approved (no code
  changes until then). Includes re-reading the grepai template to confirm R-3.
- [ ] Milestone 1: red tests and feature specification (permissive stub).
- [ ] Milestone 2: packaging assets and pure validator (green).
- [ ] Milestone 3: end-to-end integration assertions.
- [ ] Milestone 4: live-ownership pre-flight (fail-closed guard).
- [ ] Milestone 5: documentation (developers guide, users guide, ADR) and
  roadmap update.
- [ ] Milestone 6: commit, push, and mark roadmap item done.

Timestamps will be added as each item completes.

## Surprises & Discoveries

- Observation: `crates/repovec-core/src/lib.rs` already defines `RuntimePaths`
  with `git_mirrors_root()`, `worktrees_root()`, `grepai_root()`, and
  `github_oauth_token_credential()`, each with a doctest, but it has no consumers
  anywhere in the workspace.
  Evidence: recon grep across the workspace found references only within
  `lib.rs`.
  Impact: the validator becomes the first real consumer of `RuntimePaths`.
- Observation: `qdrant_quadlet::validate_checked_in_qdrant_quadlet` is not called
  by `repovec-ci systemd-gate` or by daemon startup; only `systemd_units` is.
  Evidence: expert review of `crates/repovec-ci` and `appliance::daemon_startup`.
  Impact: the plan must not claim `make validate-systemd` already validates
  every appliance contract; it must explicitly extend the gate to call
  `validate_checked_in_directory_layout()`.
- Observation: `/etc/repovec/` (mode `0750`, `root:repovec`) and the
  `0400 repovec:repovec` secret files already exist and are integration-tested
  from task 1.2.2.
  Evidence: `packaging/libexec/repovec-qdrant-api-key` lines 64, 72, 101-102 and
  `integration-tests/provisioning/test_qdrant_api_key.py`.
  Impact: 1.3.3 keeps `/etc/repovec` helper-authoritative (SI-4) and does not
  re-provision it.

## Decision Log

- Decision D-1: create the `/var/lib/repovec` tree with a checked-in
  `packaging/tmpfiles.d/repovec.conf` processed by `systemd-tmpfiles`, paired
  with the existing `packaging/sysusers.d/repovec.conf`.
  Rationale: `systemd-sysusers` creates the passwd/group entry but never the home
  directory; `tmpfiles.d` is the freedesktop-idiomatic and Debian-policy endorsed
  mechanism for creating directories below `/var`, is declarative and idempotent,
  and matches the appliance's declarative-asset style.
  Date/Author: 2026-07-22, planning agent.
- Decision D-7: also ship `packaging/systemd/repovec-provision.service`, a
  oneshot that applies the `sysusers.d` and `tmpfiles.d` assets at target start.
  Rationale: boot-time `systemd-tmpfiles-setup` has not run on a freshly
  installed, un-rebooted host, so without an install-time trigger
  `systemctl start repovec.target` fails its own success criterion. The oneshot
  runs `systemd-sysusers <asset>` then `systemd-tmpfiles --create <asset>`
  (sysusers before tmpfiles), ordered `Before=repovec-qdrant-api-key.service
  qdrant.service repovecd.service repovec-mcpd.service` and
  `WantedBy=repovec.target`, mirroring the existing `repovec-qdrant-api-key.service`
  oneshot pattern. Image builds (6.3.1) must run the same ordered pair.
  Date/Author: 2026-07-22, planning agent (from systemd expert review P0-1).
- Decision D-2: add a new flat validator module
  `crates/repovec-core/src/appliance/directory_layout/` following the
  `parser` + `error` + `validate_*` + `include_str!` shape of `systemd_units`.
  No observer port (consistent with `systemd_units` and the explicit guidance in
  `qdrant_quadlet/observer.rs` not to copy that port). Hand-roll `Display` and
  `impl Error`; do not use `thiserror` (no existing appliance error does).
  Date/Author: 2026-07-22, planning agent.
- Decision D-8: the validator's error fields are typed to match house
  convention: a `Mode(u16)` newtype whose `Display` is `{:04o}`, `Utf8PathBuf`
  for paths, `&'static str` for expected (contract) values, `String` for observed
  values, and an `asset: &'static str` discriminator with a `.asset()` accessor
  (the analogue of `SystemdUnitError::unit()`). Mode mismatches funnel through a
  single `IncorrectMode` variant; there are no separate world-writable or
  secrets-mode variants, because exact-match against the per-directory expected
  mode already rejects any loosening (a `0700` expectation rejects any group or
  other bit; a `0750` expectation on `/etc/repovec` rejects `0755`).
  Date/Author: 2026-07-22, planning agent (from Rust/hexagonal review P0-1/P1-1).
- Decision D-9: the mode/owner policy is heterogeneous, so it is expressed once
  as a per-entry spec table `layout_contract(&RuntimePaths) -> [DirectorySpec; N]`
  inside `directory_layout` (fields: path, mode, owner, group). `RuntimePaths`
  gains only the pure path accessor `qdrant_storage_root()` (with a doctest).
  Data tree and `qdrant-storage` are `0700`; the config root, if the validator
  references it, is `0750 root:repovec`.
  Date/Author: 2026-07-22, planning agent (from Rust/hexagonal review P0-2).
- Decision D-4 (resolved): `/var/lib/repovec/qdrant-storage` is `root:root` mode
  `0700`. The official `docker.io/qdrant/qdrant:v1` image sets no `USER` and runs
  as uid 0; under rootful Podman that maps to host root, so the bind source must
  be root-writable; `repovec:repovec` would break Qdrant writes. `:Z` handles
  SELinux labelling, which is orthogonal to DAC ownership. Root traverses the
  `0700 repovec` parent via the DAC override. Dependency: valid only while Qdrant
  runs rootful with no userns remap; if `UserNS=auto`, rootless, or `User=` is
  ever added to `qdrant.container`, this ownership must change.
  Date/Author: 2026-07-22, planning agent (from systemd P1-1 and security P0-1).
- Decision D-3 (resolved): keep `qdrant-storage` in the `tmpfiles.d` asset for an
  explicit mode/owner, and accept that the Quadlet's `:Z` relabels it on each
  container start (self-healing). The integration test runs under SELinux
  enforcing to prove Qdrant writes succeed.
  Date/Author: 2026-07-22, planning agent (from systemd P0-3).
- Decision D-5: reuse `RuntimePaths` accessors as the *path* source for the spec
  table; do not add mode/owner policy to `RuntimePaths`.
  Date/Author: 2026-07-22, planning agent.
- Decision D-6: include a minimal live-ownership pre-flight as Milestone 4, in a
  sibling adapter (for example `appliance::directory_layout::live` or a
  `RuntimePaths::verify_live()` helper) wired into `repovecd`/`repovec-mcpd`
  startup after systemd-unit validation. It stats the data tree and `/etc/repovec`
  and refuses to start on any owner/mode mismatch. Rationale: for a private-data
  appliance the static text contract is necessary but not sufficient (R-6). If
  the pre-flight would breach the file/LOC tolerance, split it into a follow-up
  roadmap item and record that here rather than dropping it silently.
  Date/Author: 2026-07-22, planning agent (from security review P1-6).
- Decision D-10: extend `repovec-ci systemd-gate` (`run_systemd_gate`) to also
  call `validate_checked_in_directory_layout()`, and correct the Context claim
  about what `make validate-systemd` currently checks, so the packaging contract
  the Constraints depend on is genuinely gated in CI.
  Date/Author: 2026-07-22, planning agent (from Rust/hexagonal review P1-4).

## Outcomes & Retrospective

To be completed at milestones and at completion. Compare the delivered contract,
modes, and the live pre-flight against this purpose statement; capture any
deviation and any follow-up filed for the grepai sandbox (R-3) or a deferred
live guard (D-6).

## Context and orientation

The reader is assumed to know nothing about this repository. The repovec
appliance is a Rust workspace that turns private GitHub repositories into a
continuously indexed, MCP-queryable corpus. It ships packaging assets (systemd
units, Podman Quadlets, a `sysusers.d` declaration, and a libexec provisioning
helper) under `packaging/`, and a shared library crate `repovec-core` that
statically validates those checked-in assets so a broken contract fails a build
gate rather than a production host.

Key terms:

- **`systemd-sysusers` / `sysusers.d`**: reconstructs `/etc/passwd` and
  `/etc/group` from declarative snippets. A `u` line declares a user. It creates
  the account record but not the home directory.
- **`systemd-tmpfiles` / `tmpfiles.d`**: creates and adjusts files and
  directories from declarative snippets. A `d` line means "create this directory
  with this mode, user, and group, and adjust an existing one to match"; it does
  not recurse into existing contents. The default mode when the column is `-` is
  world-readable `0755`, so the appliance always sets it explicitly. `D` (empty
  on boot) must never be used for persistent data.
- **checked-in asset**: a file under `packaging/` embedded into `repovec-core` at
  compile time with `include_str!` and validated by a pure function.
- **`RuntimePaths`**: the pure path type in `crates/repovec-core/src/lib.rs`
  naming the config root (`/etc/repovec`) and data root (`/var/lib/repovec`) and
  deriving child paths; each accessor has a runnable doctest.
- **GECOS**: the human-readable comment field of a passwd/sysusers entry; in the
  repovec declaration it is quoted and contains spaces
  (`"repovec appliance service user"`), so a naive whitespace split would
  miscount columns.

Current state relevant to this task:

- `packaging/sysusers.d/repovec.conf` declares the `repovec` user with home
  `/var/lib/repovec` and shell `/usr/sbin/nologin`; it is not validated from Rust
  yet.
- `/etc/repovec/` is created (mode `0750`, `root:repovec`) by
  `packaging/libexec/repovec-qdrant-api-key` (task 1.2.2), which also writes the
  `0400 repovec:repovec` Qdrant API-key file. This is exercised by the opt-in
  Python harness under `integration-tests/`.
- No `packaging/tmpfiles.d/` directory exists. Nothing on disk creates
  `/var/lib/repovec`, `git-mirrors/`, `worktrees/`, `.grepai/`, or
  `qdrant-storage/`. This is the gap 1.3.3 closes.
- `packaging/systemd/repovecd.service` and `repovec-mcpd.service` declare
  `User=repovec`, `Group=repovec`, `WorkingDirectory=/var/lib/repovec`, and
  `Environment=HOME=/var/lib/repovec`. `packaging/systemd/repovec-grepai@.service`
  declares `WorkingDirectory=/var/lib/repovec/worktrees/%I` and the sandbox
  directives discussed in R-3. All fail to start until the directories exist.
- `packaging/systemd/qdrant.container` bind-mounts
  `/var/lib/repovec/qdrant-storage:/qdrant/storage:Z`.
- Appliance validators live under `crates/repovec-core/src/appliance/`, one
  submodule per asset class, each with a pure parser, a typed hand-rolled
  `Display + Error` enum, a `validate_*` function embedding the asset via
  `include_str!`, `rstest` unit tests, and an `rstest-bdd` `.feature` under
  `crates/repovec-core/tests/features/`. `systemd_units` is wired into both
  daemon startup and `repovec-ci systemd-gate`; `qdrant_quadlet` is gated only by
  its own `make test` happy-path test. The component contract is documented in
  `docs/developers-guide.md` §5.
- The `make validate-systemd` gate runs `repovec-ci systemd-gate`, which today
  validates the systemd unit set (via `systemd_units`) but not the Quadlet or any
  directory-layout asset. This plan extends it (Decision D-10).

## Documentation and skill signposts

Read these repository documents before implementation:

- `docs/roadmap.md`, especially roadmap item 1.3.3 and its cloud-init reuse in
  6.3.1.
- `docs/repovec-appliance-technical-design.md`, especially "Worktrees and
  checkout layout", "Service layout", "Qdrant under Podman + systemd", and the
  `/etc/repovec` secret-permission passages.
- `docs/developers-guide.md`, especially §5 "Appliance module" (extension
  pattern §5.6, test patterns §5.7) and §6 "Provisioning integration tests".
- `docs/documentation-style-guide.md` for the ADR format and design-document
  synchronization rules.
- `docs/rstest-bdd-users-guide.md`, `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`, and
  `docs/complexity-antipatterns-and-refactoring-strategies.md`.
- `docs/execplans/1-3-1-define-repovec-target-and-static-unit-files.md`,
  `docs/execplans/1-3-2-template-unit-for-per-repo-indexers.md`, and
  `docs/execplans/1-2-2-configure-qdrant-api-key-authentication.md` as the
  closest structural and testing precedents. Note that 1-3-1 and 1-3-2 define the
  Red-Green "red" state as tests that compile and run but fail on assertions
  against deliberately invalid content — not as a build break.
- `integration-tests/README.md`, `integration-tests/lib/constants.py`, and
  `integration-tests/provisioning/test_qdrant_api_key.py` for the E2E harness
  conventions to extend.

Use these skills during implementation:

- `leta` for semantic code navigation and refactoring; load it first.
- `rust-router` to select the smallest useful Rust skill; here it routes to
  `domain-cli-and-daemons`, `rust-errors` (typed error enum and newtype design),
  and `rust-unit-testing` (rstest fixtures, table tests, inline insta
  assertions). Use `proptest` for the parser robustness property.
- `hexagonal-architecture` to keep the pure directory-layout policy separated
  from the packaging asset and the live pre-flight adapter, without transplanting
  a `domain/ports/adapters` folder hierarchy the codebase does not use.
- `arch-decision-records` for the Y-Statement-shaped ADR.
- `commit-message` and `pr-creation` for the commit and PR steps.
- `en-gb-oxendict` for British spelling with Oxford `-ize` endings in all prose.
- `firecrawl` only when up-to-date external documentation on `tmpfiles.d`,
  `sysusers.d`, or systemd ordering is needed to resolve a specific gap.

## Plan of work

The work follows Red-Green-Refactor. Milestone 1 establishes running-but-failing
tests and the behavioural specification against a permissive stub. Milestone 2
adds the packaging assets and the pure validator to turn them green. Milestone 3
proves the host contract end to end. Milestone 4 adds the live fail-closed guard.
Milestone 5 documents. Milestone 6 commits and closes the roadmap item. Each
milestone ends with the full gate set and, once gates pass, a clean
`coderabbit review --agent`.

### Stage A: approval gate (no code changes)

Present this plan and await explicit approval. Before writing any test, re-read
`packaging/systemd/repovec-grepai@.service` and confirm R-3: record its actual
`ProtectSystem=` and `ProtectHome=` values and confirm `/var/lib/repovec` is
writable inside the sandbox.

### Milestone 1: red tests and feature specification (permissive stub)

1. Add `qdrant_storage_root()` to `RuntimePaths` in
   `crates/repovec-core/src/lib.rs`, returning `data_root/qdrant-storage`, with
   an `# Examples` doctest matching the existing accessors. Add a focused unit
   test
   in the existing `lib.rs` test module asserting the value. This is a lock test,
   captured separately from the red evidence.

2. Create the module skeleton
   `crates/repovec-core/src/appliance/directory_layout/` with the full
   `DirectoryLayoutError` enum in `error.rs`, the `Mode(u16)` newtype, and a
   `mod.rs` whose `validate_directory_layout(..)` returns `Ok(())` (a permissive
   stub) and whose `checked_in_*()` accessors return the embedded strings. This
   makes the crate compile so tests run.

3. Create `crates/repovec-core/tests/features/directory_layout.feature` with the
   happy path plus these unhappy scenarios (each fails against the permissive
   stub because the stub accepts everything):

   ```gherkin
   Feature: repovec directory-layout contract
     The appliance ships checked-in sysusers.d and tmpfiles.d assets that
     provision the repovec user and its private directory tree.

     Scenario: The checked-in layout assets satisfy the appliance contract
       Given the checked-in repovec layout assets
       When the directory-layout assets are validated
       Then the directory-layout asset set is accepted

     Scenario: The tmpfiles asset must declare every required data directory
       Given the checked-in repovec layout assets
       And the worktrees directory entry is removed from the tmpfiles asset
       When the directory-layout assets are validated
       Then validation fails because a required directory entry is missing

     Scenario: The tmpfiles asset must not declare unexpected directories
       Given the checked-in repovec layout assets
       And an unexpected directory entry is added to the tmpfiles asset
       When the directory-layout assets are validated
       Then validation fails because an unexpected directory entry is present

     Scenario: Data directories must be private to the repovec user
       Given the checked-in repovec layout assets
       And the worktrees directory mode is widened to 0750
       When the directory-layout assets are validated
       Then validation fails because the directory mode is incorrect

     Scenario: Data directories must be owned by the repovec user
       Given the checked-in repovec layout assets
       And the git-mirrors directory owner is changed to root
       When the directory-layout assets are validated
       Then validation fails because the directory owner is incorrect

     Scenario: A data directory group must be repovec
       Given the checked-in repovec layout assets
       And the grepai directory group is changed to wheel
       When the directory-layout assets are validated
       Then validation fails because the directory group is incorrect

     Scenario: Directory entries must set explicit modes and ownership
       Given the checked-in repovec layout assets
       And the worktrees directory mode is replaced with the default token
       When the directory-layout assets are validated
       Then validation fails because a required field is not explicit

     Scenario: The tmpfiles asset must not declare the secrets directory
       Given the checked-in repovec layout assets
       And an /etc/repovec entry is added to the tmpfiles asset
       When the directory-layout assets are validated
       Then validation fails because the secrets directory has a single authority

     Scenario: A malformed tmpfiles line is rejected
       Given the checked-in repovec layout assets
       And a malformed line is inserted into the tmpfiles asset
       When the directory-layout assets are validated
       Then validation fails because a line is malformed

     Scenario: The sysusers asset must declare the repovec user home
       Given the checked-in repovec layout assets
       And the sysusers home path is changed away from /var/lib/repovec
       When the directory-layout assets are validated
       Then validation fails because the sysusers home is incorrect

     Scenario: The sysusers asset must keep the nologin shell
       Given the checked-in repovec layout assets
       And the sysusers shell is changed to /bin/bash
       When the directory-layout assets are validated
       Then validation fails because the sysusers shell is incorrect
   ```

4. Add `crates/repovec-core/tests/directory_layout_bdd.rs` following
   `systemd_units_bdd.rs`: a `#[derive(Default)]` world holding the mutated asset
   strings and an `Option<Result<(), DirectoryLayoutError>>`, a `#[fixture]`, and
   `#[given]`/`#[when]`/`#[then]` functions wired by `#[scenario(...)]`.

5. Add `crates/repovec-core/src/appliance/directory_layout/tests.rs` with one
   parametrized `#[rstest]` per `DirectoryLayoutError` variant that mutates a copy
   of the checked-in asset, runs the real `validate_*`, asserts the typed variant,
   and captures the operator-facing `Display` via an **inline** insta snapshot
   (`assert_snapshot!(err.to_string(), @"…")`), so no `.snap` files are added. Add
   an `rstest` edge table for octal-mode normalization (`0750` vs `750`) and for
   malformed lines. Add `tests_proptest.rs` with one robustness property: for an
   arbitrary line the tokenizer either yields a well-formed entry or returns
   `MalformedLine`, never panics, and never returns a partially populated entry
   (no `prop_assume!` filtering of inputs the parser must handle).

6. Run the focused tests against the permissive stub and record the red evidence:
   the happy path passes; every unhappy case fails on its `assert_eq!` typed
   variant. Capture the transcript.

Validation gate for Milestone 1: the tests compile, run, and fail for the
intended reason. Do not proceed until the red state is captured in `Progress`.

### Milestone 2: packaging assets and pure validator (green)

1. Create `packaging/tmpfiles.d/repovec.conf` (data tree only; `/etc/repovec` is
   helper-authoritative per SI-4):

   ```plaintext
   # Provision the repovec appliance directory tree. repovec-provision.service
   # runs systemd-sysusers before systemd-tmpfiles --create, so the repovec user
   # exists and ownership resolves by name. Modes are explicit: the systemd-
   # tmpfiles default (-) is world-readable 0755, which would leak private repos.
   d /var/lib/repovec               0700 repovec repovec -
   d /var/lib/repovec/git-mirrors   0700 repovec repovec -
   d /var/lib/repovec/worktrees     0700 repovec repovec -
   d /var/lib/repovec/.grepai       0700 repovec repovec -
   d /var/lib/repovec/qdrant-storage 0700 root root -
   ```

2. Create `packaging/systemd/repovec-provision.service` (Decision D-7):

   ```plaintext
   [Unit]
   Description=Provision the repovec user and directory layout
   Wants=systemd-sysusers.service
   After=systemd-sysusers.service
   Before=repovec-qdrant-api-key.service qdrant.service repovecd.service repovec-mcpd.service
   RefuseManualStop=yes

   [Service]
   Type=oneshot
   RemainAfterExit=yes
   ExecStart=/usr/bin/systemd-sysusers /usr/lib/sysusers.d/repovec.conf
   ExecStart=/usr/bin/systemd-tmpfiles --create /usr/lib/tmpfiles.d/repovec.conf
   StandardOutput=journal
   StandardError=journal

   [Install]
   WantedBy=repovec.target
   ```

3. Create the validator module. `parser.rs` provides a low-level tokenizer that
   handles comments, blank lines, the `-` placeholder, and the quoted GECOS
   field, then two typed views: `TmpfilesEntry { kind, path, mode, user, group }`
   (accepting only `d`, rejecting others with `MalformedLine`) and
   `SysusersUserLine { name, home, shell }`. `error.rs` holds
   `DirectoryLayoutError` (Decision D-8) and the `Mode(u16)` newtype. `mod.rs`
   replaces the stub `validate_directory_layout` with real logic: build the spec
   table via `layout_contract(&RuntimePaths::appliance_defaults())` (Decision
   D-9), require that the set of `d` entries in the asset equals the contract's
   path set (missing → `MissingDirectoryEntry`, extra → `UnexpectedDirectoryEntry`;
   this is the bidirectional drift guard), check each entry's mode/owner/group by
   exact match, reject `-` default tokens (SI-5), reject any `/etc/repovec` entry
   (SI-4), and validate the sysusers `u` line's home and shell.

4. Re-export the module from `crates/repovec-core/src/appliance/mod.rs` (one
   additive line). Add the module's happy-path unit test
   (`validate_checked_in_directory_layout().expect(...)`).

5. Extend `repovec-ci systemd-gate` (`run_systemd_gate`) to also call
   `validate_checked_in_directory_layout()` (Decision D-10).

6. Turn the tests green. Run the focused unit test, the edge table, the property
   test, and the BDD runner; record green transcripts. Then run the full gate set
   sequentially, each piped through `tee`:

   ```sh
   make check-fmt 2>&1 | tee /tmp/check-fmt-$(git branch --show-current).out
   make typecheck 2>&1 | tee /tmp/typecheck-$(git branch --show-current).out
   make lint      2>&1 | tee /tmp/lint-$(git branch --show-current).out
   make test      2>&1 | tee /tmp/test-$(git branch --show-current).out
   make validate-systemd 2>&1 | tee /tmp/validate-systemd-$(git branch --show-current).out
   ```

7. Once gates pass, run `coderabbit review --agent`, resolve all applicable
   findings, re-run the affected gates, and commit.

### Milestone 3: end-to-end integration assertions

Prove the roadmap success criterion on a real systemd host using the opt-in
privileged harness under `integration-tests/`, under SELinux enforcing.

1. As a first step, verify systemd runs as PID 1 in the container; if the
   existing harness cannot host systemd-as-PID-1, record the limitation and treat
   the static contract plus a documented manual run as the substitute (1.2.2
   precedent).
2. Extend `integration-tests/lib/constants.py` with the data-tree expectations
   (paths, `repovec:repovec` ownership, mode `0700`; `qdrant-storage` `root:root`
   `0700`). Keep `ETC_DIR_MODE = "0750"` unchanged.
3. Add `integration-tests/provisioning/test_directory_layout.py` that installs
   the `sysusers.d` and `tmpfiles.d` assets, runs `systemd-sysusers` then
   `systemd-tmpfiles --create`, asserts the tree exists with the expected
   ownership (by name, proving R-2) and modes, then starts `qdrant.service`
   specifically, asserts `systemctl is-active qdrant.service`, and runs the
   existing authenticated gRPC liveness probe on loopback. Do not assert
   `is-active repovec.target`, because the target's `Wants=` are weak and pulling
   in the absent `cloudflared.service` would make target activeness meaningless;
   document this exclusion. Mark the suite opt-in; it must not join `make test`.
4. Document the invocation (`make integration-test`) and its prerequisites.

Validation gate: on a capable host, the lifecycle test passes; where it cannot
run, the limitation is documented and the static contract still gates CI.

### Milestone 4: live-ownership pre-flight (fail-closed guard)

1. Add a sibling adapter (for example `appliance::directory_layout::live` or
   `RuntimePaths::verify_live()`), explicitly outside the pure validator, that
   stats `/var/lib/repovec`, its data children, and `/etc/repovec`, returning
   a typed error if any is misowned or over-permissive (data tree must be
   `repovec`-owned with `mode & 0o077 == 0`; `/etc/repovec` must be `root`-owned,
   group `repovec`, non-world). Use `cap-std`/`camino` already in the crate.
2. Wire it into `repovecd`/`repovec-mcpd` startup after systemd-unit validation,
   failing closed (exit non-zero) on mismatch, with structured `tracing` fields.
   Add unit tests using a `tempfile` scratch tree (pre-approved dev-dependency)
   for the pass path and each mismatch path.
3. If, at this point, the file or LOC tolerance would be breached, stop, record
   it in `Decision Log`, and split the live guard into a follow-up roadmap item
   (leaving R-6 explicitly open) rather than exceeding tolerance silently.

Validation gate: the pre-flight passes on a correct tree and fails closed on each
seeded mismatch; the full gate set is green.

### Milestone 5: documentation and roadmap update

1. Add a subsection to `docs/developers-guide.md` §5 (for example §5.8
   "`directory_layout` validation surface") describing the public API, the typed
   error and `Mode` newtype, the spec-table contract, the asset paths and their
   installed locations (`/usr/lib/tmpfiles.d/repovec.conf`,
   `/usr/lib/sysusers.d/repovec.conf`), the `repovec-provision.service` trigger,
   the `systemd-gate` extension, the live pre-flight adapter, and the purity
   boundary. Keep the §5.6 extension-pattern list accurate.
2. Update `docs/users-guide.md`: the appliance provisions `/var/lib/repovec` (and
   its `git-mirrors/`, `worktrees/`, `.grepai/`, `qdrant-storage/` children) as
   private (`0700`) directories owned by `repovec` (`qdrant-storage` by `root`),
   and `/etc/repovec/` holds secrets with restricted permissions; operators must
   not loosen these modes, and the daemons refuse to start if they are loosened.
3. Write an ADR under `docs/adr-NNN-repovec-directory-provisioning.md` (next free
   number; ADRs do not yet exist, so `adr-001` unless one has since been added)
   in the Y-Statement format, recording: `tmpfiles.d` + `sysusers.d` over an
   imperative helper or unit `StateDirectory=`; the `repovec-provision.service`
   install-time trigger and the sysusers-before-tmpfiles ordering; `0700` data
   tree for private-repo confidentiality; `qdrant-storage` `root:root` and its
   rootless dependency; single authority for `/etc/repovec`; and the live
   pre-flight as the runtime backstop. Reference it from
   `docs/repovec-appliance-technical-design.md` and this ExecPlan.
4. Run `make markdownlint` and `make nixie`; re-run the full Rust gate set; then
   `coderabbit review --agent`; resolve findings; commit.

### Milestone 6: commit, close, and PR

1. Mark roadmap item 1.3.3 done in `docs/roadmap.md` (`[ ]` → `[x]` with a short
   completion note in the established style).
2. Ensure all gates are green, commit, and push.

## Concrete steps

Run everything from the repository root. The branch for this work is
`1-3-3-create-repovec-system-user-and-directory-layout`.

Red evidence (Milestone 1), expected to fail against the permissive stub:

```sh
cargo test -p repovec-core directory_layout 2>&1 | tee /tmp/red-directory-layout.out
# Expect: the happy-path case passes; every unhappy case fails its assert_eq!
# on the typed DirectoryLayoutError variant. No compile error.
```

Green evidence (Milestone 2), expected to pass after implementation:

```sh
cargo test -p repovec-core directory_layout 2>&1 | tee /tmp/green-directory-layout.out
# Expect: ok. N passed; 0 failed.
```

Full gate set (run sequentially, never in parallel):

```sh
make check-fmt 2>&1 | tee /tmp/check-fmt-$(git branch --show-current).out
make typecheck 2>&1 | tee /tmp/typecheck-$(git branch --show-current).out
make lint      2>&1 | tee /tmp/lint-$(git branch --show-current).out
make test      2>&1 | tee /tmp/test-$(git branch --show-current).out
make validate-systemd 2>&1 | tee /tmp/validate-systemd-$(git branch --show-current).out
```

Opt-in end-to-end (Milestone 3, on a host with rootful Podman + systemd under
SELinux enforcing):

```sh
make integration-test 2>&1 | tee /tmp/integration-$(git branch --show-current).out
# Expect: the tree ownership/modes assert; qdrant.service is active; the
# authenticated gRPC liveness probe succeeds on loopback.
```

Update transcripts in `Artefacts and notes` as steps complete.

## Validation and acceptance

Acceptance is behavioural:

1. `cargo test -p repovec-core directory_layout` fails before Milestone 2 (happy
   passes, unhappy fails) and passes after. The BDD scenarios fail before and
   pass after.
2. `make check-fmt`, `make typecheck`, `make lint`, `make test`, and
   `make validate-systemd` all pass with zero warnings, and `make validate-systemd`
   now exercises `validate_checked_in_directory_layout()`.
3. Inline insta snapshots capture the operator-facing `Display` of each
   `DirectoryLayoutError` variant, derived from real `validate_*` failures.
4. The live pre-flight fails closed on each seeded ownership/mode mismatch and
   passes on a correct tree.
5. On a capable host, `make integration-test` shows the `/var/lib/repovec` tree
   present with `repovec:repovec` ownership and mode `0700` (`qdrant-storage`
   `root:root` `0700`; `/etc/repovec` `0750 root:repovec`), `qdrant.service`
   active, and Qdrant reachable on loopback gRPC — the roadmap success criterion.
   Where the sandbox cannot run this suite, the limitation is recorded and the
   static contract plus live pre-flight gate the behaviour.

Quality criteria ("done" means):

- Tests: all Rust unit, property, and BDD tests pass; the live pre-flight tests
  pass; the E2E lifecycle test passes on a capable host or its inability to run
  here is documented.
- Lint/typecheck: `make lint` and `make typecheck` clean.
- Docs: developers guide §5, users guide, and the ADR updated; markdownlint and
  nixie clean.
- Security: SI-1 through SI-5 hold; secret-file and secrets-directory permissions
  are unchanged or tightened, never loosened.

Quality method: the gate commands above, plus a clean `coderabbit review
--agent` at each implementation and documentation milestone.

## Idempotence and recovery

- `tmpfiles.d` and `sysusers.d` processing is idempotent; re-running
  `systemd-tmpfiles --create` and `systemd-sysusers` converges without drift and
  does not empty populated directories (the `d` type does not clear contents).
  Note that `d` does not repair misowned *existing children*; the live pre-flight
  detects that case rather than silently tolerating it.
- The Rust validator is a pure read of embedded strings and is safe to re-run.
- If a milestone fails midway, revert the working tree to the last green commit
  (`git restore` / `git reset --hard <commit>` on the feature branch) and retry.
- The integration test operates inside a throwaway privileged container; failures
  leave no residue on the developer host.

## Artefacts and notes

Transcripts (red, green, gate, live-preflight, and integration runs) and the
Stage-A R-3 confirmation will be pasted here as concise codefenced excerpts as
work proceeds.

## Interfaces and dependencies

New and changed interfaces.

In `crates/repovec-core/src/lib.rs`, extend `RuntimePaths` with a pure path
accessor (doctest required):

```rust
impl RuntimePaths {
    /// Qdrant's persistent storage directory (`data_root/qdrant-storage`).
    #[must_use]
    pub fn qdrant_storage_root(&self) -> Utf8PathBuf; // data_root/qdrant-storage
}
```

In `crates/repovec-core/src/appliance/directory_layout/mod.rs`:

```rust
pub fn checked_in_repovec_tmpfiles() -> &'static str;
pub fn checked_in_repovec_sysusers() -> &'static str;
pub const CHECKED_IN_REPOVEC_TMPFILES_PATH: &str = "packaging/tmpfiles.d/repovec.conf";
pub const INSTALLED_REPOVEC_TMPFILES_PATH: &str = "/usr/lib/tmpfiles.d/repovec.conf";
pub const INSTALLED_REPOVEC_SYSUSERS_PATH: &str = "/usr/lib/sysusers.d/repovec.conf";

pub fn validate_checked_in_directory_layout() -> Result<(), DirectoryLayoutError>;
pub fn validate_directory_layout(
    tmpfiles: &str,
    sysusers: &str,
) -> Result<(), DirectoryLayoutError>;
```

In `crates/repovec-core/src/appliance/directory_layout/error.rs` (hand-rolled
`Display` and `impl Error`, no `thiserror`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mode(pub u16); // Display renders as {:04o}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryLayoutError {
    MalformedLine { asset: &'static str, line_number: usize, line: String },
    NonExplicitField { asset: &'static str, line_number: usize, field: &'static str },
    MissingDirectoryEntry { path: Utf8PathBuf },
    UnexpectedDirectoryEntry { path: Utf8PathBuf },
    ForbiddenSecretsEntry { path: Utf8PathBuf },
    IncorrectMode { path: Utf8PathBuf, expected: Mode, actual: Mode },
    IncorrectOwner { path: Utf8PathBuf, expected: &'static str, actual: String },
    IncorrectGroup { path: Utf8PathBuf, expected: &'static str, actual: String },
    SysusersMissingUser,
    SysusersIncorrectHome { expected: &'static str, actual: String },
    SysusersIncorrectShell { expected: &'static str, actual: String },
}

impl DirectoryLayoutError {
    /// The packaging asset the failure relates to, for structured logging.
    #[must_use]
    pub const fn asset(&self) -> &'static str { /* total mapping */ }
}
```

The per-entry contract lives in `directory_layout`, not on `RuntimePaths`:

```rust
struct DirectorySpec {
    path: Utf8PathBuf,
    mode: Mode,
    owner: &'static str,
    group: &'static str,
}

fn layout_contract(paths: &RuntimePaths) -> [DirectorySpec; 5];
```

Libraries and mechanisms to use (already available; no new external
dependencies): `camino` (`Utf8Path`/`Utf8PathBuf`) for paths, hand-rolled error
traits, `rstest`/`rstest-bdd`/`rstest-bdd-macros` for tests, `proptest` for the
single parser-robustness property, `insta` inline snapshots for `Display`, and
`cap-std` (already a dependency) plus `tempfile` (dev-dependency, pre-approved)
for the live pre-flight tests. Packaging assets:
`packaging/tmpfiles.d/repovec.conf` (new),
`packaging/systemd/repovec-provision.service` (new), and
`packaging/sysusers.d/repovec.conf` (existing). Installed targets:
`/usr/lib/tmpfiles.d/repovec.conf` and `/usr/lib/sysusers.d/repovec.conf`.

## Revision note

Revision 2 (2026-07-22): substantially revised after a four-lens
community-of-experts review (systemd/packaging, hexagonal/Rust API, testing
rigour, and security/operations). Changes: added the `repovec-provision.service`
install-time trigger (D-7, R-1) so `systemctl start repovec.target` succeeds
without a reboot; tightened the data tree to `0700` and added security invariants
SI-1 through SI-5; resolved `qdrant-storage` to `root:root` `0700` (D-4) and the
`:Z` relabel interaction (D-3, R-5); made `/etc/repovec` single-authority (SI-4)
and kept it helper-owned; typed the error fields with a `Mode` newtype,
`Utf8PathBuf`, and an `.asset()` discriminator, hand-rolling the error traits
(D-8); moved the heterogeneous mode/owner policy into a `directory_layout` spec
table, keeping `RuntimePaths` pure (D-9); switched the red state to a permissive
stub; narrowed the E2E assertion to `qdrant.service` + gRPC liveness under SELinux
enforcing; added the bidirectional drift guard and a live fail-closed pre-flight
(D-6, R-6); reframed the proptest to a robustness property and added an rstest
edge table; corrected Risk R-3 (`ProtectSystem=full`, not `ProtectHome`); adopted
inline snapshots and raised the file tolerance to 24; and extended
`repovec-ci systemd-gate` to cover the new contract (D-10).

Revision 1 (2026-07-22): initial DRAFT.
