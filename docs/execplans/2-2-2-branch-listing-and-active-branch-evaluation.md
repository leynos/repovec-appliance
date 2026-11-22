# Evaluate active branches for every accessible repository (2.2.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (awaiting design review and user approval)

Branch: `2-2-2-branch-listing-and-active-branch-evaluation`

Roadmap item: `2.2.2` in `docs/roadmap.md`.

## Purpose / big picture

repovec-appliance indexes GitHub repositories so that a remote agent can search
them through a Model Context Protocol (MCP) endpoint. The appliance models a
repository as a grepai *workspace* and each indexed branch as a *project* in
that workspace. Indexing every branch forever is too expensive, so the
technical design defines an *active branch policy*: the default branch is
always indexed, branches pushed recently are indexed, branches referenced by
open pull requests may be indexed, and a per-repository cap evicts the least
recently used branches beyond it.

Roadmap item 2.2.1 (complete) lets `repovecd` list every repository a supplied
credential can see. Nothing yet lists branches, gathers evidence of branch
activity, applies the policy, or records which branches should be active.
Roadmap item 2.2.3 (the reconciler) needs exactly that record as its *desired
state*: the set of active branches per repository, which it will compare
against the mirrors, worktrees, and indexer units that actually exist.

After this work an operator can run one command against GitHub:

```sh
cargo run -p repovecd --example evaluate_active_branches -- \
  --user-store /etc/repovec --state-file /tmp/active-branches.json
```

and observe, on standard output, one line per repository naming its active
branches, for example:

```plaintext
octo-org/widgets: main, feature/search (2 evicted)
octo-org/archive: unavailable (forbidden)
```

and, in `/tmp/active-branches.json`, a versioned, atomically written snapshot
of the desired state that the reconciler will consume. A library caller obtains
the same result as a typed `DesiredState` value from
`repovec_core::active_branches::discover_desired_state`.

Two promises matter more than the happy path:

- A repository whose branch evidence could not be observed completely is
  recorded as *unavailable*, never as having no active branches. The reconciler
  must never read a failed observation as an instruction to delete worktrees.
- A failure that affects the whole credential (authentication rejected, rate
  limited, transport failure, or a failed repository listing) publishes
  nothing, so the previously stored snapshot stays authoritative.

Existing behaviour is preserved: the `list_github_repositories` example, the
repository listing port, and every 2.2.1 error category keep their contracts.
One deliberate change tightens an existing representation:
`RepositorySummary::default_branch` becomes a validated `BranchName` instead of
an unvalidated string, so an invalid default branch fails the repository
listing with `invalid_response` instead of flowing onward unchecked.

## Constraints

- Implementation begins only after the user explicitly approves this plan.
- Keep domain types, the policy, the selection kernel, the discovery use case,
  and the driven ports in `repovec-core`, with no HTTP, JSON, filesystem,
  environment, or clock access there. Put the GitHub adapter, the snapshot file
  adapter, and the one-shot composition in `repovecd`. Dependencies point
  inward; adapters never call one another.
- Every GitHub REST listing goes through the crate-private
  `repovecd::github_rest` traversal (ADR 0002 and developers' guide section
  9.2). Do not add a second traversal, a trait object over traversals, retries,
  backoff, or scheduling.
- Do not implement the periodic scheduler or the desired-versus-actual diff
  (2.2.3), mirrors or worktrees (2.3), webhooks (2.4), indexer lifecycle (3.x),
  TUI configuration (5.3), or GitHub App token issuance. Do not add a daemon
  configuration file or adopt `ortho-config` in this item (see decision D9).
- Do not filter repositories by archive state, fork state, privacy, or owner
  type; the technical design assigns no such policy to this item.
- Honour the environment access policy (developers' guide section 8 and
  `clippy.toml`). Read no environment variable. The wall-clock instant is read
  once, at the composition root, and passed inward as a value.
- Keep credentials, branch names, repository names, and response bodies out of
  logs and diagnostics. Telemetry carries numeric repository identifiers,
  counts, stable error categories, and durations only. Standard output may name
  repositories and branches, as the 2.2.1 command already does.
- Keep the 400-line code-file limit, the module-level `//!` comment rule, the
  `expect`-only-in-tests rule, item-scoped `#[expect(..., reason = ...)]` only,
  and custom messages on every assertion (`AGENTS.md`).
- Authorized dependency changes, effective only on plan approval:
  - runtime: `time` `0.3` (caret, currently `0.3.53` in `Cargo.lock`) with
    default features off and the `std` and `parsing` features, in
    `repovecd` only, for RFC 3339 timestamps (decision D6);
  - development: `googletest` and `pretty_assertions` are already workspace
    dependencies and may be added to `repovec-core`'s dev-dependencies;
    `serde`/`serde_json` may be added to `repovecd` uses that already depend
    on them; no other crate may be added.
  - verification tooling: the pinned Verus release and the
    `rust-prover-tools` commit pin, both outside the Cargo graph (decision
    D11).
- Interface changes are authorized only as listed in
  `Interfaces and dependencies`. All are pre-1.0, application-internal surfaces
  with no external consumer, so callers change together and no compatibility
  layer is added.
- The snapshot file is a new persisted format. Its schema carries an explicit
  version from the first release (decision D8); no earlier deployed format
  exists, so no migration is required.
- Do not edit completed ExecPlans; they are historical documents.
- Run formatting, format checking, lint, and tests sequentially, through the
  Makefile targets, logging through `tee` with `set -o pipefail`. Do not bypass
  compile admission, create an isolated Cargo cache, or kill other agents'
  processes.

If satisfying the objective requires violating a constraint, stop, record the
conflict in `Decision log`, set the status to `BLOCKED`, and escalate.

## Tolerances (exception triggers)

Budgets exclude this ExecPlan file and every other file under
`docs/execplans/`. The comparison base is the merge base with `main`
(`git merge-base HEAD origin/main`; `f4da025` at drafting). Counts use net
lines (additions minus deletions) per category from:

```sh
base=$(git merge-base HEAD origin/main)
git diff --numstat "$base" -- . ':!docs/execplans/**' > /tmp/2-2-2-numstat.txt
```

Classification is whole-file by path:

- Code: `*.rs` (production, tests, Verus proofs, examples), `*.feature`,
  `*.py`, `Makefile`, and insta `*.snap` files.
- Configuration: `Cargo.toml`, `Cargo.lock`, `.config/**`, `tools/**` except
  `*.md`, `typos.local.toml`, and `.github/**`. `typos.toml` is generated and
  excluded, but its regeneration must be committed separately if it changes.
- Documentation: `*.md` outside `docs/execplans/`.

Budgets, each enforced independently:

- Files: stop and escalate above 95 changed non-ExecPlan files.
- Code: stop above 11,500 net lines in total; within that, stop above 3,600
  net lines of production Rust (non-test files under `crates/*/src`, excluding
  `tests.rs`, `tests_*.rs`, `*_tests.rs`, `test_support.rs`, and `tests/`
  directories) and above 1,300 net lines under `verus/`.
- Configuration: stop above 300 net lines (the `Cargo.lock` entry for `time`
  and its already-locked dependants is expected to be under 40 lines).
- Documentation: stop above 1,200 net lines.
- Interface: stop if any public signature beyond `Interfaces and dependencies`
  must change.
- Dependencies: stop if any crate beyond the `Constraints` list is needed.
- Architecture: stop if `github_rest` continuation confinement must accept
  anything beyond the canonical per-repository path and a single profile-named
  cursor parameter (decision D3), or if the policy semantics in decision D4
  cannot be implemented as specified.
- Verification: stop if a planned Verus lemma cannot be proved without
  `assume`, `admit`, `external_body`, or a new axiom after three focused
  attempts, and record the open goal.
- Iterations: stop if a focused test still fails after five fix attempts.
- Time: stop if any milestone exceeds two working days of elapsed effort.
- Ambiguity: stop if GitHub's live behaviour contradicts a recorded axiom
  (`AXIOM-*` below).

These numbers come from measured history. Roadmap item 2.2.1 recorded no
numeric budget; measured afterwards (`git diff --numstat ffacb2f^ ffacb2f`), it
changed 81 files: 64 Rust files with 10,845 net lines (about 3,900 production,
6,975 test), 7 Markdown files with 466 net lines outside ExecPlans, and about
150 configuration lines. Its scope grew from "endpoint-specific" to a shared
traversal mid-plan. Roadmap item 1.3.3 set 24 files and about 1,000 net lines
excluding tests, landed near 1,700 net lines, and needed a user-approved
extension; the miss was in test and asset code that its budget excluded. Items
1.2.2 (12 files, 650 lines), 1.2.3 (14 files, 850 lines), and 2.1.1 (18 files,
900 lines) also excluded tests and recorded no increase. This item's estimate,
traced in `Context and orientation`, is about 2,900 production lines, 6,300
test lines, 800 proof lines, 150 contract-test lines, and 900 documentation
lines. The budgets add roughly 20 per cent contingency and, unlike the earlier
plans, include tests so that the dominant cost is visible.

## Risks

- Risk: GitHub's per-repository Link continuations use the canonical
  `/repositories/{id}/...` path rather than `/repos/{owner}/{repo}/...`.
  Severity: high. Likelihood: certain (observed). Mitigation: request the
  canonical path from the first page (decision D2), so continuation path
  confinement holds unchanged; protocol tests pin it.
- Risk: the repository activity endpoint paginates with an opaque `after`
  cursor, which `github_rest` currently rejects because only `page` may vary.
  Severity: high. Likelihood: certain (observed). Mitigation: a profile-named
  continuation parameter (decision D3), amending ADR 0002 through ADR 0003.
- Risk: the documented activity example (`pushed_at`, `push_type`, `pusher`)
  disagrees with live responses (`timestamp`, `activity_type`, `actor`).
  Severity: medium. Likelihood: certain (observed). Mitigation: decode the
  observed fields, record `AXIOM-activity-shape`, and fail the repository as
  `invalid_response` if the shape changes.
- Risk: the activity endpoint silently ignores an unknown `time_period` value
  (observed: `time_period=fortnight` returned `200`). Severity: medium.
  Likelihood: certain. Mitigation: map the window to a period only through a
  closed enumeration, and filter by timestamp client-side regardless.
- Risk: request volume. Each repository costs up to three traversals, so 1,000
  repositories cost at least 3,001 requests against a 5,000-per-hour primary
  limit. Severity: medium. Likelihood: medium. Mitigation: skip the pull
  request traversal when the policy disables it; classify rate limiting as
  credential-wide so a run stops at the first rate-limit response; record a
  whole-scan budget as a 2.2.3 concern.
- Risk: Verus tooling is new to this repository. Severity: medium. Likelihood:
  medium. Mitigation: milestone `EP-M1` lands only the pinned toolchain, the
  Makefile targets, contract tests, and the abstract model before any
  production code depends on the proof; the existing `prover-tools` CLI is on
  the host and `chutoro` provides estate precedent.
- Risk: the proof covers an abstract model, not the shipped Rust. Severity:
  high. Likelihood: medium. Mitigation: shape the production kernel as "filter,
  sort by a derived-`Ord` rank key, take a prefix" so that the only unproved
  steps are documented standard-library contracts (`AXIOM-std-sort`,
  `AXIOM-derived-ord`), and close the gap with the bridge tests in
  `Verification plan`.
- Risk: wall-clock skew between the appliance and GitHub. Severity: low.
  Likelihood: low. Mitigation: windows are whole days; record
  `AXIOM-synchronized-clock`; reject a clock before the Unix epoch.
- Risk: a branch name valid in Git is unsafe as a filesystem path in 2.3.2
  (case folding, Unicode normalization, component length). Severity: medium.
  Likelihood: low. Mitigation: `BranchName` enforces Git's own refname rules
  here; filesystem mapping remains a named 2.3.2 risk, recorded in the
  technical design.
- Risk: `Cargo.lock` churn from `time` features. Severity: low. Likelihood:
  low. Mitigation: `time` is already locked; add only `std` and `parsing`.

## Progress

- [x] (2026-11-22 19:05Z) Branch renamed and Lody session titled.
- [x] (2026-11-22 19:40Z) Reconnaissance (four Wyvern agents) and live GitHub
  probes completed.
- [x] (2026-11-22 20:30Z) Initial draft written.
- [ ] Community-of-experts design review applied.
- [ ] Plan approved by the user.
- [ ] `EP-M1` Verus on-ramp and proved selection model.
- [ ] `EP-M2` Core domain, kernel, and policy evaluation.
- [ ] `EP-M3` GitHub evidence adapter and traversal amendments.
- [ ] `EP-M4` Discovery use case and desired-state snapshot store.
- [ ] `EP-M5` One-shot composition, end-to-end, and behavioural evidence.
- [ ] `EP-M6` Documentation closure and roadmap completion.

## Surprises & discoveries

- Observation: every per-repository listing (`branches`, `pulls`, `activity`)
  returns `Link` continuations on `/repositories/{numeric id}/...` even when
  the first request used `/repos/{owner}/{repo}/...`. Evidence: probe P1 and P2
  in `Artefacts and notes`. Impact: the existing path confinement would reject
  page two; decision D2.
- Observation: the activity endpoint paginates with `after=<cursor>` and also
  advertises `rel="prev"` with `before=<cursor>`. Evidence: probe P3. Impact:
  decision D3.
- Observation: live activity records carry `timestamp`, `activity_type`, and
  `actor`; the documentation example shows `pushed_at`, `push_type`, and
  `pusher`. The shape is identical under API versions `2022-11-28` and
  `2026-03-10`. Evidence: probes P3 and P4. Impact: `AXIOM-activity-shape`.
- Observation: an unrecognized `time_period` value is ignored rather than
  rejected. Evidence: probe P5. Impact: closed `ActivityPeriod` enumeration and
  mandatory client-side filtering.
- Observation: one quarter of this repository's own activity held 622 records
  across five activity types (`branch_creation` 159, `branch_deletion` 143,
  `force_push` 83, `pr_merge` 110, `push` 127). Evidence: probe P6. Impact:
  activity limits in decision D10 must admit thousands of records per
  repository.

## Decision log

- D1. Decision: deliver a library use case plus a one-shot example
  (`evaluate_active_branches`), mirroring 2.2.1, rather than a daemon loop.
  Rationale: the scheduler is 2.2.3; the example is the operator-observable
  acceptance surface and the end-to-end test drives the same composition
  in-process. Date/Author: 2026-11-22, planning agent.
- D2. Decision: per-repository requests use GitHub's canonical
  `/repositories/{id}/{suffix}` path, built from the validated numeric
  `RepositoryId`. Rationale: GitHub's own continuations use this path, so the
  unchanged "same path as the first page" confinement accepts them; numeric
  identifiers also remove any path-injection concern from owner or repository
  names, which `RepositoryOwner` does not restrict to URL-safe characters. The
  rejected alternative, accepting an alias path set in confinement, widens the
  trust boundary of every listing. Date/Author: 2026-11-22, planning agent.
- D3. Decision: `EndpointProfile` names its continuation parameter:
  `page` for numbered listings, `after` for the activity listing. Confinement
  lets only that one parameter vary and still rejects every other added,
  dropped, repeated, or changed parameter, including `before`. Recorded in ADR
  0003, which amends ADR 0002. Rationale: smallest change that admits cursor
  pagination while keeping one traversal. Date/Author: 2026-11-22, planning
  agent.
- D4. Decision: policy semantics.
  - The candidate set is the observed branch list, deduplicated by name
    (first observation wins).
  - A branch is *pinned* when its name equals the repository's default
    branch. At most one branch is pinned.
  - A branch has *recent activity* when its latest qualifying activity
    instant `t` satisfies `t >= now - window`. Qualifying activity types are
    `push`, `force_push`, `pr_merge`, `merge_queue_merge`, and
    `branch_creation`; `branch_deletion` and unrecognized types never qualify.
  - A branch is *pull-request referenced* when the policy includes pull
    requests and an open pull request's head repository identifier equals
    this repository's identifier and its head branch equals the branch name.
    Pull requests from forks never qualify, because their head branch lives in
    another repository.
  - A branch is *admissible* when it is pinned, recent, or pull-request
    referenced.
  - "LRU eviction" ranks admissible branches by pinned first, then by most
    recent qualifying activity (a branch with no activity in the fetched
    period ranks below every branch with activity), then by branch name in
    ascending byte order. The first `cap` branches are active; the remaining
    admissible branches are *evicted* and reported.
  - The evaluation is stateless: it depends only on the observation, the
    policy, and `now`.
  Rationale: GitHub records when a ref was last updated, not when the appliance
  last used a branch, so "least recently used" is interpreted as least recently
  active. Statelessness makes evaluation idempotent and provable; hysteresis
  against churn needs the previous desired state and is left to 2.2.3 if
  measurement shows it is needed. Date/Author: 2026-11-22, planning agent.
  Status: requires user confirmation at approval.
- D5. Decision: default policy values are a 14-day window, a cap of 10 active
  branches per repository (the default branch counts towards the cap), and pull
  requests included. The window accepts 1 to 364 days and the cap 1 to 1,000.
  Rationale: the technical design gives no values; these keep a typical team
  repository fully indexed while bounding pathological ones. Date/Author:
  2026-11-22, planning agent. Status: requires user confirmation at approval.
- D6. Decision: parse GitHub timestamps with the `time` crate's RFC 3339
  parser in the adapter and carry `UnixSeconds(u64)` into the domain.
  Rationale: date arithmetic is a classic source of defects; `time` is already
  in the lockfile; the domain stays integer-only, which suits Verus. The
  rejected alternative, a handwritten fixed-format parser, would need its own
  calendar proof. Date/Author: 2026-11-22, planning agent.
- D7. Decision: classify evidence failures. `authentication_rejected`,
  `rate_limited`, and `transport_failure` are credential-wide: discovery stops
  and publishes nothing. Every other category (`forbidden`, `unexpected_status`,
  `invalid_response`, `invalid_continuation`, `cycle_detected`,
  `resource_limit_exceeded`, `invalid_input`) is repository-scoped: that
  repository is recorded as unavailable with its category and discovery
  continues. Rationale: a credential-wide failure would repeat for every later
  repository and a snapshot full of unavailable entries would carry no
  information; a repository-scoped failure must not hide the other
  repositories. Date/Author: 2026-11-22, planning agent.
- D8. Decision: store the desired state as a JSON snapshot at
  `RuntimePaths::active_branches_snapshot()`
  (`/var/lib/repovec/active-branches.json`), written atomically with mode
  `0600` beside existing data under the `repovec`-owned `0700` data root,
  versioned with `"schema_version": 1`. The core defines a `DesiredStateStore`
  port; `repovecd` supplies the file adapter. Recorded in ADR 0004. Rationale:
  the reconciler in 2.2.3 needs the last known desired state when GitHub is
  unreachable at start-up, and the operator needs an inspectable record. Using
  the existing data root avoids changing the 1.3.3 directory contract,
  `tmpfiles.d`, and provisioning tests. Core stays free of `serde`; the adapter
  owns the wire format. Date/Author: 2026-11-22, planning agent.
- D9. Decision: do not adopt `ortho-config` or a daemon configuration file in
  this item. `ActiveBranchPolicy` is a validated value with defaults; the
  one-shot command exposes `--recent-days`, `--max-branches`, and
  `--without-pull-requests`. Rationale: no crate depends on `ortho-config` yet;
  the first daemon configuration surface arrives with the 2.2.3 scheduler,
  which also needs an interval, and the TUI (5.3) owns policy editing. The
  policy constructor is the seam that configuration will call. Date/Author:
  2026-11-22, planning agent.
- D10. Decision: per-endpoint traversal limits. Branch and pull request
  listings use `ListingLimits::defaults()` with pages capped at 100 (10,000
  records); the activity listing caps pages at 200 (20,000 records). Reaching a
  limit is `resource_limit_exceeded`, which is repository-scoped. Rationale:
  2.2.1 left whole-scan budgets to this item and 2.2.3; per-endpoint caps bound
  each repository without inventing a scheduler budget. Date/Author:
  2026-11-22, planning agent.
- D11. Decision: prove the selection model with Verus (`EP-M1`), bridge it to
  the production kernel with tests, and do not use Kani. Rationale: the
  selection invariants (bounded size, pinned retention, eviction order,
  permutation invariance, cap monotonicity) are contractual and quantified over
  unbounded sequences, which is Verus's remit; the developers' guide, section
  9.6, names a pure policy kernel as the trigger for revisiting formal proof.
  Kani would add a second prover toolchain for obligations that are either
  covered by the Verus model or reduce to documented standard-library
  contracts. Date/Author: 2026-11-22, planning agent.
- D12. Decision: `RepositorySummary::default_branch` becomes
  `Option<BranchName>`, validated in the 2.2.1 wire conversion. Rationale: the
  policy compares the default branch with validated branch names; validating
  once at the boundary removes a stringly typed comparison. All callers change
  together; no compatibility accessor is kept. Date/Author: 2026-11-22,
  planning agent.
- D13. Decision: `now` is a wall-clock `UnixSeconds` read once from
  `SystemTime::now()` at the composition root. Durations in telemetry continue
  to use `monotony`'s `MonotonicClock`. Rationale: the policy compares against
  GitHub's wall-clock timestamps, which a monotonic clock cannot express. The
  developers' guide gains an explicit carve-out distinguishing timestamp
  comparison from elapsed-time decisions. Date/Author: 2026-11-22, planning
  agent.

## Outcomes & retrospective

Not started. Complete at each milestone and before marking the plan `COMPLETE`,
reconciling every discovery against the `Conformance basis`.

## Context and orientation

Inspected base revision: `f4da025` (`main`, 2026-11-22). Relocate symbols with
`grep -rn '<symbol>' crates/` rather than line numbers.

The workspace (`Cargo.toml`) has seven crates. Two matter here.
`crates/repovec-core` holds domain logic and ports; `crates/repovecd` holds the
daemon, its GitHub adapters, and one-shot compositions.
`crates/repovec-test-helpers` holds shared test fixtures, including the
scripted loopback HTTP server.

Repository listing (2.2.1) is the pattern to follow:

- `crates/repovec-core/src/repository_listing/mod.rs` defines the port
  `pub trait RepositoryListing { fn list_repositories(&self) ->
  Result<Vec<RepositorySummary>, RepositoryListingError>; }`.
  A failure returns no partial list.
- `crates/repovec-core/src/repository_listing/summary.rs` defines
  `RepositorySummary` (`id()`, `owner()`, `name()`, `is_private()`,
  `default_branch() -> Option<&str>`); `identity.rs` defines `RepositoryId`
  (non-zero `u64`), `RepositoryOwner`, and `RepositoryName` (non-empty, no
  control characters, otherwise unrestricted); `error.rs` defines
  `RepositoryListingError` with variants `InvalidInput`,
  `AuthenticationRejected`, `Forbidden`, `RateLimited`, `TransportFailure`,
  `UnexpectedStatus`, `InvalidResponse`, `InvalidContinuation`,
  `CycleDetected`, and `ResourceLimitExceeded`, each with a stable `category()`
  string.
- `crates/repovecd/src/github_rest/` is the crate-private traversal.
  `profile.rs` defines
  `EndpointProfile<T> { path: &'static str, fixed_query, items: PageItems }`.
  `traversal.rs` defines `TraversalRunner` and
  `Traversal<T> { records, pages, bytes }`. `link/mod.rs` confines a
  continuation to the initial origin and path and, in `ensure_query`, lets only
  `page` vary. `limits/mod.rs` defines `ListingLimits` (defaults: 10,000 pages,
  1,000,000 records, 8 MiB per body, 64 MiB total, 16 KiB Link value, 16 KiB
  continuation identity). `status/mod.rs` classifies statuses; only `200` is a
  page. `test_support.rs` exposes seams behind the `test-support` feature.
- `crates/repovecd/src/github_repository_client/mod.rs` defines
  `GitHubRepositoryClient` (`new`, `with_http_client`), the two repository
  profiles, and the `RepositoryListing` implementation with `tracing` events
  `"repository listing completed"` and `"repository listing failed"`. `wire.rs`
  holds `RepositoryWire`.
- `crates/repovecd/src/repository_listing_command/` is the one-shot
  composition: `ListingMode`, `UserTokenLoader`, `ListingRun`, `run_listing`,
  `RunStatus` (exit codes 0, 3, 4, 5, 6; usage errors 2), and `cli::run`.
  `crates/repovecd/examples/list_github_repositories.rs` supplies process
  streams and the encrypted token loader.
- `crates/repovecd/src/github_token_store/atomic_write.rs` holds
  `pub(super) fn write_atomically(root: &Dir, filename: &str, contents:
  &[u8]) -> io::Result<()>`
  (create-new temporary file, mode `0600`, write, `sync_all`, rename, directory
  `fsync`).
- `crates/repovec-core/src/lib.rs` defines `RuntimePaths` (`config_root`
  `/etc/repovec`, `data_root` `/var/lib/repovec`, with `git_mirrors_root`,
  `worktrees_root`, `grepai_root`, `qdrant_storage_root`).
  `crates/repovec-core/src/appliance/directory_layout/mod.rs` `layout_contract`
  makes `data_root` `0700 repovec:repovec`.
- Tests: `crates/repovecd/tests/` holds protocol, property, behavioural
  (`repository_listing_bdd.rs` with
  `tests/features/repository_listing.feature`), and in-process end-to-end tests
  (`repository_listing_e2e.rs`), with shared helpers under `tests/common/`.
  `repovec_test_helpers::recording_http` provides `RecordingServer` and
  `ScriptedResponse`; `repovec_test_helpers::repository_listing_wire` provides
  JSON builders, `link_header`, and fake tokens;
  `repovec_test_helpers::capture_logs` captures `tracing` output.

Test and verification tooling: `rstest` 0.28, `rstest-bdd` 0.5 (with
`strict-compile-time-validation`), `proptest` 1.11, `insta` 1.47 (`yaml`), and
`googletest` 0.14 are workspace dependencies; `pretty_assertions` 1.4 is
present. `cargo nextest` runs when installed (`scripts/run_rust_tests.py`). No
Kani, Verus, or `rust-prover-tools` wiring exists in the repository. The host
provides `~/.local/bin/prover-tools` and `cargo kani`. The latest Verus release
at drafting is `release/0.2026.11.02.8bf68ee` (the newer rolling release is not
a pin candidate); `leynos/rust-prover-tools` `main` is
`6be9e71bc3ce98e96087be74b4cc572ca3488cb0`. The `chutoro` repository is estate
precedent for Verus wiring (`docs/verus-toolchain.md` there).

Gates: `make check-fmt`, `make lint` (interrogate, `cargo doc`, Clippy with
`-D warnings`, Whitaker), `make test` (script tests, nextest or cargo test,
doctests), `make markdownlint` (runs `make spelling` first), `make nixie`, and
`make test-workflow-contracts`. No target runs proofs today.

Constraints from documentation: the developers' guide section 9.2 forbids a
second traversal; section 9.3 lists the steps for adding an endpoint profile;
section 9.6 names a pure policy kernel as the trigger for formal proof; the
device-flow section mandates `MonotonicClock` for elapsed-time decisions; and
section 8 forbids environment reads outside composition roots.

Budget survey. The estimate per area, from the 2.2.1 file sizes and the scope
below, is:

- core `active_branches` (names, commit identifiers, instants, policy,
  evidence, kernel, evaluation, discovery, desired state, store port): about
  1,350 production lines;
- `github_rest` amendments (canonical path, continuation parameter): about
  120 production lines;
- GitHub evidence adapter (three wire records, three profiles, port
  implementation, telemetry): about 450 production lines;
- shared atomic file module (moved, not new) and snapshot store adapter with
  wire format: about 400 production lines;
- one-shot command, command line, output, and example: about 600 production
  lines;
- tests across both crates (unit, property, protocol, behavioural,
  end-to-end, snapshot, bridge): about 6,300 lines, scaled from 2.2.1's
  test-to-production ratio of about 1.8;
- Verus model and lemmas: about 800 lines; Makefile and Python contract tests:
  about 150 lines;
- documentation: technical design (about 150), ADR 0003 (about 120), ADR 0004
  (about 140), users' guide (about 150), developers' guide (about 220),
  `tools/verus/README.md` (about 40), contents and roadmap (about 20).

Signposts. Read these before implementing:

- `docs/repovec-appliance-technical-design.md`, sections "Repository listing
  (2.2.1)", "Discovery and continuous monitoring", and "Active branch policy";
- `docs/adrs/adr-0002-shared-github-rest-traversal.md`;
- `docs/developers-guide.md`, sections 8 and 9;
- `docs/rust-testing-with-rstest-fixtures.md`, `docs/rstest-bdd-users-guide.md`,
  `docs/rust-doctest-dry-guide.md`,
  `docs/reliable-testing-in-rust-via-dependency-injection.md`,
  `docs/complexity-antipatterns-and-refactoring-strategies.md`, and, for the
  deferred configuration surface, `docs/ortho-config-users-guide.md`;
- skills: `execplans`, `hexagonal-architecture`, `rust-router` (then
  `rust-types-and-apis`, `rust-errors`, `rust-unit-testing`), `proptest`,
  `verus`, `rust-verification`, `arch-decision-records`, and `codegraph-mcp`
  for navigation when the CodeGraph server is available.

## Conformance basis

Upstream artefacts, inspected at `f4da025`:

- Terms of Reference: none exists for this repository; the roadmap and the
  technical design are the governing artefacts.
- `docs/roadmap.md`, item 2.2.2 and its objective in section 2.2.
- `docs/repovec-appliance-technical-design.md`, sections "Active branch
  policy", "Discovery and continuous monitoring", "Repository listing (2.2.1)",
  and "Worktrees and checkout layout".
- `docs/adrs/adr-0002-shared-github-rest-traversal.md` (Accepted,
  2026-11-19).
- `docs/adrs/adr-0001-repovec-directory-provisioning.md` (data root
  ownership).
- `AGENTS.md` and `docs/developers-guide.md` sections 8, 9.2, 9.3, and 9.6.

Requirements derived for this plan:

- `REQ-list`: for each accessible repository, list its branches through the
  shared traversal.
- `REQ-default`: the default branch is always active when it exists.
- `REQ-recent`: branches with qualifying activity in the last *N* days are
  admissible.
- `REQ-pr`: branches referenced by open same-repository pull requests are
  admissible when the policy includes pull requests.
- `REQ-cap`: at most `cap` branches are active per repository, evicting the
  least recently active admissible branches beyond it.
- `REQ-store`: the desired state (active branches per repository) is stored
  for the reconciler.
- `REQ-configurable`: the policy parameters are values a later configuration
  surface can set.
- `REQ-fail-safe`: a failed observation is never recorded as an empty or
  smaller active set (derived from the 2.2.1 "not a snapshot" contract and the
  reconcile-first design).

Gaps against the baseline:

- `GAP-per-repo-path`: `EndpointProfile.path` is a static path; per-repository
  endpoints need a path built from a repository identifier.
- `GAP-cursor`: continuation confinement admits only `page`.
- `GAP-branch-name`: no validated branch-name type exists; the default branch
  is an unvalidated string.
- `GAP-wall-clock`: no wall-clock seam exists; guidance forbids wall-clock
  time for elapsed-time decisions.
- `GAP-store`: no persisted state format or shared atomic-write helper exists.
- `GAP-prover`: no proof tooling exists.

Trace:

```plaintext
TDD-active-branch-policy -> REQ-default, REQ-recent, REQ-pr, REQ-cap
  -> D4, D5 -> EP-M1 (L-*) -> EP-M2 (kernel, evaluate_repository)
  -> tests: active_branch_policy_bdd, selection_properties, kernel_bridge
TDD-discovery -> REQ-list, GAP-per-repo-path, GAP-cursor -> D2, D3, D10
  -> EP-M3 -> tests: github_branch_evidence*, activity_continuations
TDD-discovery -> REQ-store, REQ-fail-safe, GAP-store -> D7, D8 -> EP-M4
  -> tests: discovery (fail-safe table), desired_state_store (round trip, snapshot)
REQ-list..REQ-store -> D1 -> EP-M5 -> tests: active_branch_e2e,
  active_branch_discovery_bdd
REQ-configurable -> D5, D9 -> EP-M2 (ActiveBranchPolicy) -> EP-M5 (flags)
GAP-branch-name -> D12 -> EP-M2 -> tests: branch_name table and git oracle
GAP-wall-clock -> D13 -> EP-M5 composition root -> developers' guide carve-out
GAP-prover -> D11 -> EP-M1 -> make verus, workflow contract tests
```

## Verification plan

### Abstract model

A repository observation is a tuple `(B, d, A, P)`:

- `B` is a finite sequence of observed branches `(name, head)`; `names(B)` is
  its set of distinct names after first-wins deduplication.
- `d` is an optional default branch name.
- `A` is a finite sequence of activity records `(name, kind, t)` with `t` in
  Unix seconds.
- `P` is a finite sequence of open pull request heads
  `(head_repository: Option<id>, head_branch)`.

A policy is `(w, c, p)`: window `w` seconds (`w = days * 86,400`, days in
1..=364), cap `c` in 1..=1,000, and the pull request flag `p`. `now` is Unix
seconds.

For a name `n` in `names(B)`:

- `pinned(n) := d == Some(n)`;
- `last(n) := max { t | (n, k, t) in A, k qualifying }`, or `None`;
- `recent(n) := last(n) == Some(t) && t + w >= now`;
- `referenced(n) := p && exists (Some(r), n) in P with r == repository id`;
- `admissible(n) := pinned(n) || recent(n) || referenced(n)`.

The rank order is a strict order on candidates `outranks(x, y)` that holds when
`x` is pinned and `y` is not; or both agree on pinned and `last(x) > last(y)`
(with `Some(_) > None`); or both agree on pinned and `last` and
`name(x) < name(y)` in byte order.

The selection is `sel(c) := take(sort_desc(filter(admissible, names(B))), c)`;
`evicted := admissible \ sel(c)`.

Discovery maps an outcome to each listed repository: `Evaluated(sel, evicted)`
when every required evidence traversal succeeded, otherwise
`Unavailable(category)` for repository-scoped failures; a credential-wide
failure aborts discovery with no snapshot.

### Invariants and lemmas

Selection (proved in Verus over the abstract model, then bridged):

- `L-rank-order`: `outranks` is irreflexive and transitive, and for distinct
  names exactly one of `outranks(x, y)` and `outranks(y, x)` holds.
- `L-sort-permutation`: `sort_desc(s)` is a permutation of `s`.
- `L-sort-sorted`: for `i < j`, `outranks(sort_desc(s)[i], sort_desc(s)[j])`
  when names in `s` are distinct.
- `L-sorted-unique`: two sequences that are sorted by `outranks`, have
  distinct names, and are permutations of one another are equal.
- `INV-bounded` (`REQ-cap`): `|sel(c)| == min(c, |admissible|)`.
- `INV-admissible`: every selected name is in `names(B)` and admissible.
- `INV-default` (`REQ-default`): if `d` is in `names(B)`, then `d` is in
  `sel(c)` (because `c >= 1` and at most one candidate is pinned).
- `INV-lru` (`REQ-cap`): for every selected `x` and every evicted `y`,
  `outranks(x, y)`.
- `INV-cap-monotone`: `c1 <= c2` implies `sel(c1)` is a prefix of `sel(c2)`.
- `INV-order-independent`: permuting `B`, `A`, or `P` does not change `sel`
  or `evicted` as sets (follows from `L-sorted-unique` and first-wins
  deduplication over a set of names).
- `L-window`: for all `u64` values of `t`, `now`, and `w`, the executable
  check `now.saturating_sub(w) <= t` equals the mathematical `t + w >= now`.

Evaluation and discovery (tests):

- `INV-evidence-recent` (`REQ-recent`): `recent` uses only qualifying
  activity types, the branch namespace, and the inclusive boundary.
- `INV-pr-same-repo` (`REQ-pr`): only pull requests whose head repository
  identifier equals the repository's identifier contribute; a `null` head
  repository never contributes; `p == false` skips the traversal entirely.
- `INV-fail-safe` (`REQ-fail-safe`): a repository with any failed evidence
  traversal is `Unavailable`, never `Evaluated` with any set.
- `INV-no-partial-publish`: a credential-wide failure or a failed repository
  listing never calls `DesiredStateStore::publish`.
- `INV-round-trip` (`REQ-store`): `load()` after `publish(s)` returns `s`.
- `INV-schema-guard`: a snapshot with an unknown `schema_version`, malformed
  JSON, or invalid identifiers fails `load()` with a typed error, never
  `Ok(None)`; only a missing file is `Ok(None)`.
- `INV-atomic-publish`: after `publish`, the file holds either the previous or
  the new snapshot in full, with mode `0600`, and no temporary file remains.
- `INV-canonical-path` (`GAP-per-repo-path`): every evidence request path is
  `/repositories/{decimal id}/{branches|pulls|activity}`.
- `INV-cursor-confinement` (`GAP-cursor`): an activity continuation may change
  only `after`; adding `before`, changing a fixed parameter, or changing the
  path is `invalid_continuation`. Numbered profiles still allow only `page`.
- `INV-branch-name` (`GAP-branch-name`): `BranchName::new` accepts exactly the
  names `git check-ref-format --branch` accepts, excluding the `@{-N}`
  shorthand.

### Axioms (trusted, not verified here)

- `AXIOM-std-sort`: `slice::sort_by` returns a permutation sorted with respect
  to a comparator that is a total order (Rust standard library documentation).
  Boundary: `select_active`.
- `AXIOM-derived-ord`: derived `Ord` on tuples, `bool`, `Option`, and
  `core::cmp::Reverse` is lexicographic, with `false < true` and
  `None < Some(_)` (Rust reference). Boundary: `RankKey`.
- `AXIOM-branches-shape`: `GET /repositories/{id}/branches?per_page=100`
  returns a bare array of objects with `name` and `commit.sha`, paginated by
  `page` (GitHub REST documentation; probe P2). Boundary: `BranchWire`.
- `AXIOM-pulls-shape`: `GET /repositories/{id}/pulls?state=open&per_page=100`
  returns a bare array whose items carry `head.ref` and `head.repo` (object
  with numeric `id`, or `null` when the fork is gone), paginated by `page`
  (GitHub REST documentation; probe P2). Boundary: `PullRequestWire`.
- `AXIOM-activity-shape`: `GET /repositories/{id}/activity` returns a bare
  array of records with string `ref`, string `activity_type`, and RFC 3339
  `timestamp`, sorted newest first, paginated by an opaque `after` cursor in a
  `rel="next"` Link, and filtered by `time_period` in `day`, `week`, `month`,
  `quarter`, `year` (probes P3 to P5). Boundary: `ActivityWire`.
- `AXIOM-activity-period`: `day`, `week`, `month`, `quarter`, and `year` cover
  at least 1, 7, 28, 89, and 364 days respectively. GitHub documents `day` as
  24 hours and `week` as 7 days; the longer spans are conservative assumptions.
  Boundary: `RecentPushWindow::covering_period`.
- `AXIOM-canonical-path`: `/repositories/{id}/...` serves the same listing as
  `/repos/{owner}/{repo}/...` (probe P1). Boundary: `INV-canonical-path`.
- `AXIOM-rename`: POSIX `rename(2)` within one directory atomically replaces
  the target. Boundary: `INV-atomic-publish`.
- `AXIOM-synchronized-clock`: the appliance clock is synchronized to within
  minutes. Boundary: `now` at the composition root.

### Methods per obligation

- Obligation: `L-rank-order`, `L-sort-permutation`, `L-sort-sorted`,
  `L-sorted-unique`, `INV-bounded`, `INV-admissible`, `INV-default`, `INV-lru`,
  `INV-cap-monotone`, and `L-window`.
  - Method: Verus proof over a spec model of candidates
    (`Seq<CandidateSpec>` with `name: nat` under a strict total order,
    `pinned: bool`, `admissible: bool`, `last: Option<nat>`), with
    `sort_desc` defined as a recursive insertion sort spec function and `sel`
    as `take` after `filter`. `L-window` is an `exec` function over `u64` with
    an `ensures` clause.
  - Rationale: the properties are quantified over all sequence lengths and
    orderings; examples and generators sample them, a proof establishes them.
  - Domain: all finite sequences with distinct names; all caps of at least 1;
    all `u64` instants.
  - Artefact: `verus/active_branches.rs` (root: spec types, `outranks`,
    `admissible`, top-level lemmas, `mod` declarations),
    `verus/active_branches_sort.rs` (permutation, sortedness, uniqueness),
    `verus/active_branches_select.rs` (bounded, admissible, default, LRU,
    monotone), `verus/active_branches_window.rs`.
  - Evidence: `make verus` exits 0 and reports every function verified with
    zero errors; before the proofs are complete the target fails with the
    open goals, never succeeding silently.
  - Non-vacuity: a `proof fn witness_eviction()` constructs a concrete
    four-candidate sequence (one pinned, three admissible with distinct
    recencies, cap 2) and proves the selection is exactly the pinned branch
    and the most recent admissible branch, showing every antecedent is
    inhabited. Negative control: reversing the recency comparison in
    `outranks` must make `INV-lru` fail to verify; reverting restores it.
    Record both transcripts. Inspect the proof for `assume`, `admit`,
    `external_body`, and added axioms with
    `grep -nE 'assume|admit|external_body|#\[verifier::external' verus/`,
    which must print nothing.
- Obligation: refinement of the model by the production kernel
  `repovec_core::active_branches::select_active`.
  - Method: three bridge tests in `crates/repovec-core`. First, an exhaustive
    `rstest` table over candidate pairs drawn from
    `{pinned, unpinned} x {None, Some(0), Some(1), Some(u64::MAX)} x {"a",
    "b"}` asserting that `RankKey` ordering agrees with a direct Rust
    transcription of the Verus `outranks`. Second, a `proptest` property
    asserting that the kernel's output satisfies the independent predicates
    for `INV-bounded`, `INV-admissible`, `INV-default`, `INV-lru`, and
    `INV-cap-monotone`, written from the abstract model rather than from the
    kernel. Third, a `proptest` property asserting `INV-order-independent` by
    shuffling the input.
  - Rationale: the production kernel is "filter, sort by derived key, take".
    With `AXIOM-std-sort` and `AXIOM-derived-ord`, the remaining refinement
    obligation is that `RankKey` encodes `outranks`, which the exhaustive pair
    table decides over every equivalence class of the comparison.
  - Artefact: `crates/repovec-core/src/active_branches/kernel_bridge_tests.rs`
    and `crates/repovec-core/src/active_branches/selection_properties.rs`.
  - Evidence: `cargo test -p repovec-core active_branches` passes; the red
    stage fails to compile before the kernel exists.
  - Non-vacuity: the generator classifies each case (eviction occurred,
    pinned present, pinned absent, recency tie, `None` recency, cap equals
    admissible count) and a companion test asserts every class occurs at least
    once in 256 cases. Negative controls: sorting ascending, or dropping the
    pinned component from `RankKey`, must fail the property; record the
    shrunk counterexamples. Proptest regression files are committed under
    `proptest-regressions/` only when a real defect is found.
- Obligation: drift between the Verus model and the production kernel.
  - Method: a contract test reading `verus/active_branches.rs` and asserting
    that each `/// Models:` annotation names a production item that exists in
    `crates/repovec-core/src/active_branches/`.
  - Artefact: `tests/workflow_contracts/verus_model_mirror_test.py`, run by
    `make test-workflow-contracts`, together with the Makefile shape tests in
    `tests/workflow_contracts/verus_make_test.py`.
  - Non-vacuity: the test fails when an annotation names a missing item;
    exercise this with a fixture file.
- Obligation: `INV-evidence-recent`, `INV-pr-same-repo`, and `INV-branch-name`.
  - Method: `rstest` tables for the finite partitions (each activity type,
    tag refs, deleted branches, boundary `t == now - w`, `t == now - w - 1`,
    fork and `null` head repositories, `p` on and off); `proptest` for
    `BranchName` with a differential oracle that runs
    `git check-ref-format --branch` on generated names.
  - Rationale: finite partitions are decided by named cases; refname validity
    spans an open input space with an authoritative external oracle.
  - Artefact: `crates/repovec-core/src/active_branches/evaluate_tests.rs`,
    `crates/repovec-core/src/active_branches/branch_name_tests.rs`, and
    `crates/repovec-core/tests/branch_name_git_oracle.rs`.
  - Non-vacuity: the generator mixes valid names with each forbidden
    construct (`..`, leading `.`, `.lock` suffix, `@{`, control characters,
    space, `~^:?*[\`, trailing `/` or `.`, `//`) and the test asserts both
    accept and reject outcomes occur. Negative control: removing the `.lock`
    rule must fail the oracle property.
- Obligation: `INV-fail-safe` and `INV-no-partial-publish`.
  - Method: an `rstest` table over every `RepositoryListingError` category
    injected into each of the three evidence calls through a scripted fake
    `BranchEvidenceSource`, asserting the outcome class (unavailable for
    repository-scoped, abort for credential-wide) and that a recording fake
    `DesiredStateStore` saw no `publish` call on abort. A `proptest` over
    sequences of per-repository outcomes asserts that every repository with a
    failure is `Unavailable` and every other repository is `Evaluated`.
  - Artefact: `crates/repovec-core/src/active_branches/discovery_tests.rs`.
  - Non-vacuity: the table includes the empty-branch success case, so a
    defect mapping failure to `Evaluated` with no branches is distinguishable.
    Negative control: mapping `forbidden` to an empty evaluated set must fail.
- Obligation: `INV-round-trip`, `INV-schema-guard`, and `INV-atomic-publish`.
  - Method: a `proptest` round trip through a temporary `cap_std` directory;
    an `insta` JSON snapshot of a representative desired state; `rstest`
    cases for missing file, unknown version, malformed JSON, invalid branch
    name, and permission bits; a test that pre-creates a snapshot, forces a
    serialization or write failure, and asserts the previous bytes survive.
  - Artefact: `crates/repovecd/src/desired_state_store/tests.rs` and its
    snapshot directory.
  - Non-vacuity: the round-trip generator includes unavailable repositories,
    empty active sets, and evicted lists.
- Obligation: `INV-canonical-path` and `INV-cursor-confinement`.
  - Method: protocol tests against `RecordingServer` asserting each request
    path and query; `rstest` tables of hostile continuations (`before` added,
    `time_period` changed, path changed to `/repos/...`, repeated `after`);
    a `proptest` over generated cursor chains, reusing the 2.2.1 pagination
    property structure, asserting complete traversal for confined chains and
    `invalid_continuation` for any mutation.
  - Artefact: `crates/repovecd/tests/github_branch_evidence.rs`,
    `crates/repovecd/tests/github_branch_evidence_continuations.rs`,
    `crates/repovecd/tests/activity_pagination_properties.rs`, and
    `crates/repovecd/src/github_rest/link/tests.rs`.
  - Non-vacuity: the cursor chain generator produces chains of length one to
    eight and at least one mutated continuation per failing case.

### Formal verification decision

Verus is warranted for the selection model (decision D11). The guarantees are
contractual (the reconciler and the operator depend on them), quantified over
unbounded inputs, and stated over a small pure kernel that is unlikely to
change shape. Verus would not add value for the GitHub adapter, the snapshot
store, the command line, or telemetry: those promises are about real HTTP,
filesystem, and process boundaries, which protocol, property, snapshot, and
end-to-end tests exercise directly. Kani is not used; its distinctive value,
exhaustive bounded execution of the production Rust, is replaced here by the
bridge tests over a finite comparison domain plus documented standard-library
contracts. `make verus` is not added to `make test`, `make lint`, `make all`,
or the pull request gate in this item, following the estate on-ramp guidance;
it is an explicit acceptance command for `EP-M1`, `EP-M2`, and `EP-M6`, and a
scheduled CI job is recorded as follow-up work.

### Suggested BDD features

Policy behaviour, at
`crates/repovec-core/tests/features/active_branch_policy.feature`, run by
`crates/repovec-core/tests/active_branch_policy_bdd.rs` (`REQ-default`,
`REQ-recent`, `REQ-pr`, `REQ-cap`; `EP-M2`):

```gherkin
Feature: Active branch policy
  Scenario: The default branch stays active without recent activity
    Given a repository whose default branch is "main"
    And the branches "main" and "feature/search"
    And "feature/search" was pushed 2 days ago
    And "main" was last pushed 40 days ago
    When the policy is evaluated with a 14 day window and a cap of 5
    Then the active branches are "main" and "feature/search"

  Scenario: Branches beyond the cap are evicted least recently active first
    Given a repository whose default branch is "main"
    And the branches "main", "a", "b", and "c"
    And "a" was pushed 1 day ago, "b" 3 days ago, and "c" 5 days ago
    When the policy is evaluated with a 14 day window and a cap of 3
    Then the active branches are "main", "a", and "b"
    And the evicted branches are "c"

  Scenario: A same-repository pull request keeps a quiet branch active
    Given a repository whose default branch is "main"
    And the branches "main" and "review/old"
    And an open pull request from "review/old" in the same repository
    When the policy is evaluated with pull requests included
    Then the active branches are "main" and "review/old"

  Scenario: A fork pull request does not activate a same-named branch
    Given a repository whose default branch is "main"
    And the branches "main" and "patch"
    And an open pull request from "patch" in a fork
    When the policy is evaluated with pull requests included
    Then the active branches are "main"

  Scenario: A deleted branch's activity is ignored
    Given a repository whose default branch is "main"
    And the branches "main"
    And "gone" was deleted 1 day ago
    When the policy is evaluated with a 14 day window and a cap of 5
    Then the active branches are "main"
```

Discovery behaviour, at
`crates/repovecd/tests/features/active_branch_discovery.feature`, run by
`crates/repovecd/tests/active_branch_discovery_bdd.rs` against `RecordingServer`
(`REQ-list`, `REQ-store`, `REQ-fail-safe`; `EP-M5`):

```gherkin
Feature: Active branch discovery
  Scenario: Activity spanning several cursor pages is fully considered
    Given a user credential that sees the repository "octo-org/widgets"
    And its activity spans 3 pages linked by "after" cursors
    When active branches are discovered
    Then the snapshot lists the branch pushed on the last page as active

  Scenario: A forbidden repository is unavailable, not empty
    Given a user credential that sees "octo-org/widgets" and "octo-org/secret"
    And listing branches of "octo-org/secret" is forbidden
    When active branches are discovered
    Then "octo-org/widgets" is evaluated
    And "octo-org/secret" is unavailable with category "forbidden"

  Scenario: Rate limiting publishes nothing and keeps the previous snapshot
    Given a stored snapshot from an earlier run
    And the activity listing of the second repository is rate limited
    When active branches are discovered
    Then the command exits with the discovery failure status
    And the stored snapshot is unchanged

  Scenario: A continuation that adds a "before" cursor is refused
    Given a user credential that sees the repository "octo-org/widgets"
    And its activity continuation adds a "before" parameter
    When active branches are discovered
    Then "octo-org/widgets" is unavailable with category "invalid_continuation"
```

Both runners exist (`rstest-bdd` 0.5); each feature is added red, with its step
definitions, before the implementation that makes it pass.

## Plan of work

Each milestone follows Red-Green-Refactor: add the smallest failing test or
proof obligation, observe the expected failure, make it pass with the smallest
change, then refactor with the focused tests and the full gates green. Commit
at least once per green step; every commit passes `make check-fmt`,
`make lint`, and `make test`.

### `EP-M1` Verus on-ramp and proved selection model

Assigned: `GAP-prover`, D11, `L-*`, `INV-bounded`, `INV-admissible`,
`INV-default`, `INV-lru`, `INV-cap-monotone`.

1. Add `tools/verus/VERSION` (`0.2026.11.02.8bf68ee`), `tools/verus/SHA256SUMS`
   (the SHA-256 of `verus-0.2026.11.02.8bf68ee-x86-linux.zip`, computed from
   the downloaded release asset and cross-checked against the release page),
   `tools/verus/README.md`, and `tools/rust-prover-tools/REF`
   (`ref: 6be9e71bc3ce98e96087be74b4cc572ca3488cb0`, or the then-current `main`
   commit recorded in `Decision log`). Add `.verus/` to `.gitignore`.
2. Red: add `tests/workflow_contracts/verus_make_test.py` asserting the pin
   files' shape, that `make install-verus` and `make verus` delegate through
   `$(PROVER_TOOLS)` with no `curl`, `unzip`, `sha256sum`, or
   `rustup toolchain install`, and, through the
   `PROVER_TOOLS_CONTRACT_TEST=1 PROVER_TOOLS=<fake>` command-line seam, the
   exact
   `prover-tools verus run --repo-root . --proof-file verus/active_branches.rs`
   invocation. Run `make test-workflow-contracts` and observe the failure.
3. Green: add the `install-verus` and `verus` targets as in the Verus
   on-ramp (pin-derived `PROVER_TOOLS`, guarded `REF`), outside `make test`,
   `make lint`, and `make all`. Confirm `make test-workflow-contracts` passes.
4. Red: write `verus/active_branches.rs` and its sub-files with the spec types,
   `outranks`, `admissible`, `sort_desc`, `sel`, and every lemma statement from
   `Invariants and lemmas`, each with an empty proof body, plus the witness. Run
   `make install-verus` then `make verus` and record the failing goals. Do not
   add `assume` or `admit`.
5. Green: prove the lemmas, in the order `L-rank-order`, sort lemmas,
   selection lemmas, `L-window`, decomposing when the solver struggles and
   scoping helpers with `assert(...) by { ... }`. Run the negative control and
   record it.
6. Add `tests/workflow_contracts/verus_model_mirror_test.py` with its fixture;
   the `/// Models:` annotations stay red-tolerant until `EP-M2` by naming only
   items introduced there, so add this test in `EP-M2` step 1 instead if it
   would otherwise fail here.

Acceptance: `make verus` verifies every function with zero errors;
`make test-workflow-contracts`, `make check-fmt`, `make lint`, `make test`, and
`make markdownlint` pass.

### `EP-M2` Core domain, kernel, and policy evaluation

Assigned: `REQ-default`, `REQ-recent`, `REQ-pr`, `REQ-cap`, `REQ-configurable`,
`GAP-branch-name`, D4, D5, D12.

1. Red: create `crates/repovec-core/src/active_branches/mod.rs` declaring
   submodules and add `pub mod active_branches;` to `lib.rs`; add
   `branch_name_tests.rs` tables and the git oracle property; run
   `cargo test -p repovec-core active_branches` and observe compile failure.
2. Green: `branch_name.rs` (`BranchName`, `BranchNameError`) and
   `commit_id.rs` (`CommitId`: 40 or 64 lowercase hexadecimal characters).
3. Change `RepositorySummary::default_branch` to `Option<&BranchName>` and
   `with_details` to take `Option<BranchName>`; update
   `RepositoryWire:: into_domain` in
   `crates/repovecd/src/github_repository_client/wire.rs` to validate, mapping
   failure to `InvalidResponse`; update every caller and test
   (`grep -rn default_branch crates/`). Add a failing test first proving an
   invalid default branch fails the listing.
4. Red then green: `instant.rs` (`UnixSeconds`, `RecentPushWindow`,
   `ActivityPeriod`, `RecentPushWindow::covering_period`), `policy.rs`
   (`BranchCap`, `ActiveBranchPolicy::new`, `defaults`), `evidence.rs`
   (`BranchHead`, `OpenPullRequestHead`, `RefActivity`, `RefActivityKind`,
   `RepositoryEvidence`), `kernel.rs` (`BranchCandidate`, `RankKey`,
   `select_active`, `Selection`), and `evaluate.rs` (`evaluate_repository`,
   `RepositoryEvaluation`, `ActiveBranch`, `ActivationReasons`), each with its
   `rstest` tests.
5. Add the bridge tests and selection properties (see `Verification plan`),
   the mirror contract test, and the policy BDD feature with its runner.
6. Refactor: keep each file under 400 lines; run `make verus` to confirm the
   annotations still match.

Acceptance: `cargo test -p repovec-core active_branches`,
`cargo test -p repovec-core --test active_branch_policy_bdd`,
`cargo test -p repovec-core --test branch_name_git_oracle`, `make verus`,
`make test-workflow-contracts`, and the three standard gates pass.

### `EP-M3` GitHub evidence adapter and traversal amendments

Assigned: `REQ-list`, `GAP-per-repo-path`, `GAP-cursor`, D2, D3, D6, D10.

1. Red: extend `crates/repovecd/src/github_rest/link/tests.rs` with cursor
   confinement cases (`after` changes accepted for a cursor profile; `page`
   rejected for it; `before` rejected for both kinds), and add `request_url`
   tests for a per-repository path. Observe failures.
2. Green: in `profile.rs`, replace `path: &'static str` with
   `path: EndpointPath` (`Fixed(&'static str)` or
   `PerRepository { suffix: &'static str }`) and add
   `continuation: ContinuationParameter` (`Page` or `Cursor(&'static str)`).
   Thread a resolved path into `GitHubApiEndpoint::request_url` and the
   continuation parameter into `ensure_query`. Update the two repository
   profiles (`Fixed`, `Page`) and `test_support.rs` together.
3. Red: protocol tests in `crates/repovecd/tests/github_branch_evidence.rs`
   for each listing (request path and query, page order, empty listing, `null`
   head repository, tag refs and unknown activity types ignored, invalid branch
   name is `invalid_response`, invalid timestamp is `invalid_response`),
   `github_branch_evidence_continuations.rs`, and
   `activity_pagination_properties.rs`.
4. Green: add `crates/repovecd/src/github_branch_evidence/` with `wire.rs`
   (`BranchWire`, `PullRequestWire`, `ActivityWire`, each decoding only the
   fields used), the three profiles (`branches?per_page=100`,
   `pulls?state=open&per_page=100`,
   `activity?time_period=<period>&per_page=100`; the period is one of five
   static profiles), and
   `impl BranchEvidenceSource for GitHubRepositoryClient`, sharing the client's
   credential, endpoint, and HTTP client, with per-endpoint limits from D10.
   Emit `tracing` events carrying the numeric repository identifier, listing
   kind, pages, records, bytes, and elapsed milliseconds, or the failure
   category.
5. Write ADR 0003 (amends ADR 0002) and add the amendment banner to ADR 0002.

Acceptance: `cargo test -p repovecd --test github_branch_evidence`,
`--test github_branch_evidence_continuations`,
`--test activity_pagination_properties`, every existing
`github_repository_client*` and `repository_listing*` test, and the three
standard gates pass.

### `EP-M4` Discovery use case and desired-state snapshot store

Assigned: `REQ-store`, `REQ-fail-safe`, `GAP-store`, D7, D8.

1. Red: `discovery_tests.rs` with the fail-safe table and property against
   scripted fakes; observe compile failure.
2. Green: `discovery.rs` (`discover_desired_state`, `DiscoveryError`,
   `RepositoryDesiredState`, `BranchSetOutcome`, `DesiredState`,
   `EvidenceFailure`) and `store.rs` (`DesiredStateStore`,
   `DesiredStateStoreError`).
3. Refactor commit (separate): move `write_atomically` from
   `github_token_store/atomic_write.rs` to `crates/repovecd/src/atomic_file.rs`
   as `pub(crate)`, with its tests, updating the token store to call it. No
   behaviour change; the existing token-store tests must pass unchanged.
4. Red then green: `crates/repovecd/src/desired_state_store/` with `wire.rs`
   (`serde` records for schema version 1) and `mod.rs`
   (`FileDesiredStateStore::new(dir: cap_std::fs_utf8::Dir, file_name)`,
   implementing the port), and its tests and snapshot.
5. Add `RuntimePaths::active_branches_snapshot()` with a doctest.
6. Write ADR 0004.

Acceptance: `cargo test -p repovec-core discovery`,
`cargo test -p repovecd --lib desired_state_store`,
`cargo test -p repovecd --lib github_token_store`, and the three standard gates
pass.

### `EP-M5` One-shot composition, end-to-end, and behavioural evidence

Assigned: `REQ-list` to `REQ-store` end to end, D1, D9, D13.

1. Red: `crates/repovecd/tests/active_branch_e2e.rs` driving the composition
   in-process with `RecordingServer` and a temporary state directory: success
   writes the snapshot and the summary lines; a repository-scoped failure
   appears as `unavailable`; a credential-wide failure leaves a pre-existing
   snapshot byte-identical and exits with the discovery failure status; a
   state-write failure exits with the store failure status. Add the discovery
   feature and its runner.
2. Green: `crates/repovecd/src/active_branch_command/` (`mod.rs` with
   `EvaluationRun` and `run_evaluation`, `cli/mod.rs` with the flags, and
   `output.rs`), reusing `ListingMode` and `UserTokenLoader` from
   `repository_listing_command`. Extend `RunStatus` with `DiscoveryFailure`
   (exit 7) and `StoreFailure` (exit 8) only if the existing codes cannot
   express them; otherwise reuse. Read the wall clock once in `cli::run` and
   pass `UnixSeconds` inward.
3. Add `crates/repovecd/examples/evaluate_active_branches.rs`, which supplies
   process streams, the encrypted token loader, and the event subscriber, and
   nothing else.
4. Add event-capture tests proving a completed run emits one non-zero
   completion event with repository and branch counts and that no token,
   repository name, or branch name appears in captured logs.

Acceptance: `cargo test -p repovecd --test active_branch_e2e`,
`--test active_branch_discovery_bdd`, `--lib active_branch_command`,
`cargo build -p repovecd --example evaluate_active_branches`, and the three
standard gates pass.

### `EP-M6` Documentation closure and roadmap completion

Assigned: every requirement's documentation, D4 to D13.

1. Technical design: rewrite "Active branch policy" with the semantics,
   defaults, evidence sources, and failure classification, linking ADRs 0003
   and 0004; extend "Discovery and continuous monitoring" with the snapshot
   contract; note the 2.3.2 filesystem-mapping risk under "Worktrees and
   checkout layout"; record the proof obligations that shaped the kernel.
2. Users' guide: a section "Active branch evaluation" covering the command,
   flags, defaults, output, exit statuses, the snapshot location and format,
   and the unavailable versus empty distinction.
3. Developers' guide: section 10 covering the port, the kernel and its
   reuse policy (the only implementation of branch selection; reconciler and
   TUI call it, never re-implement it), the shared `atomic_file` module and its
   reuse policy, the Verus proof inventory (what each file proves and does not),
   `make install-verus` and `make verus`, the wall-clock carve-out, and the
   focused test commands; update section 9.2 and 9.3 for path and continuation
   parameters.
4. `docs/contents.md`: add ADRs 0003 and 0004, this ExecPlan, and
   `tools/verus/README.md`.
5. `docs/roadmap.md`: mark 2.2.2 `[x]` with a status paragraph, only after
   every acceptance criterion passes.
6. Run `make fmt`, `make markdownlint`, `make nixie`, the three standard
   gates, `make test-workflow-contracts`, and `make verus`.

## Milestones and plateaus

- `EP-M1`: proof tooling and a proved model exist; no production code depends
  on them. Requirements advanced: `GAP-prover` and the selection invariants.
  Acceptance: `make verus` and the workflow contract tests. Conformance: design
  unchanged; no public interface or runtime dependency added; tooling pins
  only. Recovery: revert the milestone commits; nothing depends on them.
  Remaining: the bridge to production code. Compatibility: none.
- `EP-M2`: the core can evaluate one repository's evidence under a policy.
  Requirements: `REQ-default`, `REQ-recent`, `REQ-pr`, `REQ-cap`,
  `REQ-configurable`, `GAP-branch-name`. Acceptance: core unit, bridge,
  property, oracle, and BDD tests; `make verus`. Conformance: D4 and D5 as
  approved; the `default_branch` change is the only authorized interface change
  outside the new module; no new runtime dependency. Recovery: revert; 2.2.1
  tests prove the listing still works. Remaining: real evidence. Compatibility:
  none (pre-1.0 internal API, callers updated together).
- `EP-M3`: `repovecd` can list branches, open pull request heads, and branch
  activity for one repository. Requirements: `REQ-list`, `GAP-per-repo-path`,
  `GAP-cursor`. Acceptance: protocol, continuation, and property tests; all
  2.2.1 tests. Conformance: one traversal kept; ADR 0003 records the
  confinement amendment; `time` added as authorized. Recovery: revert; the
  profile change is local to `github_rest`. Remaining: orchestration and
  storage. Compatibility: none.
- `EP-M4`: discovery produces and stores a desired state. Requirements:
  `REQ-store`, `REQ-fail-safe`, `GAP-store`. Acceptance: discovery table and
  property; store round trip, snapshot, and guards; unchanged token-store
  tests. Conformance: ADR 0004 records the persisted format; no change to the
  directory contract. Recovery: revert; the atomic-file move is its own commit.
  Remaining: operator composition. Compatibility: new persisted format,
  versioned from its first release; no deployed predecessor.
- `EP-M5`: an operator can run the command end to end. Requirements: all,
  end to end. Acceptance: end-to-end, BDD, and telemetry tests; example builds.
  Conformance: no environment reads; one wall-clock read at the composition
  root. Recovery: revert; library use case remains. Remaining: documentation.
  Compatibility: none.
- `EP-M6`: documentation and roadmap reflect the implementation. Acceptance:
  documentation gates plus every earlier command. Conformance: reconcile
  discoveries with the technical design and ADRs before setting `COMPLETE`.

## Concrete steps

Working directory: the repository root. Use a shell with `set -o pipefail`. Log
names follow `/tmp/$ACTION-repovec-appliance-$(git branch --show-current).out`.

```sh
set -o pipefail
BRANCH=$(git branch --show-current)
make check-fmt 2>&1 | tee "/tmp/check-fmt-repovec-appliance-${BRANCH}.out"
make lint 2>&1 | tee "/tmp/lint-repovec-appliance-${BRANCH}.out"
make test 2>&1 | tee "/tmp/test-repovec-appliance-${BRANCH}.out"
make markdownlint 2>&1 | tee "/tmp/markdownlint-repovec-appliance-${BRANCH}.out"
make nixie 2>&1 | tee "/tmp/nixie-repovec-appliance-${BRANCH}.out"
make test-workflow-contracts 2>&1 | tee "/tmp/workflow-contracts-repovec-appliance-${BRANCH}.out"
make verus 2>&1 | tee "/tmp/verus-repovec-appliance-${BRANCH}.out"
```

Focused red and green commands per milestone:

```plaintext
EP-M1 red:   make test-workflow-contracts -> verus_make_test fails: target missing
EP-M1 red:   make verus -> open goals reported for each lemma; exit non-zero
EP-M1 green: make verus -> "verification results:: N verified, 0 errors"
EP-M2 red:   cargo test -p repovec-core active_branches -> unresolved module
EP-M2 green: cargo test -p repovec-core active_branches -> all pass
EP-M3 red:   cargo test -p repovecd --test github_branch_evidence -> unresolved items
EP-M3 green: same command -> all pass; existing listing tests still pass
EP-M4 red:   cargo test -p repovec-core discovery -> unresolved items
EP-M4 green: cargo test -p repovec-core discovery; cargo test -p repovecd --lib desired_state_store
EP-M5 red:   cargo test -p repovecd --test active_branch_e2e -> unresolved items
EP-M5 green: same command and --test active_branch_discovery_bdd -> all pass
```

Gate commands compile Rust through the shared admission queue; a waiting build
is not hung. Record any baseline failure on `main` separately, with evidence,
before treating it as a regression.

## Validation and acceptance

Behavioural acceptance:

- Running `cargo test -p repovecd --test active_branch_e2e` passes. Its
  success case scripts two repositories; afterwards the state file parses as
  schema version 1, lists `main` and the recently pushed branch for the first
  repository, records the second as `unavailable` with category `forbidden`,
  has mode `0600`, and standard output holds exactly the two summary lines.
- The rate-limit case leaves a pre-existing snapshot byte-identical and exits
  with the documented discovery failure status.
- `make verus` reports every function verified with zero errors; the negative
  control transcript shows `INV-lru` failing under the reversed comparison.
- Against GitHub, with an operator credential, the example prints one line
  per accessible repository and writes the snapshot; this manual check is
  recorded in `Artefacts and notes` and is not part of the offline gates.

Quality criteria:

- Tests: `make test` passes with the new unit, property, protocol,
  behavioural, snapshot, and end-to-end tests; each new test was observed to
  fail before its implementation.
- Verification: every `L-*` and selection `INV-*` is discharged by Verus with
  no trusted assumptions, and the bridge tests pass with non-vacuity evidence
  recorded.
- Lint and format: `make check-fmt`, `make lint`, `make markdownlint`, and
  `make nixie` pass.
- Security: no environment reads, no secrets or names in telemetry, numeric
  identifiers only in request paths, continuation confinement tightened, not
  loosened.

At each milestone boundary, record the conformance check from
`Milestones and plateaus` in `Progress`.

## Idempotence and recovery

Every step is re-runnable. Gate commands and tests are side-effect free outside
temporary directories. `make install-verus` reinstalls into `.verus/`
idempotently. The snapshot publish is atomic, so interrupting the example never
leaves a torn file; re-running overwrites the snapshot. Each milestone is a set
of commits that can be reverted without affecting earlier milestones; the
atomic-file move is a separate commit so it can be reverted independently.

## Artefacts and notes

Planning probes, run on 2026-11-22 against `leynos/repovec-appliance`
(repository identifier 1182608201) with the operator's own read-only credential
through `gh api`:

```plaintext
P1  GET repos/leynos/repovec-appliance/branches?per_page=2
    Link: <https://api.github.com/repositories/1182608201/branches?per_page=2&page=2>; rel="next", ...
    GET repositories/1182608201/branches?per_page=2 -> 200, same first page,
    X-Github-Api-Version-Selected: 2026-03-10
P2  GET repos/leynos/repovec-appliance/pulls?state=all&per_page=1
    Link: <https://api.github.com/repositories/1182608201/pulls?state=all&per_page=1&page=2>; rel="next", ...
    item: head.ref, head.label, head.sha, head.repo.id == base.repo.id for a same-repository pull request
P3  GET repos/leynos/repovec-appliance/activity?time_period=month&per_page=2
    Link: <https://api.github.com/repositories/1182608201/activity?time_period=month&per_page=2&after=Y3Vy...>; rel="next"
    second page adds rel="prev" with before=<cursor>
    item keys: activity_type, actor, after, before, id, node_id, ref, timestamp
P4  same with X-GitHub-Api-Version: 2026-03-10 -> identical shape
P5  GET .../activity?time_period=fortnight -> 200 (parameter ignored)
P6  quarter of activity: 159 branch_creation, 143 branch_deletion,
    83 force_push, 110 pr_merge, 127 push
```

Implementation evidence (red and green transcripts, negative controls, manual
run) is appended here per milestone.

## Interfaces and dependencies

In `crates/repovec-core/src/active_branches/` (new public module
`repovec_core::active_branches`), at the end of `EP-M4`:

```rust
pub struct BranchName(String);
impl BranchName { pub fn new(value: impl Into<String>) -> Result<Self, BranchNameError>; pub fn as_str(&self) -> &str; }
pub struct CommitId(String);
pub struct UnixSeconds(u64);
pub struct RecentPushWindow(u16); // days, 1..=364
pub enum ActivityPeriod { Day, Week, Month, Quarter, Year }
pub struct BranchCap(core::num::NonZeroU16); // 1..=1000
pub struct ActiveBranchPolicy { /* window, cap, include_open_pull_requests */ }
impl ActiveBranchPolicy {
    pub const fn new(window: RecentPushWindow, cap: BranchCap, include_open_pull_requests: bool) -> Self;
    pub fn defaults() -> Self; // 14 days, 10, true
}

pub trait BranchEvidenceSource {
    fn list_branches(&self, repository: RepositoryId) -> Result<Vec<BranchHead>, RepositoryListingError>;
    fn list_open_pull_request_heads(&self, repository: RepositoryId)
        -> Result<Vec<OpenPullRequestHead>, RepositoryListingError>;
    fn list_branch_activity(&self, repository: RepositoryId, period: ActivityPeriod)
        -> Result<Vec<RefActivity>, RepositoryListingError>;
}

pub fn select_active(candidates: Vec<BranchCandidate>, cap: BranchCap) -> Selection;
pub fn evaluate_repository(
    repository: &RepositorySummary,
    evidence: &RepositoryEvidence,
    policy: &ActiveBranchPolicy,
    now: UnixSeconds,
) -> RepositoryEvaluation;
pub fn discover_desired_state(
    listing: &dyn RepositoryListing,
    evidence: &dyn BranchEvidenceSource,
    policy: &ActiveBranchPolicy,
    now: UnixSeconds,
) -> Result<DesiredState, DiscoveryError>;

pub trait DesiredStateStore {
    fn publish(&self, state: &DesiredState) -> Result<(), DesiredStateStoreError>;
    fn load(&self) -> Result<Option<DesiredState>, DesiredStateStoreError>;
}
```

`DesiredState` holds `evaluated_at: UnixSeconds`, the policy, and a
`BTreeMap<RepositoryId, RepositoryDesiredState>`; each entry holds owner, name,
and
`BranchSetOutcome::{Evaluated { active, evicted }, Unavailable {
failure: EvidenceFailure }}`.
`EvidenceFailure` is a closed enumeration of the repository-scoped categories
with `category()` and `from_category()`. `DiscoveryError` distinguishes a
failed repository listing from a credential-wide evidence failure and carries
the category.

The orchestration in `discover_desired_state` calls only ports; it performs no
I/O itself, so it belongs in the core as the application service. Ownership and
reuse policy: `select_active` and `evaluate_repository` are the only
implementation of branch selection; the reconciler (2.2.3), webhook handling
(2.4), and the TUI (5.3) call them and must not re-implement policy.

In `crates/repovec-core/src/repository_listing/summary.rs`:
`RepositorySummary::default_branch(&self) -> Option<&BranchName>` and
`with_details(self, is_private: bool, default_branch: Option<BranchName>)`.

In `crates/repovec-core/src/lib.rs`:
`RuntimePaths::active_branches_snapshot(&self) -> Utf8PathBuf`.

In `crates/repovecd/src/github_rest/profile.rs` (crate-private):
`EndpointPath::{Fixed(&'static str), PerRepository { suffix: &'static str }}`
and `ContinuationParameter::{Page, Cursor(&'static str)}` as fields of
`EndpointProfile<T>`.

In `crates/repovecd`: `github_branch_evidence` (private module implementing
`BranchEvidenceSource for GitHubRepositoryClient`), `atomic_file`
(`pub(crate) fn write_atomically`), `desired_state_store`
(`pub struct FileDesiredStateStore` implementing `DesiredStateStore`), and
`active_branch_command` (`EvaluationRun`, `run_evaluation`, `cli::run`).

Dependencies:
`time = { version = "0.3", default-features = false, features =
["std", "parsing"] }`
in `crates/repovecd/Cargo.toml` (runtime, decision D6). `googletest` and
`pretty_assertions` in `crates/repovec-core` dev-dependencies. Verus and
`rust-prover-tools` are pinned under `tools/` and are not Cargo dependencies.

## Revision note

- 2026-11-22: initial draft from Wyvern reconnaissance and live GitHub probes.
  Awaiting the community-of-experts design review and user approval.
