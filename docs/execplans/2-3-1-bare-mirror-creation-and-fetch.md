# Implement bare mirror creation and fetch (2.3.1)

Status: DRAFT — awaiting explicit approval before implementation.

This ExecPlan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current at each milestone boundary.

## Purpose / big picture

Roadmap task 2.3.1 gives the control plane a callable operation that creates a
bare Git mirror at `/var/lib/repovec/git-mirrors/{owner}/{repo}.git` and
fetches updates into that same mirror on subsequent calls. A bare repository
stores Git objects and references without a checked-out source directory. A
mirror copies the remote's advertised reference names, including branches,
tags, and notes, rather than creating only remote-tracking branches.

Success is observable with a temporary upstream repository: synchronize it,
change commits and refs upstream, synchronize again, and compare the mirror's
ref names and object identifiers with the upstream advertisement. Its directory
and existing object store survive the second operation. Private GitHub access
uses the existing OAuth token without storing it in Git configuration.

This task delivers a library/application boundary plus a runnable example and
end-to-end tests. It does not add repository discovery, a scheduler, worktrees,
a new operator CLI, or automatic daemon startup synchronization. Those belong
to 2.2, 2.3.2, and 2.3.3. The roadmap remains unchecked during planning.

## Constraints

- Implementation starts only after explicit approval of this plan.
- Keep pure identity validation and the mirror port in `repovec-core`; put
  filesystem, subprocess, authentication wiring, and orchestration in
  `repovecd`. Dependencies point inward. Adapters do not call token-store or
  discovery adapters directly; the composition root supplies credentials.
- Reuse `RuntimePaths::git_mirrors_root()`. Production uses the provisioned
  private root, owned by `repovec`, with mode `0700`. Tests inject temporary
  roots. Use `cap_std::fs_utf8` and `camino` for owned filesystem operations.
- Never overwrite an invalid existing destination, follow an escaping symlink,
  delete another repository, or print/persist a bearer credential. Git command
  arguments contain no secrets. Do not change parent-process environment.
- Serialize operations on the same canonical repository across processes.
  Different repositories must not share a global operation lock.
- No shallow or partial clones, automatic worktree resets, branch filtering,
  Git pushes, automatic credential refresh, or repository removal in this task.
- Keep Rust modules below 400 lines, use semantic error enums, and retain the
  strict lint baseline. No compatibility wrappers for new or private APIs.
- Run formatting, format checks, lint, and tests sequentially. Use the shared
  Cargo cache and normal compiler admission. Disk exhaustion stops the work.

## Tolerances (exception triggers)

Stop, record a proposed deviation, set status to BLOCKED, and seek approval if
implementation needs a different transport, a new daemon/CLI workflow, changes
to provisioning ownership, a persistent metadata database, or more than 30
non-snapshot source/configuration files. The proposed Git CLI dependency and new
`base64`, `rustix`, `googletest`, and `pretty_assertions` crate requirements
are within scope; other new runtime dependencies require review. `rustix` is
reserved for safe Unix process-group termination and no-replace rename where
the standard library lacks the required operation. Existing workspace test
libraries may be reused. A necessary locking library is not silently added:
first assess the pinned standard library's file-lock API.

Stop after three unsuccessful attempts at the same focused regression and
report evidence rather than weakening its assertion. No elapsed-time limit is
imposed. A failing baseline gate is a blocker, not permission to suppress a
lint or skip a suite. New invariants or unapproved trust-boundary changes
require revisiting the contract and its verification before further coding.

## Risks

R1: configuration or environment can redirect Git or expose credentials (high
impact, medium likelihood). Build a child-only allowlisted environment,
validate managed configuration, and test hostile inherited configuration.

R2: process death, failed fetch, or competing writers can expose incomplete
state (high impact, medium likelihood). Stage creation privately, serialize
writers, publish once, and use Git's atomic ref transaction for fetch. Test
failure before publication and during a conflicting ref update.

R3: mirror semantics can invalidate later worktree assumptions (high impact,
medium likelihood). Mirror branches are remote-owned. Future worktrees must use
detached commits or a separately designed ref policy. Do not add
`--update-head-ok` to bypass checked-out-branch protection.

R4: subprocess paths are not capability-confined merely because Rust uses
`cap_std` (high impact, low likelihood within the service-owned tree). Validate
ownership and symlinks and require a private, cooperative service account.
Malicious root or same-UID writers are outside this isolation model.

R5: Git version and credential transport behaviour vary (medium impact, medium
likelihood). Record `git --version`, require the flags used here in integration
tests, and validate authenticated smart HTTP locally. Do not use live GitHub
accounts as a normal test dependency.

## Progress

- [x] (2026-10-20) Renamed the clean session branch to the requested name.
- [x] (2026-10-20) Completed two Wyvern reconnaissance passes and Git manual
  research; shared source references in context pack `pk_43viskd3`.
- [x] (2026-10-20) Completed Pandalump/Wafflecat, Telefono/Dinolump, and
  Buzzy Bee/Doggylump reviews; revised commit-point, deadline, configuration
  inspection, process draining, deployment, and shared composition contracts.
- [ ] Validate and commit the planning documents; push and open a draft PR.
- [ ] Obtain explicit plan approval.
- [ ] M1: establish contracts, identity model, and executable test seams.
- [ ] M2: implement safe initial mirror publication.
- [ ] M3: implement incremental fetch and authenticated transport.
- [ ] M4: complete end-to-end evidence and documentation; mark 2.3.1 done.

## Surprises & discoveries

The baseline has no Git clone/fetch implementation or `git2`/`gix` dependency.
`RuntimePaths` and secure directory provisioning already exist. The OAuth
implementation provides redacted `AccessToken` values and
`EncryptedGitHubTokenStore::load_token()`, so another token store is
unnecessary.

CodeGraph was requested and its skill loaded, but no CodeGraph tools were
exposed in this session, including after tool discovery. Wyvern used focused
source reads; no graph-indexing success is claimed. The pinned toolchain is
`nightly-2026-03-12`; borrow-checker posture is unknown and no design decision
here depends on a particular borrow-checker mode.

The actual Makefile runs documentation generation, Clippy, and Whitaker through
`make lint`, and a Python gate runner plus Rust tests/doctests through
`make test`. Follow these targets rather than substituting the shorter command
examples in repository guidance.

## Decision log

D1 (2026-10-20, proposed): use the installed Git CLI behind an injected runner.
Its mirror/refspec and atomic-fetch contracts avoid reimplementing Git protocol
or reference transactions. `git2` and `gix` would introduce another transport
and transaction contract without a demonstrated requirement.

D2 (2026-10-20, proposed): mirror all advertised `refs/*`, force remote-owned
updates, and prune disappeared refs, including tags. Exclude pseudorefs such as
`HEAD` from the equality contract. Clone establishes `HEAD`; fetch does not
promise to update the remote default-branch symref. Future branch discovery
supplies that information independently.

D3 (2026-10-20, proposed): validate repository identity before constructing
paths or URLs; use a private staging directory and a per-repository advisory
lock for publication/fetch. Reject suspicious existing repositories rather than
repairing arbitrary user-created Git configurations automatically.

D4 (2026-10-20, proposed): select property testing and real Git integration
rather than Verus for this slice. There is no custom ref reconciliation
algorithm: Git owns it. The only pure mapping is validated identity to two path
components. The verification section states its abstract obligations and why an
unbounded proof would not establish the filesystem/transport guarantee. If
implementation introduces a substantial pure reconciliation algorithm, return
to proof-sketch-first design and use Verus for any selected lemmas.

D5 (2026-10-20, expert-review revision): rename and Git ref transaction are
commit points. Indeterminate completion is distinct from known pre-commit
failure. The expert panel rejected rollback claims after timeout. A reusable
adapter receives a duration budget, not a construction-time deadline. Shared
composition and an example subprocess smoke test close the wiring evidence gap.
All six perspectives supported retaining the CLI and proportionate testing;
operational assurance takes precedence over a smaller runner.

## Outcomes & retrospective

Planning only. No runtime behaviour or completed roadmap status is claimed. The
implementor must record red/green evidence, gate logs, conformance checks, and
residual limitations here, and reconcile upstream documents before marking this
plan COMPLETE.

## Context and orientation

`crates/repovec-core/src/lib.rs` owns `RuntimePaths` and shared domain modules.
`crates/repovec-core/src/appliance/directory_layout/mod.rs` specifies directory
ownership/modes. `crates/repovecd/src/lib.rs` declares the runtime adapter
boundary. Existing examples are in `crates/repovecd/examples/`; OAuth ports are
in `crates/repovecd/src/github_device_flow/mod.rs`, while the command runner
and encrypted token loading are in `crates/repovecd/src/github_token_store/`.
Follow their injection patterns, without expanding the credential-transform
runner into a generic process framework. `repovecd` currently has no discovery
caller to wire here.

Create pure mirror types under `crates/repovec-core/src/git_mirror/` and the
runtime implementation under `crates/repovecd/src/git_mirror/`. Split identity,
errors, process execution, filesystem publication, and tests by responsibility.
Keep only genuinely shared fixtures in `crates/repovec-test-helpers`; feature
fixtures stay beside mirror tests.

## Conformance basis

Baseline: commit `7763a99f5dff25547a972888ed26d6381c465bb9`. No separate terms
of reference or mirror-specific ADR exists at this baseline. Local labels below
provide traceability; they are not invented upstream requirement IDs.

- C1: `docs/roadmap.md`, task 2.3.1, and
  `docs/repovec-appliance-technical-design.md`, “Worktrees and checkout
  layout”: canonical bare clone and incremental fetch → M2/M3 → real-Git ref
  comparison.
- C2: `docs/adr-001-repovec-directory-provisioning.md` and
  `docs/users-guide.md`, directory ownership contract → M1/M2 → traversal,
  symlink, ownership, and permission tests.
- C3: technical design, “Authentication: device flow”, and
  `docs/developers-guide.md`, environment access policy → M3 → credential
  injection, redaction, and child-environment tests.
- C4: user verification and testing requirements → M1–M4 → the obligation
  evidence below and sequential milestone gates.

Read `docs/rust-testing-with-rstest-fixtures.md` for fixtures,
`docs/reliable-testing-in-rust-via-dependency-injection.md` for boundary seams,
`docs/rstest-bdd-users-guide.md` for scenario binding, and
`docs/rust-doctest-dry-guide.md` for shared executable examples. Consult
`docs/complexity-antipatterns-and-refactoring-strategies.md` before extracting
helpers. `docs/ortho-config-users-guide.md` is the configuration reference;
this task does not add configuration precedence or require adopting that
library. Follow `docs/documentation-style-guide.md` for prose and ADRs.

Load `$codegraph-mcp`, `$hexagonal-architecture`, `$rust-router`, and
`$execplans` before implementation. Rust Router's `rust-verification` guidance
informs the assurance choice. Use `$firecrawl-mcp` for unresolved external
contracts; use `$verus` if the proof escalation described below is triggered.

## Behavioural contracts and abstract specification

Let an identity be a pair `(owner, repository)` of nonempty validated ASCII
components, normalized to lower case for GitHub's case-insensitive identity.
Reject separators, control characters, dot/dot-dot components, URL syntax, and
anything outside an explicit allowlist. Owner uses letters, digits, and
hyphens; repository additionally permits underscore and dot. Apply a local
safety bound of 100 ASCII bytes per component, leaving room for the storage
suffix within Linux component limits. This is an appliance input bound, not a
claim to validate every GitHub account-naming rule. Preserve a literal `.git`
suffix in a repository name: the storage suffix is appended, never stripped.
`name` and `name.git` therefore map to distinct directories.

Define `P(owner, repo) = [owner, repo + ".git"]`, relative to the trusted
mirror root. Define `R` as a finite map from advertised names in `refs/*` to
object identifiers during a successful Git transfer. Hidden server refs are
outside `R`. Symbolic refs contribute their advertised object identifiers;
preserving symbolic-ref topology is not promised. A local mirror state is
Absent, Valid(identity, refs), or Invalid. Transient staging state is never a
published mirror. Local objects not reachable from refs may remain after fetch.

Preconditions: the root is private and owned by the service; the identity and
production URL are validated; the writer holds the repository lock; the
upstream offers a successful transfer; and an existing mirror has validated
bare configuration, matching origin, and the intended refspec. A successful
create publishes Valid(identity, R). A successful fetch preserves repository
identity and storage location and replaces the mirrored ref map with R.
Repeated synchronization against unchanged R preserves that ref map, although
Git may update internal metadata. No byte-for-byte or zero-I/O claim is made.

Before publication, failed creation leaves the final destination absent.
Publication is the create commit point: after rename, reporting failure or
process death cannot imply absence. For fetch, the Git ref transaction is the
commit point. A known transaction rejection leaves refs unchanged; timeout or
lost completion after that point produces an indeterminate-completion error,
not a rollback claim. Ref changes are all-or-none under `--atomic`, while
downloaded objects and maintenance metadata may remain. Retry inspects the
existing mirror under its lock and fetches again; it never deletes a valid
mirror to resolve uncertain completion. Invalid destinations are rejected
without mutation. A network changing during fetch is not a linearizable global
snapshot: equality is to the refs Git successfully negotiated, not a later
independent network query.

The representation relation maps the canonical identity to its relative
components and maps `git for-each-ref` output to the abstract ref map. It does
not equate directory existence with validity. Successful exit status alone is
insufficient to identify a previously unknown directory as a managed mirror.

Candidate semantic arguments, stated before implementation, are: validated
components cannot introduce a parent/root component; component concatenation
with a fixed suffix is injective over canonical identities; staged publication
cannot expose a failed clone; and Git's forced mirror refspec plus pruning
implements replacement of advertised refs. These are proof sketches and
boundary obligations, not claims of machine-checked theorems. The last two
necessarily depend on external filesystem/Git behaviour.

## Interfaces and dependencies

Proposed names are `RepositoryIdentity`, `MirrorOutcome::{Created, Fetched}`,
`MirrorError`, and a domain-owned `MirrorStore` trait with
`synchronize(&self, identity: &RepositoryIdentity) -> Result<MirrorOutcome, MirrorError>`.
The runtime adapter receives its root, credential context, Git executable,
runner, monotonic clock, and timeout duration at construction. Credentials are
not part of identity, outcome, path, or public diagnostic formatting. The
shared bootstrap workflow owns orchestration through this port; avoid a
separate pass-through service whose only purpose is a fake-port test. The
composition example loads `AccessToken` and constructs the concrete adapter;
that adapter does not load tokens or invoke discovery itself.

Use one synchronous operation, consistent with the current blocking OAuth
adapters. Future async callers must schedule it off executor threads. A
`GitCommandRunner` seam accepts a structured command, child environment, and
deadline, and returns status plus bounded captured output. Each synchronize
call establishes a fresh ten-minute budget covering lock acquisition,
validation, and transfer; lock waiting is additionally capped at five seconds.
All subprocesses share that invocation deadline rather than resetting it. Allow
at most two seconds of termination/drain grace. Inject shorter budgets in tests
and cover adapter reuse and exhaustion across several commands. Retain at most
1 MiB stdout and 64 KiB stderr independently, continue draining both streams
after truncation, and return truncation flags. Metadata parsing fails closed on
truncated stdout. The runner owns child termination and reaping, including Git
transport descendants on timeout; use a dedicated process group and safe
`rustix` signalling. A descendant holding pipe handles cannot extend the drain
grace indefinitely. Do not expose infrastructure types through the domain port.

Use `/usr/bin/git` in production with arguments, never a shell string. Initial
creation uses `clone --mirror --no-local -- <url> <staging-path>`. Incremental
fetch uses
`--git-dir=<mirror> fetch --atomic --prune --no-write-fetch-head
origin +refs/*:refs/*`.
The plus permits non-fast-forward branch and tag updates. Disable
auto-maintenance during the bounded operation; do not add custom object pruning
or packing policy. Verify stored origin URL, bare flag, `remote.origin.mirror`,
and the single exact fetch refspec before each fetch. Reject conflicting
remotes, includes, URL rewrites, external object stores, and unexpected
repository extensions rather than executing their effects. Inspect
directory/config/object-path metadata without following symlinks first. Read
the local config explicitly with
`git config --file <config> --no-includes --null --list` in an isolated
non-repository working directory, with system/global config disabled. Allow
only the keys emitted by the managed clone and explicitly owned settings;
reject unknown keys before any repository-aware command. Check alternates and
Git-directory indirection without opening arbitrary external paths. Keep Git
template selection empty and disable automatic maintenance so managed config
stays auditable.

A production remote is constructed as `https://github.com/{owner}/{repo}.git`;
arbitrary user URLs, redirects, and credential-bearing URLs are rejected.
Authenticate using a child-only HTTP Authorization header via Git's
`--config-env` facility, encoding `x-access-token:<token>` as HTTP Basic
authentication. Never use `-c` with the secret value. Reset inherited HTTP
headers and credential helpers, disable prompts and redirects, retain TLS
verification, and disable hooks. Clear inherited Git configuration, tracing,
askpass, proxy, and repository-location variables using `Command::env_clear`;
explicitly supply the minimal required locale, executable search path, and
isolated configuration paths. The adapter must not log the child environment or
raw Git stderr. Same-UID/root process inspection is outside the credential
confidentiality boundary.

Use existing `thiserror`, `camino`, `cap-std`, `tempfile`, and `tracing`. Add
caret requirements for `base64`, `rustix`, `googletest`, and
`pretty_assertions` after checking their current compatible releases. Reuse
`rstest`, `rstest-bdd`, `insta`, and `proptest` from the workspace. Tests may
inject local file or loopback smart-HTTP transports through a test fixture
boundary; the production constructor must retain HTTPS/GitHub restrictions.

## Verification plan

V1 (identity/path): `rstest` tables and `proptest` in
`crates/repovec-core/src/git_mirror/tests.rs` exercise canonicalization,
rejection, relative component count, and injectivity modulo case. Generate
valid components directly, arbitrary invalid strings separately, and distinct
canonical pairs without heavy rejection. Include `a/b`, `..`, control bytes,
`name.git`, mixed case, and maximum accepted lengths. A mutation removing the
separator check or stripping `.git` must fail. This tests the executable
representation relation over broad inputs, without claiming an unbounded proof
of filesystem confinement.

V2 (publication and confinement): integration tests in
`crates/repovecd/tests/git_mirror.rs` create isolated real Git remotes and
private temporary roots. Exercise missing root, wrong ownership/mode where
portable, owner/repository symlinks, existing file/nonbare/wrong-origin
repositories, failed clone, retry, and two competing processes. Assert no
outside sentinel changes and no incomplete final mirror becomes visible. Inject
process failure after staging and before publication. Also inject failure after
rename and require a preserved valid mirror plus indeterminate completion. A
mutation that clones directly into the final destination must be detected. Root
ownership assumptions require a separate Linux appliance check; temporary-root
tests must not claim to prove deployment ownership.

V3 (faithful incremental fetch): use real Git to create branches, annotated and
lightweight tags, notes, divergent force updates, deletions, empty
repositories, and unchanged repeats. Compare sorted name/object-ID maps; assert
existing object-store identity survives fetch. A ref-lock conflict must leave
all refs unchanged, including any candidates for pruning, and return an error.
Omitting force or prune must fail named tests. Generate short sequences of ref
additions, updates, and removals with `proptest`, recording coverage classes
and shrunk counterexamples. Bound histories to 1–12 actions and 1–4 branches;
named tests cover empty and unchanged states independently. This is sampled
integration evidence, not an exhaustive Git proof.

V4 (authentication/process contract): fake-runner `rstest` cases inspect argv
and environment without printing secrets. A local smart-HTTP Git server checks
the header and returns authentication denial, a redirect, and interrupted pack
transfers. Real Git must clone/fetch with the synthetic credential and fail
safely when it is wrong. Inspect persisted config, tracing capture, errors, and
child arguments for raw and encoded token leakage. Exercise a hung child,
simultaneous stdout/stderr flooding, and a descendant retaining a pipe after
parent exit to verify bounded output, timeout, and reaping. Seed secret-bearing
stderr, inherited URL rewrites, repository-local HTTP overrides, config
includes, and object alternates as negative controls. Add a commit-then-hang
fake runner case: timeout must not claim unchanged refs. Use `googletest`
matchers for semantic results and `pretty_assertions` for ref-map/config
comparisons. No live token is used in automated tests.

V5 (behaviour and diagnostics): bind the following feature with `rstest-bdd` in
`crates/repovecd/tests/git_mirror_bdd.rs`, with feature text at
`crates/repovecd/tests/features/git_mirror.feature`. Use fallible fixtures and
real Git through the application port for happy-path scenarios. `insta`
snapshots cover stable, redacted `Display`/`Debug` error variants only where
both formats are exposed; no raw version-dependent Git output snapshots.

```gherkin
Feature: Maintain a bare repository mirror
  Scenario: Create a mirror for a repository
    Given an accessible repository with two branches and a tag
    And no mirror exists
    When the repository is synchronized
    Then a bare mirror exists at the canonical path
    And its advertised refs match the upstream

  Scenario: Fetch into an existing mirror
    Given a valid mirror of a repository
    And upstream has a forced branch update and a deleted tag
    When the repository is synchronized
    Then the existing mirror contains the new branch tip
    And the deleted tag is absent

  Scenario: Recover from a failed initial transfer
    Given no mirror exists
    And the upstream transfer fails
    When the repository is synchronized
    Then no final mirror is published
    And a subsequent successful synchronization can create it

  Scenario: Refuse an unrelated existing destination
    Given the canonical mirror path contains an unrelated repository
    When the repository is synchronized
    Then a typed destination error is returned
    And the unrelated repository is unchanged

  Scenario: Reject invalid credentials
    Given a repository requires authentication
    And the supplied credential is rejected
    When the repository is synchronized
    Then synchronization fails without exposing the credential
```

V1–V5 rely on Git's documented refspec/transaction semantics, normal Linux
filesystem rename and advisory-lock semantics, a private service-owned tree,
and the upstream's advertised object availability. They do not prove Git, TLS,
the kernel, or protection against malicious same-UID mutation. Witnesses include
`owner/repo` for valid identity, an empty upstream for empty state, and a
two-ref conflicting transaction for failure atomicity. Missing scenario
classes, excessive generator filtering, or surviving negative controls leave
the corresponding obligation undischarged.

Verus is not warranted for diagnostics, command construction, subprocess I/O,
or the delegated Git transaction. There is no Verus milestone in this design.
If a custom pure state/ref reconciliation kernel becomes necessary, stop and
amend this plan: specify its abstract maps and transitions, premises,
postconditions, representation relation, and candidate lemmas first. Require
substantive Verus proofs over the executable kernel (or an explicitly checked
refinement), with satisfiable premises and negative controls. Add a pinned
`make verify-mirrors` target invoking Verus and make it an explicit milestone
acceptance gate alongside `make check-fmt`, `make lint`, and `make test`. Never
use `assume`, `admit`, external proof bodies, or equivalent shortcuts. Keep
unproved theorem statements in documentation until implementation exposes the
actual verification conditions. Integration tests remain mandatory.

## Milestones and plateaus

### M1: contracts and test seams

Record the installed Git version and verify support for `--config-env`,
`clone --mirror`, and atomic pruning fetch. Establish the offline smart-HTTP
fixture early: a Python standard-library HTTP server invokes `git http-backend`
with an isolated CGI environment and fixture repository root, validates a
synthetic Authorization header, and supplies denial/redirect/abort cases. The
fixture starts on an ephemeral loopback port and owns/reaps its children.
Python is already required by the test gate; verify `http-backend`
availability. No third-party server or live GitHub credential is required.

After approval, establish a passing baseline, then add the smallest failing
identity test before implementing validation. Add the pure identity, outcomes,
errors, port, and port-contract tests with a fake implementation. Extend
core/runtime `lib.rs` exports and Cargo dev-dependencies together. Deliver V1
and the no-I/O port contract, with `rstest`, `googletest`, `pretty_assertions`,
and `proptest`. Capture the expected red failure, green rerun, and refactor
gates. C2/C4 are advanced; clone/fetch remain absent. Commit only the coherent
green state; rollback is a normal commit revert with no persistent data
migration.

### M2: private initial publication

Start with failing V2 and creation/recovery BDD tests, then implement the
runtime runner, root validation, per-identity lock, staging clone, validation,
and publication. Use a lock outside the published `.git` directory so creating
or publishing the repository cannot replace the lock inode. Create staging
under the same owner directory and keep it private. Never unlink a lock file
while writers may hold it. Inspect the final destination again under the lock.
Publish by same-filesystem no-replace rename only after successful clone and
validation. Reject an existing destination; do not replace it. Publication is
the final fallible state-changing step: best-effort logging/cleanup cannot
convert a known successful rename into an ordinary creation failure. If
completion cannot be observed, preserve the mirror and report indeterminate
completion when possible.

Deliver C1 creation and C2 confinement evidence, including two-process races.
Authentication/fetch remain for M3; a local-transport integration fixture is
the acceptance surface for this plateau. On failure, remove only the staging
directory created by this invocation; retry under the lock. Retain crash-left
staging safely for diagnosed cleanup rather than deleting paths by a broad name
pattern. Run all milestone gates and commit.

### M3: incremental fetch and authenticated transport

Add failing V3/V4 tests and the remaining BDD scenarios before implementation.
Implement existing-mirror validation, atomic forced/pruning fetch, secret
injection, and bounded process lifetime. Typed errors distinguish invalid
identity/destination, lock contention, unavailable Git, timeout, and failed Git
operation, including indeterminate completion after a possible commit point. Do
not infer precise authentication causes from localized stderr when Git supplies
only a generic failure; retain a sanitized operation/status category. Record
canonical identity, operation phase, lock-wait duration, execution duration,
outcome, and sanitized failure category through tracing without raw stderr. Do
not introduce repository-valued metric labels or a metrics subsystem.

Deliver C1 fetch and C3 authentication evidence. A repeated call returns
Fetched even when no ref changes; it does not imply downloaded bytes. Retry
failed fetch in place; never delete/reclone a valid mirror as automatic
recovery. Gates and real-Git failure-atomicity tests must pass before commit.
Reconciler and worktree delivery remain explicitly outside the plateau.

### M4: executable workflow, documentation, and completion

Add `crates/repovecd/examples/sync_git_mirror.rs`: parse explicit owner/repo
inputs, use appliance paths by default, load the encrypted token at the
composition root, invoke the application port, and report a redacted result.
Put the callable composition in `crates/repovecd/src/git_mirror/bootstrap.rs`
with injected credential loader and validated transport settings; the example
and end-to-end harness must both call it. Keep test transport construction out
of the production example CLI. Keep environment access at the composition root;
fixture code injects values. Provide an end-to-end harness that executes this
same wiring with temporary paths, a fake credential loader, and the local
smart-HTTP server. Cover first clone, second fetch, restart/reopen, invalid
credentials, and failure retry. Run those lifecycle tests in process through
the shared bootstrap function. Also launch the actual example as a subprocess
to verify argument parsing, usage/error exit status, and redacted output; this
smoke test does not need a production URL bypass. This tests persistence and
transport end to end without provisioning secrets or contacting GitHub in
normal CI.

The baseline has packaging assets but no cloud-init implementation; roadmap
6.3.1 owns that later work. Document `/usr/bin/git` and its HTTPS transport as
an explicit host prerequisite, and test the typed unavailable-Git outcome. Run
the example smoke check on the target Linux environment and record its Git
version. Add a dependency note to roadmap 6.3.1 so future provisioning installs
Git; do not claim that this task provisions a complete appliance.

Update `docs/repovec-appliance-technical-design.md` and add the next available
`docs/adr-NNN-git-mirror-synchronization.md` for transport, publication,
credentials, ref ownership, and verification boundaries. Reference the ADR from
the design and `docs/contents.md`. Document internal interfaces in the
technical design's mirror component section and reuse/test conventions in
`docs/developers-guide.md`. Update `docs/users-guide.md` with path, Git
prerequisite, permissions, example invocation, failure/retry behaviour, and
explicitly pending automatic reconciliation. Update the repository-layout
section if module structure changes.

Reconcile all obligations and source-document decisions, run the full gates,
and mark only roadmap item 2.3.1 checked with a `Done:` explanation. Update
this plan to COMPLETE only when evidence exists. Commit the coherent delivery
state; do not claim 2.3.2 or 2.3.3 complete.

At every milestone record: assigned C/V obligations satisfied or remaining;
continued inward dependencies; upstream assumptions still valid; no unapproved
API/dependency/trust-boundary or persistent-format change; and current trace
links. A departure is a proposed deviation requiring acceptance. No milestone
needs a compatibility shim; there is no existing mirror API or deployed mirror
metadata format to migrate.

## Concrete steps and acceptance commands

Run from the repository root on the feature branch. The focused targets below
are planned additions, not commands claimed to pass at the planning baseline.
Use `set -o pipefail` so `tee` cannot hide failures. Preserve log paths and
summaries in Progress; do not commit temporary logs.

```bash
cargo test -p repovec-core git_mirror
cargo test -p repovecd --test git_mirror
cargo test -p repovecd --test git_mirror_bdd
cargo test -p repovecd --test git_mirror_e2e
```

For each focused test, first record a missing-API compile failure or assertion
failure that specifically identifies the absent behaviour; an infrastructure
failure does not count as red evidence. After implementation require passing
assertions with no ignored new cases. Unit and behavioural tests run both
before and after each affected change.

Delegate full milestone gates to `scrutineer`, sequentially, with logs under
`/tmp/231-M<N>-<gate>.out`. Run `make build`, `make check-fmt`, `make lint`, and
`make test` after each major milestone, with `make fmt` first if formatting
changed. Documentation changes additionally require `make markdownlint` and
`make nixie`. `make markdownlint` includes spelling generation; never edit
`typos.toml` manually. Unrelated generated spelling changes require a separate
base PR under repository policy. Follow compiler admission; do not create a
private Cargo cache or bypass wrappers.

An authorized operator can later run the example against an accessible GitHub
repository, then inspect the result:

```bash
cargo run -p repovecd --example sync_git_mirror -- OWNER REPO
/usr/bin/git --git-dir=/var/lib/repovec/git-mirrors/owner/repo.git rev-parse --is-bare-repository
/usr/bin/git --git-dir=/var/lib/repovec/git-mirrors/owner/repo.git for-each-ref
```

Expected observations are Created on the first call, Fetched thereafter, `true`
for the bare check, and refs matching the successful transfer. The example's
exact rendering must be documented when implemented. Normal tests must exercise
this workflow offline; live GitHub validation is optional and must never expose
a real token in logs.

## Idempotence and recovery

A stable repository identity selects one path and one lock. Concurrent callers
serialize; the second inspects the published mirror and fetches rather than
recloning. Lock contention has a bounded typed outcome rather than an unbounded
wait. A process exit releases the advisory lock; the lock file remains. Failure
before publication leaves no final repository; failure after a possible commit
point requires inspection on retry. A crash may leave private staging or newly
downloaded unreachable objects; neither is a valid published synchronization
result.

Existing invalid paths require operator diagnosis, not automatic deletion. A
valid mirror is never discarded to recover a fetch failure. Reverting code must
preserve source data and credentials. Power-loss durability of every Git object
is not promised beyond Git/filesystem guarantees; document this limit rather
than equating rename atomicity with durable storage.

## Artefacts and research notes

Official Git manuals were inspected through Firecrawl on 2026-10-20:

- [git-clone](https://git-scm.com/docs/git-clone): `--mirror` implies bare
  layout and maps all refs; `--no-local` exercises transport for local tests.
- [git-fetch](https://git-scm.com/docs/git-fetch): `--atomic` groups ref
  updates, and pruning follows the refspec, including mirrored tags.
- [git](https://git-scm.com/docs/git): `--config-env` supplies a per-command
  configuration value without putting the secret itself in argv.

The broad `git-config` extraction was incomplete. A follow-up read of Git
upstream
[HTTP configuration documentation](https://github.com/git/git/blob/master/Documentation/config/http.adoc)
confirmed empty `http.extraHeader` resets the header list,
`http.followRedirects=false` rejects redirects, and `http.sslVerify` defaults
to true. Supply these explicitly and test the real boundary. URL-specific
configuration can override generic HTTP policy, so existing mirror config
validation must reject such unowned keys before any authenticated operation.

Revision note (2026-10-20): initial draft joins repository reconnaissance,
abstract contracts, proportionate verification, and four implementation
plateaus. Implementation remains subject to explicit approval.

Revision note (2026-10-20, expert review): clarified commit points and
indeterminate completion, shared per-call budgets, bounded draining, safe
configuration inspection, and deployment/E2E prerequisites. These refine
acceptance without expanding into worktrees or reconciliation scheduling.
