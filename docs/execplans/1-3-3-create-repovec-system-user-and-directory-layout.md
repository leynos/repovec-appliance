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
systemd host obtains, deterministically and at boot, a `repovec` service
account and a correctly owned, correctly permissioned directory tree. The
daemons (`repovecd`, `repovec-mcpd`) whose units already declare
`WorkingDirectory=/var/lib/repovec` and `HOME=/var/lib/repovec` can start
because that directory now exists; the Qdrant container can mount
`/var/lib/repovec/qdrant-storage`; and the whole `repovec.target` reaches
`active` with Qdrant answering an authenticated gRPC liveness probe.

The observable outcome a reviewer can check:

1. The repository ships a checked-in `systemd-tmpfiles` asset that declares the
   full `/var/lib/repovec` tree with `repovec:repovec` ownership and tight
   modes, plus the existing `systemd-sysusers` asset that declares the user.
2. A new pure-Rust validator in `repovec-core` proves that the checked-in
   `tmpfiles.d` and `sysusers.d` assets satisfy the appliance directory-layout
   contract derived from `RuntimePaths`, with typed errors and operator-facing
   `Display` messages under snapshot control.
3. The opt-in privileged integration harness demonstrates the end-to-end
   success criterion on a real systemd host: after the assets are installed,
   `systemctl start repovec.target` succeeds, the directory tree exists with the
   expected ownership and modes, and Qdrant answers on loopback gRPC.

This plan is in DRAFT. Do not begin implementation until the user approves it.

## Constraints

Hard invariants that must hold throughout implementation. Violation requires
escalation, not a workaround.

- The Rust validator added to `repovec-core` must remain pure and I/O-free. It
  may only parse embedded asset text (`include_str!`) and compare it against a
  contract derived from in-memory types. It must not call `systemctl`,
  `systemd-tmpfiles`, `systemd-sysusers`, `useradd`, `podman`, read
  `/etc/passwd`, or touch the live filesystem. Live provisioning is a packaging
  and operator concern, consistent with `docs/developers-guide.md` §5.2-§5.4 and
  the boundary stated in `docs/execplans/1-3-2-template-unit-for-per-repo-indexers.md`.
- `/var/lib/repovec` and its data children (`git-mirrors/`, `worktrees/`,
  `.grepai/`) must be owned by `repovec:repovec`. `/etc/repovec/` must remain
  `root:repovec` mode `0750` and its secret files `repovec:repovec` mode `0400`,
  exactly as task 1.2.2 established in `packaging/libexec/repovec-qdrant-api-key`
  and asserts in `integration-tests/`. This plan must not weaken those secret
  permissions.
- `RuntimePaths` in `crates/repovec-core/src/lib.rs` is the single source of
  truth for appliance paths. The new validator must derive the required path set
  from `RuntimePaths` rather than introducing parallel path literals. If a path
  is needed that `RuntimePaths` does not yet expose, add the accessor to
  `RuntimePaths` rather than hard-coding it in the validator.
- The existing `packaging/sysusers.d/repovec.conf` declaration
  (`u repovec - "repovec appliance service user" /var/lib/repovec /usr/sbin/nologin`)
  must not be broken. The `repovec` user's home must stay `/var/lib/repovec` and
  its shell `/usr/sbin/nologin`; existing units and integration tests depend on
  this shape.
- No numeric uid/gid may be pinned. The design defers allocation to
  `systemd-sysusers` dynamic assignment; the validator must not assert a fixed
  numeric id.
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

- Scope: if the implementation requires touching more than 18 files, or more
  than roughly 900 net lines of code and asset text (excluding committed `.snap`
  snapshot files and generated fixtures), stop and escalate.
- Interface: if any existing public API in `repovec-core` must change signature
  (as opposed to additive new functions), stop and escalate.
- Dependencies: if a new external crate dependency beyond those already in
  `[workspace.dependencies]` is required, stop and escalate. Adding
  `tempfile.workspace = true` to `crates/repovec-core` `[dev-dependencies]`, if
  needed for a filesystem-touching test, is pre-approved and does not count.
- Iterations: if a focused test still fails after 4 fix attempts, stop and
  record the blocker in `Decision Log`.
- Ambiguity: if the qdrant-storage ownership decision (see Decision Log D-4) or
  the grepai sandbox interaction (see Risk R-3) proves to materially change the
  directory contract, stop and present options.
- CodeRabbit: if `coderabbit review --agent` raises a concern that conflicts
  with this plan or requires scope beyond these tolerances, stop and ask for
  approval before acting on it.

## Risks

- Risk R-1: `systemd-tmpfiles` ordering. The `tmpfiles.d` asset chowns
  directories to `repovec:repovec`, which requires the user to exist first.
  Severity: high. Likelihood: low.
  Mitigation: `systemd-sysusers.service` is ordered `Before=`
  `systemd-tmpfiles-setup.service` in stock systemd, so at boot the user exists
  before tmpfiles runs. The integration test must assert ownership resolves to
  the `repovec` user (by name), proving the ordering held. Document the ordering
  guarantee in the ADR and the developers guide.
- Risk R-2: `systemctl start repovec.target` on an already-booted host relies on
  `systemd-tmpfiles-setup.service` having already run at boot; it is not a
  dependency of `repovec.target`. Severity: medium. Likelihood: low.
  Mitigation: on a booted host the directories already exist. The plan keeps the
  daemon units' literal-path convention but adds `StateDirectory=` /
  `ConfigurationDirectory=` as a defence-in-depth decision only if the
  integration test reveals a gap (see Decision Log D-3). The integration test
  starts the target on a fully booted container to reflect real operation.
- Risk R-3: the `repovec-grepai@.service` template (from task 1.3.2) applies
  `ProtectHome=` sandboxing that could render `/var/lib/repovec` read-only inside
  the indexer sandbox, defeating writes to `.grepai/` and worktrees. Severity:
  medium. Likelihood: medium.
  Mitigation: this is 1.3.2 / 3.2.1 territory, not 1.3.3's directory-creation
  scope, but the plan must verify the template's actual `ProtectHome=`/
  `ReadWritePaths=` directives during Milestone 1 and record the finding. If a
  conflict exists, note it as a follow-up for the indexer-lifecycle work rather
  than expanding this plan.
- Risk R-4: the privileged integration harness requires rootful nested Podman
  and systemd inside a container, which CI runners typically cannot grant.
  Severity: low. Likelihood: high (expected).
  Mitigation: keep the E2E assertions in the opt-in `integration-tests/`
  lifecycle suite (not part of `make test`), mirroring the 1.2.2 precedent, and
  provide the exact command to run them on a capable host.
- Risk R-5: parsing `tmpfiles.d` and `sysusers.d` text is more permissive than a
  full `systemd-tmpfiles` parser. Severity: low. Likelihood: medium.
  Mitigation: the validator only needs to recognize the specific directive types
  this asset uses (`d`/`D`/`z`/`Z` for tmpfiles; the `u` line for sysusers).
  Restrict the parser to those, reject malformed lines with a typed error, and
  cover the grammar with `proptest` where an invariant spans arbitrary inputs.

## Progress

- [ ] Stage A: approval gate — this DRAFT is presented and approved (no code
  changes until then).
- [ ] Milestone 1: red tests and feature specification (completed: —;
  remaining: all).
- [ ] Milestone 2: packaging assets and Rust validator (green).
- [ ] Milestone 3: end-to-end integration assertions.
- [ ] Milestone 4: documentation (developers guide, users guide, ADR) and
  roadmap update.
- [ ] Milestone 5: commit, push, and mark roadmap item done.

Timestamps will be added as each item completes.

## Surprises & Discoveries

- Observation: `crates/repovec-core/src/lib.rs` already defines `RuntimePaths`
  with `git_mirrors_root()`, `worktrees_root()`, `grepai_root()`, and
  `github_oauth_token_credential()`, but it has no consumers anywhere in the
  workspace.
  Evidence: recon grep across the workspace found references only within
  `lib.rs`.
  Impact: the new validator becomes the first real consumer of `RuntimePaths`,
  which strengthens the domain boundary at no extra cost.
- Observation: `/etc/repovec/` (mode `0750`, `root:repovec`) and the
  `0400 repovec:repovec` secret files already exist and are integration-tested
  from task 1.2.2's `packaging/libexec/repovec-qdrant-api-key`.
  Evidence: `packaging/libexec/repovec-qdrant-api-key` lines 64, 72, 101-102 and
  `integration-tests/provisioning/test_qdrant_api_key.py`.
  Impact: 1.3.3's `/etc/repovec/` obligation is largely satisfied; this plan
  makes the contract explicit and adds the `/var/lib/repovec` tree, rather than
  re-implementing secret provisioning.

## Decision Log

- Decision D-1: create the `/var/lib/repovec` directory tree with a checked-in
  `packaging/tmpfiles.d/repovec.conf` processed by `systemd-tmpfiles`, paired
  with the existing `packaging/sysusers.d/repovec.conf`.
  Rationale: `systemd-sysusers` creates the passwd/group entry but never the
  home directory; `tmpfiles.d` is the freedesktop-idiomatic and Debian-policy
  endorsed mechanism for creating directories below `/var`, runs after
  `systemd-sysusers` at boot, is declarative and idempotent, and matches the
  appliance's existing declarative-asset style. An imperative `install -d` in a
  libexec helper (the 1.2.2 pattern) is the alternative; it is retained only for
  `/etc/repovec` where a helper already runs, and is not extended to the data
  tree. See the ADR for the full comparison.
  Date/Author: 2026-07-22, planning agent.
- Decision D-2: add a new flat validator module
  `crates/repovec-core/src/appliance/directory_layout/` following the
  established `parser` + `error` + `validate_*` + `include_str!` + insta-snapshot
  shape, rather than folding directory-layout checks into
  `appliance::systemd_units` or `appliance::qdrant_quadlet`.
  Rationale: the appliance module convention is one submodule per asset class
  (`docs/developers-guide.md` §5.1, §5.6). `tmpfiles.d` and `sysusers.d` are a
  distinct asset class from unit files and Quadlets.
  Date/Author: 2026-07-22, planning agent.
- Decision D-3 (open, resolve in Milestone 3): whether daemon units should gain
  `StateDirectory=repovec` / `ConfigurationDirectory=repovec` as defence in
  depth. Default: no — keep the literal-path convention already shipped, because
  the boot-time `tmpfiles.d` guarantees existence and `StateDirectory=` would
  create a second, potentially conflicting ownership authority. Revisit only if
  the integration test shows a real start-ordering gap.
  Date/Author: 2026-07-22, planning agent.
- Decision D-4 (open, resolve in Milestone 2): ownership and mode of
  `/var/lib/repovec/qdrant-storage`. Default proposal: `root:root` mode `0750`,
  because Qdrant runs under rootful Podman and the Quadlet applies an explicit
  `:Z` SELinux relabel; today Podman auto-creates the missing bind source as
  `root:root`. Declaring it in `tmpfiles.d` makes the mode explicit and stable.
  The alternative (`repovec:repovec`) is rejected unless the Qdrant container is
  later reconfigured to run as `repovec`. Flag to the expert review.
  Date/Author: 2026-07-22, planning agent.
- Decision D-5: reuse `RuntimePaths::appliance_defaults()` as the contract source
  for the validator; extend `RuntimePaths` with a `qdrant_storage_root()`
  accessor (and any other missing child) rather than hard-coding the path.
  Rationale: keeps one source of truth (a Constraint) and makes `RuntimePaths`
  its own first consumer.
  Date/Author: 2026-07-22, planning agent.

## Outcomes & Retrospective

To be completed at milestones and at completion. Compare the delivered directory
contract and validator against this purpose statement; capture any deviation in
mode/ownership decisions and any follow-up filed for the grepai sandbox
interaction (R-3).

## Context and orientation

The reader is assumed to know nothing about this repository. The repovec
appliance is a Rust workspace that turns private GitHub repositories into a
continuously indexed, MCP-queryable corpus. It ships packaging assets (systemd
units, Podman Quadlets, a `sysusers.d` declaration, and a libexec provisioning
helper) under `packaging/`, and a shared library crate `repovec-core` that
statically validates those checked-in assets so that a broken contract fails a
build gate rather than a production host.

Key terms:

- **`systemd-sysusers` / `sysusers.d`**: a systemd facility that reconstructs
  `/etc/passwd` and `/etc/group` from declarative snippets. A `u` line declares
  a user. It creates the account record but not the home directory contents.
- **`systemd-tmpfiles` / `tmpfiles.d`**: a systemd facility that creates and
  adjusts files and directories from declarative snippets. A `d` line means
  "create this directory with this mode, user, and group"; `D` additionally
  empties it on boot; `z`/`Z` adjust ownership and mode of existing paths (`Z`
  recursively). It runs at boot via `systemd-tmpfiles-setup.service`, ordered
  after `systemd-sysusers.service`.
- **checked-in asset**: a file under `packaging/` that is embedded into
  `repovec-core` at compile time with `include_str!` and validated by a pure
  function, so the repository's copy is the contract of record.
- **`RuntimePaths`**: the type in `crates/repovec-core/src/lib.rs` that names the
  appliance's config root (`/etc/repovec`) and data root (`/var/lib/repovec`) and
  derives child paths.

Current state relevant to this task:

- `packaging/sysusers.d/repovec.conf` already declares the `repovec` user with
  home `/var/lib/repovec` and shell `/usr/sbin/nologin`. It is not validated from
  Rust yet.
- `/etc/repovec/` is created (mode `0750`, `root:repovec`) by
  `packaging/libexec/repovec-qdrant-api-key` (task 1.2.2), which also writes the
  `0400 repovec:repovec` Qdrant API-key file. This behaviour is exercised by the
  opt-in Python harness under `integration-tests/`.
- No `packaging/tmpfiles.d/` directory exists. Nothing on disk creates
  `/var/lib/repovec`, `git-mirrors/`, `worktrees/`, `.grepai/`, or
  `qdrant-storage/`. This is the gap 1.3.3 closes.
- `packaging/systemd/repovecd.service` and `repovec-mcpd.service` declare
  `User=repovec`, `Group=repovec`, `WorkingDirectory=/var/lib/repovec`, and
  `Environment=HOME=/var/lib/repovec`. `packaging/systemd/repovec-grepai@.service`
  declares `WorkingDirectory=/var/lib/repovec/worktrees/%I`. All of these will
  fail to start until the directories exist.
- `packaging/systemd/qdrant.container` bind-mounts
  `/var/lib/repovec/qdrant-storage:/qdrant/storage:Z`.
- Appliance validators live under `crates/repovec-core/src/appliance/`, one
  submodule per asset class, each with a pure parser, a typed `Display + Error`
  enum, a `validate_*` function that embeds the asset via `include_str!`,
  colocated insta `Display` snapshots, `rstest` unit tests, and an `rstest-bdd`
  `.feature` under `crates/repovec-core/tests/features/`. The component contract
  is documented in `docs/developers-guide.md` §5.
- The `make validate-systemd` gate (which runs `repovec-ci systemd-gate`)
  validates checked-in systemd/appliance contracts and is required in CI.

## Documentation and skill signposts

Read these repository documents before implementation:

- `docs/roadmap.md`, especially roadmap item 1.3.3 (and its cloud-init reuse in
  6.3.1).
- `docs/repovec-appliance-technical-design.md`, especially "Worktrees and
  checkout layout", "Service layout", "Qdrant under Podman + systemd", and the
  `/etc/repovec` secret-permission passages.
- `docs/developers-guide.md`, especially §5 "Appliance module" (the extension
  pattern §5.6 and test patterns §5.7) and §6 "Provisioning integration tests".
- `docs/documentation-style-guide.md` for the ADR format (§ "Architectural
  decision records") and design-document synchronization rules.
- `docs/rstest-bdd-users-guide.md`, `docs/rust-testing-with-rstest-fixtures.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`, and
  `docs/complexity-antipatterns-and-refactoring-strategies.md` for testing and
  refactoring discipline.
- `docs/execplans/1-3-1-define-repovec-target-and-static-unit-files.md`,
  `docs/execplans/1-3-2-template-unit-for-per-repo-indexers.md`, and
  `docs/execplans/1-2-2-configure-qdrant-api-key-authentication.md` as the
  closest structural and testing precedents.
- `integration-tests/README.md`, `integration-tests/lib/constants.py`, and
  `integration-tests/provisioning/test_qdrant_api_key.py` for the E2E harness
  conventions to extend.

Use these skills during implementation:

- `leta` for semantic code navigation, references, and refactoring; load it
  first for any code task.
- `rust-router` to select the smallest useful Rust skill; for this work it
  routes to `domain-cli-and-daemons` (packaging/provisioning shape),
  `rust-errors` (typed error enum design), and `rust-unit-testing` (rstest
  fixtures, table tests, insta assertions). Use `proptest` for parser
  invariants.
- `hexagonal-architecture` to keep the pure directory-layout policy separated
  from the packaging/host adapter, without transplanting a `domain/ports/adapters`
  folder hierarchy the codebase does not use.
- `arch-decision-records` for the Y-Statement-shaped ADR.
- `rstest-bdd` guidance (via the users guide) for the behavioural feature.
- `commit-message` and `pr-creation` for the commit and PR steps.
- `en-gb-oxendict` for British/Oxford spelling in all prose.
- `firecrawl` only when up-to-date external documentation on `tmpfiles.d`,
  `sysusers.d`, or systemd ordering is needed to resolve a specific gap.

## Plan of work

The work follows Red-Green-Refactor. Milestone 1 establishes failing tests and
the behavioural specification. Milestone 2 adds the packaging asset and the pure
validator to turn them green. Milestone 3 adds the end-to-end proof. Milestone 4
documents. Milestone 5 commits and closes the roadmap item. Each milestone ends
with the full gate set and, once gates pass, a clean `coderabbit review --agent`.

### Stage A: approval gate (no code changes)

Present this plan and await explicit approval. Do not proceed to Milestone 1
until the user confirms. Before writing any test, re-read
`packaging/systemd/repovec-grepai@.service` to record its actual `ProtectHome=`
and `ReadWritePaths=` directives against Risk R-3.

### Milestone 1: red tests and feature specification

Add, but do not yet satisfy, the specification for the directory-layout
contract.

1. Extend `RuntimePaths` in `crates/repovec-core/src/lib.rs` with a
   `qdrant_storage_root()` accessor returning `data_root/qdrant-storage`, plus a
   single method that enumerates the required data directories in order (for
   example `data_directories() -> [Utf8PathBuf; N]`) so the validator and tests
   share one list. Add a focused unit test in the existing `lib.rs` test module
   asserting each accessor's value. This test is additive and passes
   immediately; it exists to lock the path contract.

2. Create the behavioural specification
   `crates/repovec-core/tests/features/directory_layout.feature` describing the
   contract in Gherkin. Include at least these scenarios (happy path plus
   representative unhappy paths):

   ```gherkin
   Feature: repovec directory-layout contract
     The appliance ships checked-in sysusers.d and tmpfiles.d assets that
     provision the repovec user and its directory tree.

     Scenario: The checked-in layout assets satisfy the appliance contract
       Given the checked-in repovec layout assets
       When the directory-layout assets are validated
       Then the directory-layout asset set is accepted

     Scenario: The tmpfiles asset must declare every required data directory
       Given the checked-in repovec layout assets
       And the worktrees directory entry is removed from the tmpfiles asset
       When the directory-layout assets are validated
       Then validation fails because a required directory entry is missing

     Scenario: Data directories must be owned by the repovec user
       Given the checked-in repovec layout assets
       And the git-mirrors directory owner is changed to root
       When the directory-layout assets are validated
       Then validation fails because the directory owner is incorrect

     Scenario: The secrets directory must not be world-accessible
       Given the checked-in repovec layout assets
       And the config directory mode is widened to 0755
       When the directory-layout assets are validated
       Then validation fails because the secrets directory is not restricted

     Scenario: The sysusers asset must declare the repovec user home
       Given the checked-in repovec layout assets
       And the sysusers home path is changed away from /var/lib/repovec
       When the directory-layout assets are validated
       Then validation fails because the sysusers home is incorrect
   ```

3. Add the step-definition/runner file
   `crates/repovec-core/tests/directory_layout_bdd.rs` following the shape of
   `crates/repovec-core/tests/systemd_units_bdd.rs`: a `#[derive(Default)]`
   world struct holding the mutated asset strings and an
   `Option<Result<(), DirectoryLayoutError>>`, a `#[fixture]`, and
   `#[given]`/`#[when]`/`#[then]` functions wired by `#[scenario(...)]`. The
   mutations reference symbols that do not exist yet, so this file will not
   compile — that is the red state for the BDD layer.

4. Add the unit-test module
   `crates/repovec-core/src/appliance/directory_layout/tests.rs` (declared but
   feature-gated behind `#[cfg(test)]`) with `#[rstest]` cases enumerating each
   `DirectoryLayoutError` variant, and an insta `Display` snapshot harness in the
   `qdrant_quadlet`/`systemd_units` style (via
   `insta::assert_snapshot!(scenario.snapshot_label(), err.to_string())`). Add a
   `tests_proptest.rs` sibling asserting a parser invariant (for example: every
   accepted `tmpfiles.d` `d` line round-trips through the parser to the same
   path/mode/owner/group tuple, and any line with an unknown type is rejected
   with `MalformedLine`). These reference the not-yet-created module and fail to
   compile.

5. Run the focused tests and record the red evidence (compile failure for the
   missing module, then, once the module skeleton exists but returns
   `unimplemented!`/an empty contract, assertion failures). Capture transcripts.

Validation gate for Milestone 1: the new tests are present and fail for the
intended reason. Do not proceed until the red state is captured in `Progress`.

### Milestone 2: packaging asset and pure validator (green)

1. Create `packaging/tmpfiles.d/repovec.conf`. Proposed content (subject to
   Decision D-4 on `qdrant-storage`):

   ```plaintext
   # Provision the repovec appliance directory tree. systemd-sysusers creates
   # the repovec user (see packaging/sysusers.d/repovec.conf) before
   # systemd-tmpfiles-setup runs, so ownership resolves by name.
   d /var/lib/repovec            0750 repovec repovec -
   d /var/lib/repovec/git-mirrors 0750 repovec repovec -
   d /var/lib/repovec/worktrees   0750 repovec repovec -
   d /var/lib/repovec/.grepai     0750 repovec repovec -
   d /var/lib/repovec/qdrant-storage 0750 root root -
   d /etc/repovec                 0750 root repovec -
   ```

2. Create the validator module
   `crates/repovec-core/src/appliance/directory_layout/` with:
   - `mod.rs` exposing `checked_in_repovec_tmpfiles() -> &'static str`,
     `checked_in_repovec_sysusers() -> &'static str`,
     `CHECKED_IN_REPOVEC_TMPFILES_PATH`, `INSTALLED_REPOVEC_TMPFILES_PATH`
     (`/usr/lib/tmpfiles.d/repovec.conf`),
     `validate_checked_in_directory_layout()` and
     `validate_directory_layout(tmpfiles, sysusers)` (exact signatures in
     `Interfaces and dependencies`).
     Embed the assets with
     `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../packaging/tmpfiles.d/repovec.conf"))`
     and the analogous `sysusers.d` path.
   - `parser.rs`: a private `pub(super)` parser producing typed tmpfiles entries
     (`TmpfilesEntry { kind, path, mode, user, group }`) and a sysusers `u`-line
     view. Recognize only the directive types the asset uses; reject others with
     `MalformedLine { line_number }`.
   - `error.rs`: `pub enum DirectoryLayoutError` implementing
     `Error + Display + Clone + Debug + Eq + PartialEq` with variants such as
     `MissingDirectoryEntry { path }`, `IncorrectMode { path, expected, actual }`,
     `IncorrectOwner { path, expected, actual }`,
     `IncorrectGroup { path, expected, actual }`,
     `WorldWritable { path }`, `SecretsDirNotRestricted { path, mode }`,
     `SysusersMissingUser`, `SysusersIncorrectHome { expected, actual }`,
     `SysusersIncorrectShell { expected, actual }`, and
     `MalformedLine { line_number }`. Provide a `path()`/`asset()` accessor for
     structured logging, mirroring `SystemdUnitError::unit()`.
   - `tests.rs`, `tests_proptest.rs`, and `snapshots/` (created in Milestone 1)
     now compile and pass.
   The validation policy derives its required-directory set, expected owners, and
   expected modes from `RuntimePaths::appliance_defaults()` and the
   `data_directories()` enumeration added in Milestone 1, so the domain policy
   (which directories, which owner, which modes) is expressed once and the
   adapter asset (`tmpfiles.d` text) is checked against it.

3. Re-export the module from `crates/repovec-core/src/appliance/mod.rs` (a single
   additive line).

4. Turn the tests green. Run the focused unit test, the proptest, and the BDD
   runner; record green transcripts. Then run the full gate set sequentially,
   each piped through `tee`:

   ```sh
   make check-fmt 2>&1 | tee /tmp/check-fmt-$(git branch --show-current).out
   make typecheck 2>&1 | tee /tmp/typecheck-$(git branch --show-current).out
   make lint      2>&1 | tee /tmp/lint-$(git branch --show-current).out
   make test      2>&1 | tee /tmp/test-$(git branch --show-current).out
   make validate-systemd 2>&1 | tee /tmp/validate-systemd-$(git branch --show-current).out
   ```

5. Once gates pass, run `coderabbit review --agent`, resolve all applicable
   findings, re-run the affected gates, and commit.

### Milestone 3: end-to-end integration assertions

Prove the roadmap success criterion on a real systemd host using the opt-in
privileged harness under `integration-tests/`.

1. Extend `integration-tests/lib/constants.py` with the data-tree expectations
   (paths, `repovec:repovec` ownership, mode `0750`, and the `qdrant-storage`
   ownership resolved by Decision D-4).
2. Add a provisioning lifecycle test (for example
   `integration-tests/provisioning/test_directory_layout.py`) that installs the
   `sysusers.d` and `tmpfiles.d` assets, runs `systemd-sysusers` then
   `systemd-tmpfiles --create`, and asserts the tree exists with the expected
   ownership and modes; then starts `repovec.target` and asserts it reaches
   `active` and that the Qdrant gRPC endpoint answers on loopback (reusing the
   existing container/harness helpers and the Qdrant liveness path). Mark it
   opt-in exactly like the existing lifecycle suite; it must not join
   `make test`.
3. Document the exact invocation (`make integration-test`) and its prerequisites.
   Because this suite requires privileges the sandbox may lack, if it cannot run
   here, record that in `Progress` and `Decision Log` and rely on the paper
   contract (Rust validator) plus a manual run instruction, consistent with the
   1.2.2 precedent.

Validation gate: on a capable host, the lifecycle test passes; where it cannot
run, the limitation is documented and the static contract still gates CI.

### Milestone 4: documentation and roadmap update

1. Add a new subsection to `docs/developers-guide.md` §5 (for example §5.8
   "`directory_layout` validation surface") describing the public API, the typed
   error, the `RuntimePaths`-derived contract, the `tmpfiles.d`/`sysusers.d`
   asset paths and their installed locations, and the purity constraint. Keep the
   §5.6 extension-pattern list accurate.
2. Update `docs/users-guide.md` with an operator-facing note: the appliance
   provisions `/var/lib/repovec` (and its `git-mirrors/`, `worktrees/`,
   `.grepai/`, `qdrant-storage/` children) owned by `repovec`, and `/etc/repovec/`
   holds secrets with restricted permissions; operators should not loosen these
   modes.
3. Write an ADR under `docs/adr-NNN-repovec-directory-provisioning.md` (next free
   number; ADRs do not yet exist, so `adr-001` unless one has since been added)
   in the Y-Statement format from `docs/documentation-style-guide.md`, recording
   the choice of `tmpfiles.d` + `sysusers.d` over an imperative libexec helper or
   unit `StateDirectory=`, with the boot-ordering rationale and the
   `qdrant-storage` ownership decision. Reference it from
   `docs/repovec-appliance-technical-design.md` and this ExecPlan.
4. Run `make markdownlint` and `make nixie`; then re-run the full Rust gate set;
   then `coderabbit review --agent`; resolve findings; commit.

### Milestone 5: commit, close, and PR

1. Mark roadmap item 1.3.3 as done in `docs/roadmap.md` (change `[ ]` to `[x]`
   and add a short completion note in the established style).
2. Ensure all gates are green, commit, and push.

## Concrete steps

Run everything from the repository root
`/home/leynos/.lody/repos/github---leynos---repovec-appliance/worktrees/<worktree>`.
The branch for this work is
`1-3-3-create-repovec-system-user-and-directory-layout`.

Red evidence (Milestone 1), expected to fail before implementation:

```sh
cargo test -p repovec-core directory_layout 2>&1 | tee /tmp/red-directory-layout.out
# Expect: compile error (unresolved module `directory_layout`) or, once the
# skeleton exists, assertion failures for each DirectoryLayoutError case.
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

Opt-in end-to-end (Milestone 3, on a host with rootful Podman + systemd):

```sh
make integration-test 2>&1 | tee /tmp/integration-$(git branch --show-current).out
# Expect: the directory-layout lifecycle test asserts the tree ownership/modes,
# repovec.target reaches active, and Qdrant answers on loopback gRPC.
```

Update transcripts in `Artefacts and notes` as steps complete.

## Validation and acceptance

Acceptance is behavioural:

1. `cargo test -p repovec-core directory_layout` fails before Milestone 2 (for
   the intended reason) and passes after. The BDD scenarios in
   `directory_layout.feature` fail before and pass after.
2. `make check-fmt`, `make typecheck`, `make lint`, `make test`, and
   `make validate-systemd` all pass with zero warnings.
3. Committed insta snapshots capture the operator-facing `Display` string of each
   `DirectoryLayoutError` variant, derived from real `validate_*` failures (not
   hand-constructed errors), per `docs/developers-guide.md` §5.7.
4. On a capable host, `make integration-test` shows `systemctl start
   repovec.target` succeeding, the `/var/lib/repovec` tree present with
   `repovec:repovec` ownership and mode `0750` (and `/etc/repovec` `0750`
   `root:repovec`), and Qdrant reachable on loopback gRPC — the roadmap success
   criterion. Where the sandbox cannot run this suite, the limitation is recorded
   and the static Rust contract gates CI.

Quality criteria ("done" means):

- Tests: all Rust unit, proptest, and BDD tests pass; the E2E lifecycle test
  passes on a capable host or its inability to run here is documented.
- Lint/typecheck: `make lint` and `make typecheck` clean.
- Docs: developers guide §5, users guide, and the ADR updated and
  markdownlint/nixie clean.
- Security: secret-file and secrets-directory permissions are unchanged or
  tightened, never loosened.

Quality method: the gate commands above, plus a clean `coderabbit review
--agent` at each implementation and documentation milestone.

## Idempotence and recovery

- `tmpfiles.d` and `sysusers.d` processing is inherently idempotent; re-running
  `systemd-tmpfiles --create` and `systemd-sysusers` converges without drift and
  does not clobber existing data (the `d` type does not empty populated
  directories).
- The Rust validator is a pure read of embedded strings; it has no side effects
  and is safe to re-run.
- If a milestone fails midway, revert the working tree to the last green commit
  (`git restore` / `git reset --hard <commit>` on the feature branch) and retry;
  no destructive host state is created by the Rust or asset changes.
- The integration test operates inside a throwaway privileged container; failures
  leave no residue on the developer host.

## Artefacts and notes

Transcripts (red, green, gate, and integration runs) will be pasted here as
concise codefenced excerpts as work proceeds, focused on what proves success.

## Interfaces and dependencies

New and changed interfaces at the end of Milestone 2:

In `crates/repovec-core/src/lib.rs`, extend `RuntimePaths`:

```rust
impl RuntimePaths {
    pub fn qdrant_storage_root(&self) -> Utf8PathBuf; // data_root/qdrant-storage
    /// Required data subdirectories, in declaration order.
    pub fn data_directories(&self) -> Vec<Utf8PathBuf>;
}
```

In `crates/repovec-core/src/appliance/directory_layout/mod.rs`:

```rust
pub fn checked_in_repovec_tmpfiles() -> &'static str;
pub fn checked_in_repovec_sysusers() -> &'static str;
pub const CHECKED_IN_REPOVEC_TMPFILES_PATH: &str = "packaging/tmpfiles.d/repovec.conf";
pub const INSTALLED_REPOVEC_TMPFILES_PATH: &str = "/usr/lib/tmpfiles.d/repovec.conf";

pub fn validate_checked_in_directory_layout() -> Result<(), DirectoryLayoutError>;
pub fn validate_directory_layout(
    tmpfiles: &str,
    sysusers: &str,
) -> Result<(), DirectoryLayoutError>;
```

In `crates/repovec-core/src/appliance/directory_layout/error.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DirectoryLayoutError {
    MissingDirectoryEntry { path: String },
    IncorrectMode { path: String, expected: String, actual: String },
    IncorrectOwner { path: String, expected: String, actual: String },
    IncorrectGroup { path: String, expected: String, actual: String },
    WorldWritable { path: String },
    SecretsDirNotRestricted { path: String, mode: String },
    SysusersMissingUser,
    SysusersIncorrectHome { expected: String, actual: String },
    SysusersIncorrectShell { expected: String, actual: String },
    MalformedLine { line_number: usize },
}
```

Libraries and mechanisms to use (already available; no new external
dependencies): `camino` (`Utf8Path`/`Utf8PathBuf`) for paths, `thiserror` for
the error enum, `rstest`/`rstest-bdd`/`rstest-bdd-macros` for tests, `proptest`
for parser invariants, and `insta` (yaml) for `Display` snapshots. `tempfile`
may be added to `crates/repovec-core` `[dev-dependencies]` only if a test
needs a scratch directory. Packaging assets: `packaging/tmpfiles.d/repovec.conf`
(new)
and `packaging/sysusers.d/repovec.conf` (existing). Installed target for the
tmpfiles asset is `/usr/lib/tmpfiles.d/repovec.conf`.

## Revision note

Initial DRAFT authored 2026-07-22. Structure, testing conventions, and the
appliance validator pattern follow the 1.3.1, 1.3.2, and 1.2.2 execplans and
`docs/developers-guide.md` §5. Open decisions D-3 (`StateDirectory=`) and D-4
(`qdrant-storage` ownership) are flagged for the expert review and for
resolution during implementation.
