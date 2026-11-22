# Evaluate active branches for every accessible repository (2.2.2)

This ExecPlan (execution plan) is a living document. The sections `Constraints`,
`Tolerances`, `Risks`, `Progress`, `Surprises & discoveries`, `Decision log`,
`Outcomes & retrospective`, `Conformance basis`, and `Verification plan` must
be kept up to date as work proceeds.

Status: DRAFT (design review applied; awaiting user approval)

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
against the mirrors, worktrees, and indexer units that actually exist, and act
on by creating or deleting them.

After this work an operator, running as the `repovec` user, can run one command
against GitHub:

```sh
cargo run -p repovecd --example evaluate_active_branches -- \
  --user-store /etc/repovec --state-file /var/lib/repovec/active-branches.json
```

and observe, on standard output, one line per repository, for example:

```plaintext
octo-org/widgets: active main, feature/search; 2 evicted
octo-org/archive: held (forbidden since 2026-11-20T08:00:00Z); 3 active kept
octo-org/old-tool: absent since 2026-11-21T09:30:00Z
```

and, in the state file, a versioned, atomically written snapshot of the desired
state that the reconciler will consume. A library caller obtains the same
result as a typed `DesiredState` from
`repovec_core::active_branches::refresh_desired_state`.

The design is shaped by one rule: **the snapshot must never tell the reconciler
to delete something merely because an observation failed.** Concretely:

- A repository whose evidence could not be observed completely is *held*: its
  entry keeps the last successfully evaluated active set, marked with the
  failure and the time it began. It is never recorded as having no active
  branches.
- A repository that disappears from a successful listing becomes *absent*, a
  tombstone that keeps its last known active set and the time it vanished, so
  the reconciler can apply a grace period. A sudden mass disappearance is
  flagged as suspect.
- A run that stops early (rate limit, repeated transport failure, or
  cancellation) still publishes: repositories it did not reach are held with
  their previous state, and the next run resumes where this one stopped.
- Only failures that invalidate the whole run publish nothing: authentication
  rejected, invalid configuration, a failed repository listing, or an appliance
  clock that disagrees with GitHub's by more than five minutes.

Existing behaviour is preserved: the `list_github_repositories` example, the
repository listing port, and every 2.2.1 error category keep their contracts.
`RepositorySummary` gains two optional, additive observations (`pushed_at` and
`open_issues_count`) used to skip needless requests.

## Constraints

- Implementation begins only after the user explicitly approves this plan.
- Keep domain types, the policy, the selection kernel, discovery, and the
  driven ports in `repovec-core`, with no HTTP, JSON, filesystem, environment,
  or clock access there. Put the GitHub adapter, the snapshot file adapter, and
  the one-shot composition in `repovecd`. Dependencies point inward; adapters
  never call one another; sibling core modules must not depend on each other
  cyclically.
- Every GitHub REST listing goes through the crate-private
  `repovecd::github_rest` traversal (ADR 0002 and developers' guide section
  9.2). Do not add a second traversal, a trait object over traversals, retries,
  or backoff.
- Do not implement the periodic scheduler, the desired-versus-actual diff, or
  removal grace periods (2.2.3), mirrors or worktrees (2.3), webhooks (2.4),
  indexer lifecycle (3.x), TUI configuration (5.3), metrics export, or GitHub
  App token issuance. Do not add a daemon configuration file or adopt
  `ortho-config` in this item (decision D9).
- Do not filter repositories by archive state, fork state, privacy, or owner
  type, and do not special-case bot activity (decision D15).
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
- Authorized dependency and toolchain changes, effective only on plan
  approval:
  - runtime: `time` `0.3` (caret; `0.3.53` is already in `Cargo.lock`) with
    default features off and the `std` and `parsing` features, in `repovecd`
    only (decision D6);
  - development: `googletest` and `pretty_assertions` (already workspace
    dependencies) in `repovec-core`'s dev-dependencies;
  - workspace `rust-version` from `1.85` to `1.89`, for `std::fs::File::lock`
    (decision D16); the pinned nightly toolchain is unaffected;
  - verification tooling outside the Cargo graph: the pinned Verus release and
    the `rust-prover-tools` commit pin (decision D11).
- Interface changes are authorized only as listed in
  `Interfaces and dependencies`. All are pre-1.0, application-internal surfaces
  with no external consumer, so callers change together and no compatibility
  layer is added.
- The snapshot file is a new persisted format with an explicit
  `schema_version` from its first release (decision D8); no earlier deployed
  format exists, so no migration is required.
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
- Configuration: `Cargo.toml`, `Cargo.lock`, `.config/**`, `.gitignore`,
  `tools/**` except `*.md`, `typos.local.toml`, and `.github/**`. `typos.toml`
  is generated and excluded; if it changes, commit it separately.
- Documentation: `*.md` outside `docs/execplans/`.

Budgets, each enforced independently:

- Files: stop and escalate above 115 changed non-ExecPlan files.
- Code: stop above 14,500 net lines in total; within that, stop above 4,900
  net lines of production Rust (non-test files under `crates/*/src` and
  `crates/*/examples`, excluding `tests.rs`, `tests_*.rs`, `*_tests.rs`,
  `test_support.rs`, `*_properties.rs`, and `tests/` directories) and above 900
  net lines under `verus/`.
- Configuration: stop above 300 net lines.
- Documentation: stop above 1,500 net lines.
- Interface: stop if any public signature beyond `Interfaces and dependencies`
  must change.
- Dependencies: stop if any crate or toolchain change beyond `Constraints` is
  needed.
- Architecture: stop if `github_rest` must change beyond a profile-named
  continuation parameter, the ordered early-stop cutoff, and the single-request
  status probe (decision D3), or if the policy semantics in D4 cannot be
  implemented as specified.
- Verification: stop if a planned Verus lemma cannot be proved without
  `assume`, `admit`, `external_body`, or a new axiom after three focused
  attempts, or if `EP-M1` exceeds one working day; record the open goal and the
  options (narrow the lemma, or move the proof to a follow-up item) in
  `Decision log`.
- Iterations: stop if a focused test still fails after five fix attempts.
- Time: stop if any other milestone exceeds two working days of elapsed effort.
- Ambiguity: stop if GitHub's live behaviour contradicts a recorded axiom
  (`AXIOM-*` below) or a planned probe (P7 to P10) disagrees with the plan.

These numbers come from measured history. Roadmap item 2.2.1 recorded no
numeric budget; measured afterwards (`git diff --numstat ffacb2f^ ffacb2f`), it
changed 81 files: 64 Rust files with 10,845 net lines (about 3,900 production,
6,975 test), 7 Markdown files with 466 net lines outside ExecPlans, and about
150 configuration lines. Its scope grew from "endpoint-specific" to a shared
traversal mid-plan. Roadmap item 1.3.3 set 24 files and about 1,000 net lines
excluding tests, landed near 1,700, and needed a user-approved extension; the
miss was in test and asset code that its budget excluded. Items 1.2.2 (12
files, 650 lines), 1.2.3 (14 files, 850 lines), and 2.1.1 (18 files, 900 lines)
also excluded tests and recorded no increase. The design review grew this
item's scope (carry-forward, tombstones, early stop, clock check, publication
lock), raising the estimate traced in `Context and orientation` to about 4,050
production lines, 7,900 test lines, 650 proof lines, 200 contract-test lines,
and 1,250 documentation lines. The budgets add roughly 20 per cent contingency
and, unlike the earlier plans, include tests so the dominant cost is visible.

## Risks

- Risk: GitHub's per-repository Link continuations use the canonical
  `/repositories/{id}/...` path. Severity: high. Likelihood: certain
  (observed). Mitigation: request the canonical path from the first page
  (decision D2); protocol tests pin it.
- Risk: the activity endpoint paginates with an opaque `after` cursor, which
  `github_rest` currently rejects. Severity: high. Likelihood: certain.
  Mitigation: a profile-named continuation parameter (D3, ADR 0003).
- Risk: the documented activity example disagrees with live responses
  (`timestamp`, `activity_type`, `actor`). Severity: medium. Likelihood:
  certain. Mitigation: decode the observed fields; `AXIOM-activity-shape`; a
  shape change holds the repository with `invalid_response`.
- Risk: GitHub may serve activity data a few hours old because of caching.
  Severity: medium. Likelihood: high. Mitigation: a branch whose listed head
  differs from the `after` commit of its newest activity record has moved since
  the cached data, so it is treated as active now (D4).
- Risk: the activity endpoint silently ignores an unknown `time_period`, and
  the span of `month`, `quarter`, and `year` is undocumented. Severity: medium.
  Likelihood: certain. Mitigation: map windows to periods through a closed
  enumeration that picks the next larger period than strictly needed, filter by
  timestamp client-side, and run probe P7 to measure the spans.
- Risk: busy repositories exceed the activity record budget. Severity: high.
  Likelihood: medium (622 records per quarter in this small repository).
  Mitigation: an ordered early-stop cutoff ends the traversal once records are
  older than the window, and the traversal verifies newest-first ordering
  instead of trusting it (D3).
- Risk: request volume against the 5,000-per-hour primary limit. Severity:
  medium. Likelihood: medium. Mitigation: skip the activity traversal when the
  repository's `pushed_at` precedes the window, skip the pull request traversal
  when `open_issues_count` is zero or the policy excludes pull requests, stop
  requests at the first rate-limit response, publish partial results with
  carry-forward, and resume from the stopping point next run (D7, D14).
- Risk: the appliance clock is wrong at boot, making every branch look old.
  Severity: high. Likelihood: low. Mitigation: compare `now` with GitHub's
  `Date` header from the free `/rate_limit` probe and abort above five minutes
  of skew (D13).
- Risk: an organization's repositories vanish from a successful listing
  (expired SAML single sign-on authorization, changed token scope). Severity:
  high. Likelihood: medium. Mitigation: tombstones and the mass-shrink flag
  (D8); 2.2.3 owns the removal grace period.
- Risk: concurrent writers (this command, the 2.2.3 daemon, 2.4 webhook
  updates) replace a fresher snapshot with an older one. Severity: medium.
  Likelihood: medium. Mitigation: exclusive advisory lock and a monotonic
  `evaluated_at` check at publication (D16).
- Risk: churn at the cap boundary causes repeated worktree deletion and
  re-embedding. Severity: medium. Likelihood: medium. Mitigation: per-run
  `branches_added` and `branches_removed` counts against the previous snapshot;
  D4 explicitly permits a 2.2.3 removal grace period.
- Risk: Verus tooling is new to this repository and the proof covers a model.
  Severity: medium. Likelihood: medium. Mitigation: `EP-M1` lands tooling and a
  declarative model first; the production kernel is shaped as "filter, sort by
  a pinned derived-`Ord` key, take a prefix" so the proved prefix lemma plus
  documented standard-library contracts connect model and code; bridge tests
  close the remaining gap; a one-day tolerance stops the milestone from holding
  up the deliverable.
- Risk: fine-grained tokens without Contents read, or pull-only
  collaborators, may be refused the activity listing. Severity: medium.
  Likelihood: unknown. Mitigation: probe P10; refusals are repository-scoped
  holds with the last known state, visible in the summary and the snapshot.
- Risk: branch names valid in Git are unsafe as filesystem paths in 2.3.2
  (case folding, Unicode normalization, `a` versus `a/b` nesting). Severity:
  medium. Likelihood: low. Mitigation: `BranchName` deliberately implements no
  path conversion; the technical design records the 2.3.2 mapping obligation.
- Risk: the snapshot is created root-owned when an operator runs the example
  as root. Severity: low. Likelihood: medium. Mitigation: `--state-file` is
  required (no default), and the users' guide instructs running as `repovec`.

## Progress

- [x] (2026-11-22 19:05Z) Branch renamed and Lody session titled.
- [x] (2026-11-22 19:40Z) Reconnaissance (four Wyvern agents) and live GitHub
  probes P1 to P6 completed.
- [x] (2026-11-22 20:30Z) Initial draft written and committed.
- [x] (2026-11-22 21:30Z) Community-of-experts design review (structure and
  contracts, failure modes and scale, alternatives and viability, verification
  rigour) completed and applied; see Revision note.
- [ ] Plan approved by the user.
- [ ] `EP-M1` Verus on-ramp and proved selection model.
- [ ] `EP-M2` Core vocabulary, kernel, and repository evaluation.
- [ ] `EP-M3` Traversal amendments and GitHub evidence adapter.
- [ ] `EP-M4` Discovery, carry-forward, and the snapshot store.
- [ ] `EP-M5` One-shot composition, end-to-end, and behavioural evidence.
- [ ] `EP-M6` Documentation closure and roadmap completion.

## Surprises & discoveries

- Observation: every per-repository listing (`branches`, `pulls`, `activity`)
  returns `Link` continuations on `/repositories/{numeric id}/...` even when
  the first request used `/repos/{owner}/{repo}/...`. Evidence: probes P1 to
  P3. Impact: the existing path confinement would reject page two; D2.
- Observation: the activity endpoint paginates with `after=<cursor>` and also
  advertises `rel="prev"` with `before=<cursor>`. Evidence: probe P3. Impact:
  D3.
- Observation: live activity records carry `timestamp`, `activity_type`, and
  `actor`; the documentation example shows `pushed_at`, `push_type`, and
  `pusher`. The shape is identical under API versions `2022-11-28` and
  `2026-03-10`. Evidence: probes P3 and P4. Impact: `AXIOM-activity-shape`.
- Observation: an unrecognized `time_period` value is ignored rather than
  rejected. Evidence: probe P5. Impact: closed period enumeration and mandatory
  client-side filtering.
- Observation: one quarter of this repository's activity held 622 records
  across five types (`branch_creation` 159, `branch_deletion` 143, `force_push`
  83, `pr_merge` 110, `push` 127). Evidence: probe P6. Impact: early-stop
  cutoff (D3).

## Decision log

- D1. Decision: deliver a library use case plus a one-shot example
  (`evaluate_active_branches`), mirroring 2.2.1, rather than a daemon loop.
  Rationale: the scheduler is 2.2.3; the example is the operator-observable
  acceptance surface and the end-to-end test drives the same composition
  in-process. Date/Author: 2026-11-22, planning agent.
- D2. Decision: per-repository requests use GitHub's canonical
  `/repositories/{id}/{suffix}` path, which the adapter formats from the
  validated numeric `RepositoryId` and passes as the traversal's initial URL.
  `github_rest` gains no path abstraction. Rationale: GitHub's own
  continuations use this path, so the unchanged "same path as the first page"
  confinement accepts them; numeric identifiers remove path injection from
  owner or repository names, and the path follows repository renames and
  transfers. The rejected alternatives were an alias path set in confinement,
  which widens every listing's trust boundary, and an `EndpointPath` enum,
  which would pull a domain identifier into `github_rest` for no gain.
  Date/Author: 2026-11-22, planning agent, revised after design review.
- D3. Decision: three bounded `github_rest` amendments, recorded in ADR 0003,
  which amends ADR 0002.
  - `EndpointProfile` names its continuation parameter: `page` for numbered
    listings, `after` for the activity listing. Confinement lets only that
    parameter vary, rejects an empty value, and still rejects every other
    added, dropped, repeated, or changed parameter, including `before`.
  - An optional ordered cutoff: a profile may declare that its records are
    ordered newest first by a timestamp, and the caller may supply a cutoff.
    The traversal fails with `invalid_response` if any record is newer than
    its predecessor (so the ordering is checked, not trusted) and completes
    successfully after the first page whose oldest record precedes the cutoff.
    A traversal so ended is complete for the window, not for the listing.
  - A single-request status probe sharing the traversal's status
    classification, body bounds, and redirect policy, used only for
    `GET /rate_limit`, which GitHub does not count against the primary limit.
  Rationale: each change is the smallest that admits a required behaviour while
  keeping one implementation of GitHub's protocol. Date/Author: 2026-11-22,
  planning agent, revised after design review.
- D4. Decision: policy semantics.
  - The candidate set is the observed branch list. A name observed twice with
    the same head collapses to one candidate; a name observed with different
    heads holds the repository as `inconsistent_evidence`.
  - A branch is *pinned* when its name equals the repository's default branch.
  - The *recency* of a branch is the latest of: the timestamp of its newest
    qualifying activity record inside the window; the `updated_at` of a
    qualifying open pull request whose head is the branch, if inside the
    window; and `now` when the branch's listed head differs from the `after`
    commit of its newest activity record (the cached activity is stale).
    Qualifying activity types are `push`, `force_push`, `pr_merge`,
    `merge_queue_merge`, and `branch_creation`; `branch_deletion` and
    unrecognized types never qualify. A branch with no such evidence has no
    recency.
  - A branch is *recent* when it has a recency; *pull-request referenced* when
    the policy includes pull requests and an open pull request's head
    repository identifier equals this repository's identifier and its head
    branch equals the name (fork pull requests never qualify); *admissible*
    when pinned, recent, or pull-request referenced.
  - Ranking puts the pinned branch first, then more recent before less recent
    (no recency ranks last), then names in ascending byte order. The first
    `cap` admissible branches are active; the rest are *evicted* and reported.
  - Evaluation is stateless and depends only on the observation, the policy,
    and `now`, with all evidence clipped to the window. A removal grace period
    or hysteresis applied by the 2.2.3 reconciler when acting on the desired
    state is not a re-implementation of this policy and is permitted.
  Rationale: GitHub records when refs change, not when the appliance last
  served a branch, so "least recently used" is interpreted as least recently
  active, with an open pull request's update counting as use. Statelessness
  keeps evaluation idempotent and provable. Date/Author: 2026-11-22, planning
  agent, revised after design review. Status: requires user confirmation at
  approval.
- D5. Decision: defaults are a 14-day window, a cap of 10 active branches per
  repository (the default branch counts towards the cap), and pull requests
  included. The window accepts 1 to 364 days and the cap 1 to 100. Rationale:
  the technical design gives no values; these keep a typical team repository
  fully indexed while bounding pathological ones. Date/Author: 2026-11-22,
  planning agent, revised after design review. Status: requires user
  confirmation at approval.
- D6. Decision: parse GitHub timestamps with the `time` crate's RFC 3339
  parser in the adapter and carry `UnixSeconds(u64)` into the domain.
  Rationale: date arithmetic is a classic source of defects; `time` is already
  in the lockfile; the domain stays integer-only, which suits Verus. The
  rejected alternative, a handwritten fixed-format parser, would need its own
  calendar proof. Date/Author: 2026-11-22, planning agent.
- D7. Decision: failure scope, decided by an exhaustive `match` in
  `FailureScope::of(&RepositoryListingError)`, never by category strings.
  - Run-invalidating (publish nothing): `authentication_rejected`,
    `invalid_input` (configuration), any failure of the repository listing
    itself, and `clock_skew` from the status probe.
  - Run-stopping (stop issuing requests; publish with the unreached
    repositories held as `not_reached`): `rate_limited`, five consecutive
    `transport_failure` results, and caller cancellation.
  - Repository-scoped (hold that repository; continue): every other category,
    including a single `transport_failure` and 5xx `unexpected_status`.
  Rationale: only failures that make the whole observation untrustworthy should
  block publication; a rate limit or an outage part-way through must not freeze
  the snapshot forever, and a transient fault on one repository must not
  discard the others. Date/Author: 2026-11-22, planning agent, revised after
  design review.
- D8. Decision: persist the desired state as a JSON snapshot. The core
  defines a `DesiredStateStore` port; `repovecd` supplies the file adapter; ADR
  0004 holds the normative schema with an example. Schema version 1 is a
  top-level object with `schema_version`, `evaluated_at`, the policy,
  `listing_shrink_suspected`, `resume_after` (optional repository identifier),
  and `repositories`, an array ordered by numeric `id`. Each entry has `id`,
  `owner`, `name`, and a tagged `state`:
  - `evaluated` with `observed_at`, `active` (objects with `name` and
    `reasons`), and `evicted` (names);
  - `held` with `failure` (category string), `since`, and `last_known`
    (optional evaluated set with its `observed_at`);
  - `absent` with `since` and `last_known`.
  Readers tolerate unknown fields; an unrecognized `failure` category loads as
  `unrecognized` and still means hold; `schema_version` changes only when a
  version 1 reader would misread the file. Tombstones older than 30 days are
  dropped. `listing_shrink_suspected` is set when more than the larger of 10
  repositories or 25 per cent of the previously present repositories become
  absent in one run. `evicted` and `reasons` are informational; the reconciler
  consumes only the state tags and active names. The file lives where the
  caller says; the appliance location is
  `RuntimePaths::active_branches_snapshot()`
  (`/var/lib/repovec/active-branches.json`) in the `repovec`-owned `0700` data
  root, mode `0600`. Rationale: the roadmap requires the desired state to be
  stored for the reconciler; carry-forward and tombstones need the previous
  state; an operator needs an inspectable record; using the existing data root
  avoids changing the 1.3.3 directory contract. The design review's
  alternative, an in-memory hand-over with persistence deferred to 2.2.3, was
  rejected because it cannot carry state across runs, which the fail-safe rules
  above require. Date/Author: 2026-11-22, planning agent, revised after design
  review.
- D9. Decision: do not adopt `ortho-config` or a daemon configuration file in
  this item. `ActiveBranchPolicy` is a validated value with defaults; the
  one-shot command exposes `--recent-days`, `--max-branches`, and
  `--without-pull-requests`. Rationale: no crate depends on `ortho-config` yet;
  the first daemon configuration surface arrives with the 2.2.3 scheduler, and
  the TUI (5.3) owns policy editing. The policy constructor is the seam
  configuration will call. Date/Author: 2026-11-22, planning agent.
- D10. Decision: evidence limits are a separate `EvidenceLimits` value
  (`branches`, `pulls`, `activity`, each a `ListingLimits`) passed to the
  evidence adapter, independent of the repository listing's limits. Defaults:
  branches and pulls cap pages at 100; activity caps pages at 200 and relies on
  the early-stop cutoff. Reaching a limit is `resource_limit_exceeded`, which
  is repository-scoped. Tests shrink them through the constructor. Date/Author:
  2026-11-22, planning agent, revised after design review.
- D11. Decision: prove a declarative selection model with Verus (`EP-M1`) and
  bridge it to the production kernel with tests; do not use Kani. Rationale:
  the user requires Verus for contractual business logic, and the developers'
  guide, section 9.6, names a pure policy kernel as the trigger. The design
  review showed that re-proving insertion sort would prove a sort the
  production code does not use; the model therefore defines selection
  declaratively (a candidate is selected exactly when it is admissible and
  fewer than `cap` admissible candidates outrank it) and proves the prefix
  lemma that connects any outranks-sorted sequence to that definition, which is
  exactly the step the production code takes after `sort_by`. The alternatives
  reviewer argued for dropping Verus as disproportionate; that view is recorded
  and rejected because of the explicit requirement, with the one-day tolerance
  as the safeguard. Kani is not used: the proof covers the model, the only
  exhaustively checked refinement step is `RankKey` against `outranks`, and the
  evidence-to-candidate mapping, filter, and prefix are sampled by property
  tests against a brute-force oracle; that residual gap is accepted.
  Date/Author: 2026-11-22, planning agent, revised after design review.
- D12. Decision: `RepositorySummary::default_branch` stays the 2.2.1 raw
  string. `evaluate_repository` validates it as a `BranchName`; an invalid
  default branch holds that repository as `invalid_response`. Rationale: the
  design review showed that validating in the listing conversion would turn one
  odd repository into a run-invalidating listing failure, and would create a
  dependency cycle between `repository_listing` and `active_branches`.
  `BranchName` lives in a neutral `repovec_core::git_ref` module. Date/Author:
  2026-11-22, planning agent, revised after design review.
- D13. Decision: `now` is a wall-clock `UnixSeconds` read once from
  `SystemTime::now()` at the composition root. Before any listing, discovery
  calls the status probe; if `|now - Date| > 300` seconds it aborts with
  `clock_skew`. Durations in telemetry continue to use `monotony`'s
  `MonotonicClock`. Rationale: the policy compares against GitHub's wall-clock
  timestamps, which a monotonic clock cannot express, and a wrong clock would
  make every branch look stale. The developers' guide gains an explicit
  carve-out. Date/Author: 2026-11-22, planning agent, revised after design
  review.
- D14. Decision: request economy. `RepositorySummary` gains optional
  `pushed_at` and `open_issues_count`, decoded from the existing listing
  responses. Discovery skips the activity traversal when `pushed_at` precedes
  `now - window - 86,400` seconds (one day of slack for cached values) and
  skips the pull request traversal when `open_issues_count` is zero or the
  policy excludes pull requests; a skipped traversal contributes no evidence. A
  repository with no default branch skips every evidence traversal and is
  evaluated as empty. Repositories duplicated in the listing are deduplicated
  by identifier, first observation wins. Repositories are processed in
  ascending identifier order starting after the previous run's `resume_after`,
  wrapping around. Rationale: most repositories are dormant; probe P9 confirms
  which pushes update `pushed_at` before relying on it. Date/Author:
  2026-11-22, planning agent, added after design review.
- D15. Decision: bot activity is not special-cased in this item. Rationale:
  the technical design is silent; a filter is a policy surface for 5.3 to add
  through `ActiveBranchPolicy` if needed. Date/Author: 2026-11-22, planning
  agent, added after design review.
- D16. Decision: the file store takes an exclusive `std::fs::File::lock` on a
  sibling `<file>.lock` for the read-compare-write of `publish`, and refuses
  with `superseded` when the stored `evaluated_at` is newer than the snapshot
  being published. This raises the workspace `rust-version` to 1.89. Rationale:
  the one-shot command, the 2.2.3 daemon, and 2.4 targeted updates will
  overlap; a lock in the standard library needs no new dependency. Date/Author:
  2026-11-22, planning agent, added after design review.
- D17. Decision: telemetry is structured `tracing` events now; `metrics`
  emission is deferred to the item that installs an exporter (2.2.3 or later).
  Each run emits one completion event with counts of repositories by state,
  `branches_added`, `branches_removed`, the oldest `held` `since`, the status
  probe's remaining quota, and elapsed milliseconds. Rationale: `AGENTS.md`
  requires an exercised collection path for new metrics, and none exists yet.
  Date/Author: 2026-11-22, planning agent, added after design review.
- D18. Decision: the command's credential modes move from
  `repository_listing_command` to a shared `repovecd::github_credential_source`
  module (`ListingMode`, `UserTokenLoader`, acquisition), in a separate
  refactor commit, and both commands use it. Rationale: two commands share one
  credential contract; importing one command's internals from another couples
  them. Date/Author: 2026-11-22, planning agent, added after design review.
- D19. Decision: deliver on this one branch as milestone commits rather than
  a stack of pull requests. Rationale: the plan and its implementation share a
  branch by convention in this repository; milestones are coherent rollback
  points. The design review's suggestion to split the traversal amendments into
  their own pull request remains open for the user to choose at approval.
  Date/Author: 2026-11-22, planning agent.

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
  A failure returns no partial list; the list preserves repeated observations.
- `summary.rs` defines `RepositorySummary` (`id()`, `owner()`, `name()`,
  `is_private()`, `default_branch() -> Option<&str>`, built with `new` and
  `with_details`); `identity.rs` defines `RepositoryId` (non-zero `u64`),
  `RepositoryOwner`, and `RepositoryName` (non-empty, no control characters,
  otherwise unrestricted); `error.rs` defines `RepositoryListingError`
  (`InvalidInput`, `AuthenticationRejected`, `Forbidden`, `RateLimited`,
  `TransportFailure`, `UnexpectedStatus`, `InvalidResponse`,
  `InvalidContinuation`, `CycleDetected`, `ResourceLimitExceeded`), each with a
  stable `category()` string.
- `crates/repovecd/src/github_rest/` is the crate-private traversal.
  `profile.rs` defines
  `EndpointProfile<T> { path: &'static str, fixed_query, items: PageItems }`.
  `traversal.rs` defines
  `TraversalRunner { client, credential, profile, initial: Url, limits }` and
  `Traversal<T> { records, pages, bytes }`; the caller builds `initial`.
  `link/mod.rs` confines a continuation to the initial origin and path, and
  `ensure_query` lets only `page` vary. `limits/mod.rs` defines `ListingLimits`
  (defaults: 10,000 pages, 1,000,000 records, 8 MiB per body, 64 MiB total, 16
  KiB Link value, 16 KiB continuation identity). `status/mod.rs` classifies
  statuses; only `200` is a page. `test_support.rs` exposes seams behind
  `test-support`.
- `crates/repovecd/src/github_repository_client/mod.rs` defines
  `GitHubRepositoryClient` (`new`, `with_http_client`), two repository
  profiles, and the `RepositoryListing` implementation; `wire.rs` holds
  `RepositoryWire` and `into_domain`.
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
  (`repository_listing_e2e.rs`), with helpers under `tests/common/`.
  `repovec_test_helpers::recording_http` provides `RecordingServer` and
  `ScriptedResponse`; `repovec_test_helpers::repository_listing_wire` provides
  JSON builders, `link_header`, and fake tokens;
  `repovec_test_helpers::capture_logs` captures `tracing` output.

Test and verification tooling: `rstest` 0.28, `rstest-bdd` 0.5 (with
`strict-compile-time-validation`), `proptest` 1.11, `insta` 1.47 (`yaml`), and
`googletest` 0.14 are workspace dependencies; `pretty_assertions` 1.4 is
present. `cargo nextest` runs when installed (`scripts/run_rust_tests.py`). No
Kani, Verus, or `rust-prover-tools` wiring exists in the repository. The host
provides `~/.local/bin/prover-tools`. The latest Verus release at drafting is
`release/0.2026.11.02.8bf68ee` (the newer rolling release is not a pin
candidate); `leynos/rust-prover-tools` `main` is
`6be9e71bc3ce98e96087be74b4cc572ca3488cb0`. The `chutoro` repository is estate
precedent for Verus wiring (`docs/verus-toolchain.md` there).

Gates: `make check-fmt`, `make lint` (interrogate, `cargo doc`, Clippy with
`-D warnings`, Whitaker), `make test` (script tests, nextest or cargo test,
doctests), `make markdownlint` (runs `make spelling` first), `make nixie`, and
`make test-workflow-contracts`. No target runs proofs today.

Constraints from documentation: developers' guide section 9.2 forbids a second
traversal; section 9.3 lists the steps for adding an endpoint profile; section
9.6 names a pure policy kernel as the trigger for formal proof; the device-flow
section mandates `MonotonicClock` for elapsed-time decisions; section 8 forbids
environment reads outside composition roots.

Budget survey. Estimated net lines per area, from 2.2.1's file sizes and the
reviewed scope:

- core `git_ref` (`BranchName`, `CommitId`) and `active_branches`
  (instants, policy, evidence, kernel, evaluation, failure scope, discovery
  with carry-forward and tombstones, desired state, store port, refresh
  command): about 1,900 production lines;
- `RepositorySummary` additions and wire decoding: about 80;
- `github_rest` amendments (continuation parameter, ordered cutoff, status
  probe): about 250;
- GitHub evidence adapter (three wire records and profiles, port
  implementation, period mapping, telemetry): about 550;
- shared `atomic_file` and `github_credential_source` modules (moved), and
  the snapshot store adapter with wire format and lock: about 550;
- one-shot command, command line, output, and example: about 720;
- tests across both crates: about 7,900 lines (2.2.1's ratio of about 1.8
  plus the carry-forward and tombstone tables);
- Verus model and lemmas: about 650; Makefile and Python contract tests:
  about 200;
- documentation: technical design (about 180), ADR 0003 (about 150), ADR 0004
  (about 230), users' guide (about 200), developers' guide (about 300),
  `tools/verus/README.md` (about 40), contents, roadmap, and ADR 0002 banner
  (about 30).

Signposts. Read these before implementing:

- `docs/repovec-appliance-technical-design.md`, sections "Repository listing
  (2.2.1)", "Discovery and continuous monitoring", "Active branch policy", and
  "Worktrees and checkout layout";
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
  for navigation when the CodeGraph server is available (it failed to connect
  during planning).

## Conformance basis

Upstream artefacts, inspected at `f4da025`:

- Terms of Reference: none exists for this repository; the roadmap and the
  technical design are the governing artefacts.
- `docs/roadmap.md`, item 2.2.2 and the objective of section 2.2.
- `docs/repovec-appliance-technical-design.md`, sections "Active branch
  policy" (`TDD-active-branch-policy`), "Discovery and continuous monitoring"
  (`TDD-discovery`), "Repository listing (2.2.1)", and "Worktrees and checkout
  layout".
- `docs/adrs/adr-0002-shared-github-rest-traversal.md` (Accepted,
  2026-11-19) and `docs/adrs/adr-0001-repovec-directory-provisioning.md`.
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
- `REQ-store`: the desired state is stored for the reconciler.
- `REQ-configurable`: the policy parameters are values a later configuration
  surface can set.
- `REQ-fail-safe`: a failed, partial, or missing observation never shrinks a
  repository's recorded active set or erases its last known state (derived from
  the 2.2.1 "not a snapshot" contract and the reconcile-first design).

Gaps against the baseline:

- `GAP-per-repo-path`: per-repository endpoints need a path built from a
  repository identifier.
- `GAP-cursor`: continuation confinement admits only `page`.
- `GAP-volume`: activity listings are unbounded in time without a cutoff.
- `GAP-branch-name`: no validated branch-name type exists.
- `GAP-wall-clock`: no wall-clock seam exists; guidance forbids wall-clock
  time for elapsed-time decisions.
- `GAP-store`: no persisted state format or shared atomic-write helper exists.
- `GAP-prover`: no proof tooling exists.

Trace:

```plaintext
TDD-active-branch-policy -> REQ-default, REQ-recent, REQ-pr, REQ-cap
  -> D4, D5 -> EP-M1 (L-prefix, INV-*) -> EP-M2 (select_active, evaluate_repository)
  -> tests: active_branch_policy_bdd, kernel_bridge, evaluation_properties
TDD-discovery -> REQ-list, GAP-per-repo-path, GAP-cursor, GAP-volume -> D2, D3, D10, D14
  -> EP-M3 -> tests: github_branch_evidence*, activity_pagination_properties
TDD-discovery -> REQ-store, REQ-fail-safe, GAP-store -> D7, D8, D16 -> EP-M4
  -> tests: discovery_tests (scope table, carry-forward, tombstones), desired_state_store
REQ-list..REQ-fail-safe -> D1, D13, D17 -> EP-M5 -> tests: active_branch_e2e,
  active_branch_discovery_bdd, active_branch_events
REQ-configurable -> D5, D9 -> EP-M2 (ActiveBranchPolicy) -> EP-M5 (flags)
GAP-branch-name -> D12 -> EP-M2 -> tests: branch_name table and git oracle
GAP-wall-clock -> D13 -> EP-M5 composition root -> developers' guide carve-out
GAP-prover -> D11 -> EP-M1 -> make verus, workflow contract tests
```

## Verification plan

### Abstract model

A repository observation is a tuple `(R, B, A, P)`:

- `R` is the repository summary: identifier, optional default branch `d`,
  optional `pushed_at`, optional `open_issues_count`.
- `B` is a finite sequence of observed branches `(name, head)`.
- `A` is a finite sequence of branch activity records
  `(name, kind, t, after)` with `t` in Unix seconds.
- `P` is a finite sequence of open pull request heads
  `(head_repository: Option<id>, head_branch, updated_at)`.

A policy is `(w, c, p)`: window `w` seconds (`w = days * 86,400`, days in
1..=364), cap `c` in 1..=100, and the pull request flag `p`. `now` is Unix
seconds. `in_window(t) := now.saturating_sub(w) <= t`.

Evaluation of one repository:

1. If `d` is absent, the outcome is `Evaluated(∅, ∅)` and no evidence is
   fetched. If `d` is not a valid branch name, the outcome is
   `Held(invalid_response)`.
2. Deduplicate `B` by name; differing heads under one name yield
   `Held(inconsistent_evidence)`. If `B` is non-empty and does not contain `d`,
   the outcome is `Held(inconsistent_evidence)`. An empty `B` yields
   `Evaluated(∅, ∅)`.
3. For each name `n`, compute `pinned(n) := d == n`, `recency(n)` as in D4,
   `referenced(n)`, and `admissible(n)`. Each candidate is
   `(name, pinned, admissible, last := recency)`.
4. Select with the kernel.

The candidate model used by the proof is a finite sequence `s` of candidates
`{ name: nat, pinned: bool, admissible: bool, last: Option<nat> }`, with `name`
abstracting `BranchName` under a strict total order. Well-formedness:

```plaintext
wf(s) := names in s are pairwise distinct
         && (forall i. s[i].pinned ==> s[i].admissible)
         && (at most one i with s[i].pinned)
```

`outranks(x, y)` holds when `x.pinned && !y.pinned`; or `x.pinned == y.pinned`
and `x.last > y.last` (with `Some(_) > None`); or `x.pinned == y.pinned`,
`x.last == y.last`, and `x.name < y.name`.

Declarative selection, independent of any sorting algorithm:

```plaintext
selected(s, c, x) := x in s && x.admissible
                     && |{ y in s | y.admissible && outranks(y, x) }| < c
evicted(s, c, x)  := x in s && x.admissible && !selected(s, c, x)
```

The production kernel computes `take(sort_desc(filter(admissible, s)), c)` where
`sort_desc` is `slice::sort_by` with the `RankKey` comparator.

Discovery over a listing and a previous snapshot `S0`:

- run-invalidating failures return an error and publish nothing;
- each listed repository, in rotation order, is evaluated, or held with the
  failure and `S0`'s last known set, or, after a run-stopping failure, held as
  `not_reached` with `S0`'s state carried forward;
- each repository in `S0` but not in the listing becomes `Absent` (or stays
  absent, keeping its original `since`), and tombstones older than 30 days are
  dropped;
- the result is published through the store under the lock and monotonic
  check.

### Invariants and lemmas

Selection, proved in Verus over the candidate model:

- `L-rank-order`: under `wf`, `outranks` is irreflexive, transitive, and total
  on distinct names (exactly one of `outranks(x, y)`, `outranks(y, x)` holds for
  `x != y`).
- `L-prefix`: under `wf(s)`, if `t` is a permutation of
  `filter(admissible, s)` with `outranks(t[i], t[j])` for all `i < j`, then for
  every `x`, `x in take(t, c)` if and only if `selected(s, c, x)`. This is the
  refinement lemma connecting the production shape to the declarative spec.
- `INV-bounded` (`REQ-cap`): `|{x | selected(s, c, x)}| == min(c,
  | filter(admissible, s) | )`. |
- `INV-admissible`: `selected(s, c, x)` implies `x in s` and `x.admissible`.
- `INV-default` (`REQ-default`): under `wf(s)` and `c >= 1`, every pinned `x`
  in `s` is selected.
- `INV-lru` (`REQ-cap`), stated over fields rather than over `outranks` so a
  wrong ranking cannot satisfy it: for every selected `x` and evicted `y`,
  `x.pinned || !y.pinned`; and if `x.pinned == y.pinned` then
  `x.last >= y.last` (with `None` least); and if both are equal then
  `x.name < y.name`.
- `INV-cap-monotone`: `c1 <= c2` and `selected(s, c1, x)` imply
  `selected(s, c2, x)`.
- `INV-order-independent-model`: `selected` depends only on the set of
  elements of `s`, so any permutation of `s` selects the same elements.

Evaluation and discovery, established by tests:

- `INV-order-independent` (property test): permuting `B`, `A`, or `P` leaves
  the names in the active and evicted sets unchanged, or yields the same `Held`
  category.
- `INV-evidence` (`REQ-recent`, `REQ-pr`): recency uses only qualifying
  activity types, the branch namespace, in-window instants (inclusive
  boundary), same-repository pull requests, and the stale-head rule; fork and
  `null` head repositories never contribute; `p == false` and
  `open_issues_count == 0` skip the pull request traversal; an old `pushed_at`
  skips the activity traversal.
- `INV-window-period`: for every window of 1 to 364 days, the activity period
  chosen covers at least the window under `AXIOM-activity-period`.
- `INV-scope` (`REQ-fail-safe`): every `RepositoryListingError` variant maps
  to exactly the scope D7 assigns.
- `INV-fail-safe` (`REQ-fail-safe`): for every previous snapshot and every
  sequence of per-repository outcomes, a repository that is held, not reached,
  or absent keeps its previous last known set; no failure produces an evaluated
  entry; no evaluated entry is produced for a repository whose evidence failed.
- `INV-no-partial-publish`: a run-invalidating failure never calls
  `DesiredStateStore::publish`, and the stored file stays byte-identical.
- `INV-tombstone`: absent entries keep their first `since`, are dropped after
  30 days, and `listing_shrink_suspected` is set exactly when the threshold in
  D8 is crossed.
- `INV-resume`: after a run-stopping failure, `resume_after` names the last
  repository reached, and the next run starts with the repository after it.
- `INV-round-trip` (`REQ-store`): `load()` after `publish(s)` returns `s`.
- `INV-schema-guard`: a missing file is `Ok(None)`; malformed JSON, an
  unknown `schema_version`, or invalid identifiers fail with a typed error; an
  unknown failure category loads as `unrecognized`.
- `INV-publication`: publication is atomic (`AXIOM-rename`), leaves mode
  `0600` and no temporary file, takes the lock, and refuses an older
  `evaluated_at` with `superseded`.
- `INV-cursor-confinement` (`GAP-cursor`): an activity continuation may change
  only a non-empty `after`; adding `before`, changing a fixed parameter, or
  changing the path is `invalid_continuation`. Numbered profiles still allow
  only `page`.
- `INV-early-stop` (`GAP-volume`): with a cutoff, the traversal returns every
  record at or after the cutoff, stops after the first page whose oldest record
  precedes it, and fails with `invalid_response` on any out-of-order record.
- `INV-canonical-path` (`GAP-per-repo-path`): every evidence request path is
  `/repositories/{decimal id}/{branches|pulls|activity}`.
- `INV-branch-name` (`GAP-branch-name`): `BranchName::new(n)` accepts exactly
  when `git check-ref-format refs/heads/<n>` accepts.

### Axioms (trusted, not verified here)

- `AXIOM-std-sort`: `slice::sort_by` returns a permutation sorted with respect
  to a comparator that is a total order (Rust standard library documentation).
  Boundary: `select_active`.
- `AXIOM-derived-ord`: derived `Ord` on tuples, `bool`, `Option`, and
  `core::cmp::Reverse` is lexicographic, with `false < true` and
  `None < Some(_)`; `str`'s `Ord` is lexicographic over bytes and total; and
  `BranchName` derives `Ord` and `Eq` from its `String` (Rust reference and
  standard library). Boundary: `RankKey`, pinned as exactly
  `(bool, Option<u64>, Reverse<BranchName>)` compared in descending order, with
  no other component.
- `AXIOM-branches-shape`: `GET /repositories/{id}/branches?per_page=100`
  returns a bare array of objects with `name` and `commit.sha`, paginated by
  `page` (GitHub REST documentation; probe P1). Boundary: `BranchWire`.
- `AXIOM-pulls-shape`: `GET /repositories/{id}/pulls?state=open&per_page=100`
  returns a bare array whose items carry `head.ref`, `head.repo` (object with
  numeric `id`, or `null`), and `updated_at`, paginated by `page` (GitHub REST
  documentation; probe P2). Boundary: `PullRequestWire`.
- `AXIOM-activity-shape`: `GET /repositories/{id}/activity` returns a bare
  array of records with string `ref`, string `activity_type`, string `after`,
  and RFC 3339 `timestamp`, sorted newest first, paginated by an opaque `after`
  cursor in a `rel="next"` Link, and filtered by `time_period` (probes P3 to
  P5). The ordering half is checked at run time by `INV-early-stop`. Boundary:
  `ActivityWire`.
- `AXIOM-activity-freshness`: activity data can lag by a few hours because of
  caching (GitHub REST documentation). Boundary: the stale-head rule in D4.
- `AXIOM-activity-period`: `day`, `week`, `month`, `quarter`, and `year` cover
  at least 1, 7, 28, 89, and 364 days. GitHub documents `day` and `week`; the
  rest are conservative assumptions that probe P7 must confirm. Boundary:
  `covering_period`, which picks the next larger period than needed.
- `AXIOM-pushed-at`: a repository's `pushed_at` is at or after the newest
  push, force push, merge, or branch creation on any branch, allowing for up to
  one day of cache lag (probe P9). Boundary: the activity skip in D14.
- `AXIOM-open-issues-count`: `open_issues_count` counts open pull requests
  together with open issues, so zero implies no open pull requests (GitHub REST
  documentation). Boundary: the pull request skip in D14.
- `AXIOM-canonical-path`: `/repositories/{id}/...` serves the same listing as
  `/repos/{owner}/{repo}/...` (probe P1).
- `AXIOM-rate-limit-probe`: `GET /rate_limit` does not count against the
  primary limit and carries a `Date` header (GitHub REST documentation; probe
  P8).
- `AXIOM-rename`: POSIX `rename(2)` within one directory atomically replaces
  the target. `AXIOM-flock`: `File::lock` provides an exclusive advisory lock
  between cooperating processes on Linux.

### Methods per obligation

- Obligation: `L-rank-order`, `L-prefix`, `INV-bounded`, `INV-admissible`,
  `INV-default`, `INV-lru`, `INV-cap-monotone`, and
  `INV-order-independent-model`.
  - Method: Verus proof over the candidate model, with `wf` as a `requires`
    clause on every lemma and every wrapper lemma that calls one.
  - Rationale: the properties are quantified over all sequence lengths and
    orderings; `L-prefix` is the substantive counting argument that justifies
    the production "sort then take" shape for every input.
  - Domain: all finite sequences satisfying `wf`; all caps of at least 1.
  - Artefact: `verus/active_branches.rs` (root: spec types, `wf`,
    `outranks`, `selected`, top-level lemmas, the witness, and `mod`
    declarations), `verus/active_branches_rank.rs`,
    `verus/active_branches_prefix.rs`, and
    `verus/active_branches_selection.rs`. Each spec item carries a
    `/// Models: <production item> { <fields> }` annotation.
  - Evidence: `make verus` exits 0 with zero errors; before the proofs are
    complete it fails with the open goals.
  - Non-vacuity: `proof fn witness()` builds a concrete four-candidate
    sequence (one pinned and least recent, three admissible with distinct
    recencies, cap 2), asserts `wf`, calls every top-level lemma on it, and
    proves the selection is exactly the pinned branch and the most recent
    other branch. Negative controls, each run and reverted with transcripts
    recorded: reversing the recency comparison in `outranks` must make the
    field-level `INV-lru` and the witness fail; removing distinctness from
    `wf` must make `L-rank-order` totality fail. Trust audit, which must
    print nothing (comment lines are excluded before matching):

    ```sh
    grep -vhE '^\s*//' verus/*.rs \
      | grep -nE 'assume|admit|external_body|external_fn|axiom|verifier::external|verifier\(external'
    ```

- Obligation: refinement of the model by the production kernel
  `repovec_core::active_branches::select_active` (crate-private).
  - Method: the kernel's input is a `CandidateSet` whose only constructor
    builds candidates from evaluated evidence, so `wf` holds by construction
    (distinct names after deduplication; `pinned` computed from the single
    default branch; `admissible` set whenever `pinned`). Bridge tests: an
    exhaustive `rstest` table over every pair drawn from `{pinned, unpinned}
    x {None, Some(0), Some(1), Some(u64::MAX)} x {"a", "b"}` asserting that
    `RankKey` ordering agrees with a direct Rust transcription of `outranks`;
    and a `proptest` property over generated candidate sets asserting that
    the kernel's output equals the brute-force `selected` predicate (counting
    outranking admissible candidates, no sorting), plus the field-level
    `INV-lru`.
  - Rationale: with `AXIOM-std-sort` and `AXIOM-derived-ord`, `L-prefix`
    reduces the remaining obligation to "`RankKey` encodes `outranks`", which
    the pair table decides for every equivalence class of the comparison.
  - Artefact: `crates/repovec-core/src/active_branches/kernel_bridge_tests.rs`
    and `crates/repovec-core/src/active_branches/kernel_properties.rs`.
  - Non-vacuity: a deterministic proptest runner
    (`TestRunner::new_with_rng` with `TestRng::deterministic_rng`) classifies
    cases (eviction occurred, pinned present and least recent with
    `c < |admissible|`, pinned absent, recency tie, `None` recency, cap equal
    to the admissible count) and a companion test asserts each class occurs.
    Negative controls: sorting ascending, or dropping the pinned component
    from `RankKey`, must fail; record the shrunk counterexamples.
- Obligation: `INV-evidence`, `INV-order-independent`, and
  `INV-window-period` (the evidence-to-candidate mapping, which is where
  defects are most likely).
  - Method: `rstest` tables for the finite partitions (each activity type,
    tag refs, deleted branches, inclusive and exclusive window boundaries,
    `now < w`, `now == 0`, `t > now`, `t == u64::MAX`, stale head, fork and
    `null` head repositories, `p` on and off, skip conditions); a `proptest`
    property over generated observations `(R, B, A, P, policy, now)`,
    including duplicate names, tag refs, deletions, out-of-window records, and
    fork pull requests, comparing `evaluate_repository` with a reference
    evaluator written directly from `Abstract model` (brute-force selection,
    no shared helpers); a permutation property for `INV-order-independent`;
    and an exhaustive loop over windows 1 to 364 for `INV-window-period`.
  - Artefact: `crates/repovec-core/src/active_branches/evaluate_tests.rs`,
    `crates/repovec-core/src/active_branches/evaluation_properties.rs`, and
    `crates/repovecd/src/github_branch_evidence/period_tests.rs`.
  - Non-vacuity: classification as above, including at least one generated
    case per hold reason. Negative control: counting `branch_deletion` as
    qualifying must fail.
- Obligation: `INV-branch-name`.
  - Method: `rstest` table of each refname rule and a `proptest` differential
    against `git check-ref-format refs/heads/<name>` as the oracle. Git is a
    hard prerequisite of the test suite (it is present wherever the repository
    is checked out); the test fails, rather than skipping, if `git` is absent.
  - Artefact: `crates/repovec-core/src/git_ref/tests.rs` and
    `crates/repovec-core/tests/branch_name_git_oracle.rs`.
  - Non-vacuity: the generator mixes valid names with each forbidden
    construct (`..`, leading `.` in a component, `.lock` component suffix,
    `@{`, the name `@`, control characters, space, `~^:?*[\`, trailing `/`
    or `.`, `//`) and asserts both outcomes occur. Negative control: removing
    the `.lock` rule must fail.
- Obligation: `INV-scope`, `INV-fail-safe`, `INV-tombstone`, and `INV-resume`.
  - Method: an exhaustive `rstest` table over every `RepositoryListingError`
    variant for `FailureScope::of`; scripted fake ports injecting each scope
    into each evidence call and into the status probe; a `proptest` over
    previous snapshots and per-repository outcome sequences (including
    run-stopping failures at every position and repositories added, removed,
    and returning) asserting the carry-forward, tombstone, shrink-flag, and
    resume rules against a reference model.
  - Artefact: `crates/repovec-core/src/active_branches/scope_tests.rs`,
    `discovery_tests.rs`, and `discovery_properties.rs`.
  - Non-vacuity: the table includes the empty-branch success case, so a
    defect mapping failure to an empty evaluated set is distinguishable.
    Negative control: mapping `forbidden` to an empty evaluated set, or
    dropping `last_known` on hold, must fail.
- Obligation: `INV-no-partial-publish`, `INV-round-trip`, `INV-schema-guard`,
  and `INV-publication`.
  - Method: `refresh_desired_state` (the core command that loads, discovers,
    and publishes) is tested with a recording fake store, so the no-publish
    assertion is made where `publish` is called; the file store is tested
    with a `proptest` round trip through a temporary `cap_std` directory, an
    `insta` JSON snapshot of a representative desired state, `rstest` cases
    for the guards and permission bits, a lock test using two store handles
    with deterministic ordering, and a superseded-publication test.
  - Artefact: `crates/repovec-core/src/active_branches/refresh_tests.rs`,
    `crates/repovecd/src/desired_state_store/tests.rs` and its snapshots.
  - Non-vacuity: the round-trip generator includes every state tag, empty
    active sets, missing `last_known`, and `resume_after`.
- Obligation: `INV-canonical-path`, `INV-cursor-confinement`, and
  `INV-early-stop`.
  - Method: protocol tests against `RecordingServer` asserting each request
    path and query; `rstest` tables of hostile continuations (`before`
    added, empty `after`, `time_period` changed, path changed to `/repos/...`,
    repeated `after`) and of ordering faults; a `proptest` over generated
    cursor chains and timestamp sequences, reusing the 2.2.1 pagination
    property structure, asserting completeness above the cutoff, the stopping
    page, and the failure on any out-of-order record.
  - Artefact: `crates/repovecd/src/github_rest/link/tests.rs`,
    `crates/repovecd/tests/github_branch_evidence.rs`,
    `crates/repovecd/tests/github_branch_evidence_continuations.rs`, and
    `crates/repovecd/tests/activity_pagination_properties.rs`.
  - Non-vacuity: chains of one to eight pages; at least one case stops early
    and one case reaches the end without crossing the cutoff.
- Obligation: model and production stay aligned.
  - Method: a contract test reading the `verus/` files and asserting that
    each `/// Models:` annotation names a production item, and fields, that
    exist under `crates/repovec-core/src/active_branches/`.
  - Artefact: `tests/workflow_contracts/verus_model_mirror_test.py` with a
    fixture whose annotation names a missing item and must fail.

### Formal verification decision

Verus is used for the selection model (decision D11), whose guarantees are
contractual, quantified over unbounded inputs, and stated over a small pure
kernel. The proof is declarative: it proves what selection means and that the
production shape computes it, rather than re-proving a sort. Verus is not used
for the evidence mapping, discovery, the GitHub adapter, the store, or the
command line: those promises concern real HTTP, filesystem, and process
behaviour or finite rule tables, which protocol, property, snapshot, and
end-to-end tests exercise directly. Kani is not used (D11). `make verus` stays
out of `make test`, `make lint`, `make all`, and the pull request gate,
following the estate on-ramp guidance; it is an explicit acceptance command for
`EP-M1`, `EP-M2`, and `EP-M6`, and a scheduled CI job is follow-up work.

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

  Scenario: A branch that moved after the cached activity counts as active
    Given a repository whose default branch is "main"
    And the branches "main" and "hotfix"
    And the newest activity for "hotfix" was 20 days ago at a different head
    When the policy is evaluated with a 14 day window and a cap of 5
    Then the active branches are "main" and "hotfix"

  Scenario: A default branch missing from the branch list holds the repository
    Given a repository whose default branch is "main"
    And the branches "develop"
    When the policy is evaluated with a 14 day window and a cap of 5
    Then the repository is held with "inconsistent_evidence"
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

  Scenario: A forbidden repository keeps its last known branches
    Given a stored snapshot where "octo-org/secret" has active branch "main"
    And listing branches of "octo-org/secret" is forbidden
    When active branches are discovered
    Then "octo-org/secret" is held with "forbidden" and keeps "main"

  Scenario: Rate limiting publishes the repositories already evaluated
    Given a user credential that sees three repositories
    And the activity listing of the second repository is rate limited
    When active branches are discovered
    Then the first repository is evaluated
    And the other two are held as "not_reached" with their previous state
    And the next run starts with the second repository

  Scenario: A repository missing from the listing becomes a tombstone
    Given a stored snapshot that includes "octo-org/old-tool"
    And the listing no longer includes "octo-org/old-tool"
    When active branches are discovered
    Then "octo-org/old-tool" is absent and keeps its last known branches

  Scenario: Rejected authentication publishes nothing
    Given a stored snapshot from an earlier run
    And the credential is rejected
    When active branches are discovered
    Then the command exits with the discovery failure status
    And the stored snapshot is unchanged
```

Both runners exist (`rstest-bdd` 0.5); each feature is added red, with its step
definitions, before the implementation that makes it pass.

## Plan of work

Each milestone follows Red-Green-Refactor: add the smallest failing test or
proof obligation, observe the expected failure, make it pass with the smallest
change, then refactor with focused tests and the full gates green. Commit at
least once per green step; every commit passes `make check-fmt`, `make lint`,
and `make test`. At each milestone boundary, record the conformance check from
`Milestones and plateaus` in `Progress`.

### `EP-M1` Verus on-ramp and proved selection model

Assigned: `GAP-prover`, D11, `L-rank-order`, `L-prefix`, and the selection
`INV-*`.

1. Add `tools/verus/VERSION` (`0.2026.11.02.8bf68ee`), `tools/verus/SHA256SUMS`
   (SHA-256 of `verus-0.2026.11.02.8bf68ee-x86-linux.zip`, computed from the
   downloaded asset and cross-checked against the release page),
   `tools/verus/README.md`, and `tools/rust-prover-tools/REF`
   (`ref: 6be9e71bc3ce98e96087be74b4cc572ca3488cb0`, or the then-current `main`
   commit, recorded in `Decision log`). Add `.verus/` to `.gitignore`.
2. Red: add `tests/workflow_contracts/verus_make_test.py` asserting the pin
   files' shape, that `make install-verus` and `make verus` delegate through
   `$(PROVER_TOOLS)` with no `curl`, `unzip`, `sha256sum`, or
   `rustup toolchain install`, and, through the
   `PROVER_TOOLS_CONTRACT_TEST=1 PROVER_TOOLS=<fake>` command-line seam, the
   exact
   `prover-tools verus run --repo-root . --proof-file verus/active_branches.rs`
   invocation. Run `make test-workflow-contracts`; it fails because the targets
   are missing.
3. Green: add the `install-verus` and `verus` targets as in the Verus on-ramp
   (pin-derived `PROVER_TOOLS`, guarded `REF`), outside `make test`,
   `make lint`, and `make all`.
4. Red: write the four `verus/` files with the spec types, `wf`, `outranks`,
   `selected`, every lemma statement, and the witness, each lemma with an empty
   body. Run `make install-verus` then `make verus` and record the failing
   goals. Never add `assume` or `admit`.
5. Green: prove `L-rank-order`, then `INV-admissible`, `INV-default`,
   `INV-lru`, `INV-cap-monotone`, `INV-order-independent-model`, `INV-bounded`,
   and finally `L-prefix`, decomposing when the solver struggles and scoping
   helpers with `assert(...) by { ... }`.
6. Run both negative controls and the trust audit; record transcripts in
   `Artefacts and notes`; revert the controls.

Acceptance: `make verus` verifies every function with zero errors; the
negative-control and trust-audit transcripts are recorded;
`make test-workflow-contracts`, `make check-fmt`, `make lint`, `make test`, and
`make markdownlint` pass.

### `EP-M2` Core vocabulary, kernel, and repository evaluation

Assigned: `REQ-default`, `REQ-recent`, `REQ-pr`, `REQ-cap`, `REQ-configurable`,
`GAP-branch-name`, D4, D5, D12, D14 (summary fields).

1. Red: add `crates/repovec-core/src/git_ref/` with `tests.rs` tables and
   `crates/repovec-core/tests/branch_name_git_oracle.rs`; declare
   `pub mod git_ref;` in `lib.rs`; `cargo test -p repovec-core git_ref` fails
   to compile.
2. Green: `git_ref/mod.rs` with `BranchName`, `BranchNameError`, and
   `CommitId` (40 or 64 lowercase hexadecimal characters). `BranchName` does
   not implement `AsRef<Path>`.
3. Red then green: add `pushed_at: Option<UnixSeconds>` and
   `open_issues_count: Option<u64>` to `RepositorySummary` (additive builder
   `with_activity_hints`), decode them in
   `crates/repovecd/src/github_repository_client/wire.rs`, and extend the wire
   builders in `repovec-test-helpers`. `UnixSeconds` lives in
   `repository_listing/instant.rs`; timestamp parsing stays in `repovecd`.
4. Red then green in `crates/repovec-core/src/active_branches/`:
   `window.rs` (`RecentPushWindow`, `in_window`), `policy.rs` (`BranchCap`,
   `ActiveBranchPolicy::new`, `defaults`), `evidence.rs` (`BranchHead`,
   `OpenPullRequestHead`, `RefActivity`, `RefActivityKind`,
   `RepositoryEvidence`), `kernel.rs` (`CandidateSet`, `RankKey`,
   `select_active`, `Selection`; crate-private), and `evaluate.rs`
   (`evaluate_repository`, `RepositoryEvaluation`, `EvaluatedSet`,
   `ActiveBranch`, `ActivationReasons`, `HoldReason`), each with `rstest` tests.
5. Add the kernel bridge tests, the kernel and evaluation properties, the
   policy BDD feature with its runner, and
   `tests/workflow_contracts/verus_model_mirror_test.py` with its fixture.
6. Refactor: keep each file under 400 lines.

Acceptance: `cargo test -p repovec-core git_ref`,
`cargo test -p repovec-core active_branches`,
`cargo test -p repovec-core --test active_branch_policy_bdd`,
`cargo test -p repovec-core --test branch_name_git_oracle`, every existing
`repository_listing` test, `make verus`, `make test-workflow-contracts`, and
the three standard gates pass.

### `EP-M3` Traversal amendments and GitHub evidence adapter

Assigned: `REQ-list`, `GAP-per-repo-path`, `GAP-cursor`, `GAP-volume`, D2, D3,
D6, D10.

1. Red: extend `crates/repovecd/src/github_rest/link/tests.rs` with cursor
   confinement cases (`after` changes accepted for a cursor profile; empty
   `after` rejected; `page` rejected for it; `before` rejected for both kinds),
   and add traversal tests for the ordered cutoff and the status probe. Observe
   failures.
2. Green: in `profile.rs`, add `continuation: ContinuationParameter` (`Page`
   or `Cursor(&'static str)`) and an optional ordering declaration; thread the
   parameter into `ensure_query`; add the cutoff to `TraversalRunner` and the
   status probe beside it, sharing `status` and `body`. Update the two
   repository profiles and `test_support.rs` together.
3. Red: protocol tests in `crates/repovecd/tests/github_branch_evidence.rs`
   (request path and query for each listing, page order, empty listing, `null`
   head repository, tag refs and unknown activity types ignored, invalid branch
   name or timestamp is `invalid_response`, skipped traversals send no request),
   `github_branch_evidence_continuations.rs`, and
   `activity_pagination_properties.rs`; extend `repovec_test_helpers` with
   branch, pull request, and activity builders.
4. Green: `crates/repovecd/src/github_branch_evidence/` with `wire.rs`
   (`BranchWire`, `PullRequestWire`, `ActivityWire`), `period.rs`
   (`covering_period`), the profiles (`branches?per_page=100`,
   `pulls?state=open&per_page=100`, and five static activity profiles
   `activity?time_period=<period>&per_page=100`), `EvidenceLimits`, and
   `impl BranchEvidenceSource for GitHubRepositoryClient` and
   `impl GitHubServiceStatus for GitHubRepositoryClient`, sharing the client's
   credential, endpoint, and HTTP client. Emit `tracing` events with the
   numeric repository identifier, listing kind, pages, records, bytes, whether
   the cutoff ended the traversal, and elapsed milliseconds, or the failure
   category.
5. Write ADR 0003 (amends ADR 0002) and add the amendment banner to ADR 0002.

Acceptance: `cargo test -p repovecd --lib github_rest`,
`cargo test -p repovecd --test github_branch_evidence`,
`--test github_branch_evidence_continuations`,
`--test activity_pagination_properties`, every existing
`github_repository_client*` and `repository_listing*` test, and the three
standard gates pass.

### `EP-M4` Discovery, carry-forward, and the snapshot store

Assigned: `REQ-store`, `REQ-fail-safe`, `GAP-store`, D7, D8, D14, D16.

1. Red: `scope_tests.rs`, `discovery_tests.rs`, `discovery_properties.rs`,
   and `refresh_tests.rs` against scripted fakes; observe compile failure.
2. Green: `scope.rs` (`FailureScope::of`), `desired_state.rs`
   (`DesiredState`, `RepositoryDesiredState`, `RepositoryState`,
   `EvidenceFailure`), `discovery.rs` (`discover_desired_state`,
   `DiscoveryControl`, `DiscoveryError`), `store.rs` (`DesiredStateStore`,
   `DesiredStateStoreError`), and `refresh.rs` (`refresh_desired_state`).
3. Refactor commit (separate): move `write_atomically` from
   `github_token_store/atomic_write.rs` to `crates/repovecd/src/atomic_file.rs`
   as `pub(crate)`, with its tests; the token-store tests pass unchanged.
4. Red then green: raise the workspace `rust-version` to `1.89`; add
   `crates/repovecd/src/desired_state_store/` with `wire.rs` (schema version 1
   records) and `mod.rs`
   (`FileDesiredStateStore::new(dir: cap_std::fs_utf8::Dir, file_name: &str)`,
   the lock, and the monotonic check), with its tests and snapshot.
5. Add `RuntimePaths::active_branches_snapshot()` with a doctest.
6. Write ADR 0004 with the normative schema example.

Acceptance: `cargo test -p repovec-core active_branches`,
`cargo test -p repovecd --lib desired_state_store`,
`cargo test -p repovecd --lib github_token_store`, and the three standard gates
pass.

### `EP-M5` One-shot composition, end-to-end, and behavioural evidence

Assigned: all requirements end to end, D1, D9, D13, D17, D18.

1. Refactor commit (separate): move `ListingMode`, `UserTokenLoader`, and
   credential acquisition into
   `crates/repovecd/src/github_credential_source.rs`;
   `repository_listing_command` uses it; its tests pass unchanged.
2. Red: `crates/repovecd/tests/active_branch_e2e.rs` drives the composition
   in-process with `RecordingServer` and a temporary state directory, covering
   success, a held repository, a rate-limited run with resume, a tombstone, a
   run-invalidating failure leaving the file byte-identical, clock skew, and a
   superseded publication; add the discovery feature and its runner, and
   `crates/repovecd/tests/active_branch_events.rs` for telemetry.
3. Green: `crates/repovecd/src/active_branch_command/` (`mod.rs` with
   `EvaluationRun`, `run_evaluation`, and `EvaluationStatus`; `cli/mod.rs` with
   `--user-store`, `--installation-token-stdin`, a required `--state-file`,
   `--recent-days`, `--max-branches`, and `--without-pull-requests`; and
   `output.rs`). Exit statuses: 0 published (held repositories are reported on
   standard error), 2 usage, 3 credential failure, 6 diagnostic write failure,
   7 run invalidated (nothing published), 8 store failure (including
   `superseded`), 5 output failure after publication. Read the wall clock once
   in `cli::run` and pass `UnixSeconds` inward.
4. Add `crates/repovecd/examples/evaluate_active_branches.rs`, which supplies
   process streams, the encrypted token loader, and the event subscriber, and
   nothing else.
5. Telemetry tests prove one non-zero completion event per run with the D17
   fields, and that no token, repository name, or branch name appears in
   captured logs.

Acceptance: `cargo test -p repovecd --test active_branch_e2e`,
`--test active_branch_discovery_bdd`, `--test active_branch_events`,
`--lib active_branch_command`, `--lib repository_listing_command`,
`cargo build -p repovecd --example evaluate_active_branches`, and the three
standard gates pass.

### `EP-M6` Documentation closure and roadmap completion

Assigned: documentation for D2 to D19.

1. Technical design: rewrite "Active branch policy" with the semantics,
   defaults, evidence sources, request economy, and the proof obligations that
   shaped the kernel; extend "Discovery and continuous monitoring" with failure
   scopes, carry-forward, tombstones, resumption, the clock check, and the
   snapshot contract, linking ADRs 0003 and 0004; add the 2.3.2 path-mapping
   obligation under "Worktrees and checkout layout".
2. Users' guide: section "Active branch evaluation" covering the command,
   flags, defaults, required token permission (Contents read), output, exit
   statuses, the snapshot location, ownership, and format, and the meaning of
   held, not reached, and absent; known limitation that a renamed branch loses
   its activity history.
3. Developers' guide: section 10 covering the ports, the kernel and its reuse
   policy (`evaluate_repository` is the only implementation of branch
   selection; 2.2.3, 2.4, and 5.3 call it), the `atomic_file` and
   `github_credential_source` reuse policies, the Verus proof inventory (what
   each file proves and does not, and the bridge), `make install-verus` and
   `make verus`, the wall-clock carve-out, and the focused test commands;
   update sections 9.2 and 9.3 for continuation parameters, the cutoff, and the
   status probe.
4. `docs/contents.md`: add ADRs 0003 and 0004, this ExecPlan, and
   `tools/verus/README.md`.
5. `docs/roadmap.md`: mark 2.2.2 done with a status paragraph, only after
   every acceptance criterion passes.
6. Run `make fmt`, `make markdownlint`, `make nixie`, the three standard
   gates, `make test-workflow-contracts`, and `make verus`; reconcile
   discoveries with the technical design and ADRs before setting `COMPLETE`.

## Milestones and plateaus

- `EP-M1`: proof tooling and a proved model exist; no production code depends
  on them. Requirements advanced: `GAP-prover` and the selection invariants.
  Acceptance: `make verus` with transcripts; workflow contract tests.
  Conformance: design unchanged; no runtime dependency; tooling pins only.
  Recovery: revert the milestone commits. Remaining: the production bridge.
  Compatibility: none.
- `EP-M2`: the core can evaluate one repository's evidence under a policy.
  Requirements: `REQ-default`, `REQ-recent`, `REQ-pr`, `REQ-cap`,
  `REQ-configurable`, `GAP-branch-name`. Acceptance: core unit, bridge,
  property, oracle, and BDD tests; `make verus`; workflow contracts.
  Conformance: D4 and D5 as approved; `RepositorySummary` changes are additive;
  no new runtime dependency. Recovery: revert; 2.2.1 tests prove the listing
  still works. Remaining: real evidence. Compatibility: none.
- `EP-M3`: `repovecd` can list branches, open pull request heads, and branch
  activity for one repository, and probe GitHub's clock and quota. Requirements:
  `REQ-list`, `GAP-per-repo-path`, `GAP-cursor`, `GAP-volume`. Acceptance:
  protocol, continuation, cutoff, and property tests; all 2.2.1 tests.
  Conformance: one traversal kept; ADR 0003 records the amendments; `time`
  added as authorized. Recovery: revert; changes are local to `github_rest` and
  the new adapter. Remaining: orchestration and storage. Compatibility: none.
- `EP-M4`: discovery produces, carries forward, and stores a desired state.
  Requirements: `REQ-store`, `REQ-fail-safe`, `GAP-store`. Acceptance: scope,
  discovery, and refresh tests; store round trip, snapshot, guards, lock.
  Conformance: ADR 0004 records the persisted format; no change to the
  directory contract; `rust-version` raised as authorized. Recovery: revert;
  the atomic-file move is its own commit. Remaining: operator composition.
  Compatibility: new persisted format, versioned from its first release.
- `EP-M5`: an operator can run the command end to end. Requirements: all.
  Acceptance: end-to-end, BDD, and telemetry tests; example builds.
  Conformance: no environment reads; one wall-clock read at the composition
  root. Recovery: revert; the library use case remains. Remaining:
  documentation. Compatibility: none.
- `EP-M6`: documentation and roadmap reflect the implementation. Acceptance:
  documentation gates plus every earlier command. Conformance: upstream
  reconciliation complete before `COMPLETE`.

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
EP-M2 red:   cargo test -p repovec-core git_ref -> unresolved module
EP-M2 green: cargo test -p repovec-core active_branches -> all pass
EP-M3 red:   cargo test -p repovecd --test github_branch_evidence -> unresolved items
EP-M3 green: same command -> all pass; existing listing tests still pass
EP-M4 red:   cargo test -p repovec-core discovery -> unresolved items
EP-M4 green: cargo test -p repovec-core active_branches; cargo test -p repovecd --lib desired_state_store
EP-M5 red:   cargo test -p repovecd --test active_branch_e2e -> unresolved items
EP-M5 green: same command, --test active_branch_discovery_bdd, --test active_branch_events -> all pass
```

Probes to run before relying on the matching axiom, with the operator's own
read-only credential through `gh api` and the results recorded in
`Artefacts and notes`:

```plaintext
P7  time_period spans: request day, week, month, quarter, year for a busy
    public repository and record the oldest timestamp returned by each (EP-M3)
P8  GET /rate_limit: confirm the Date header and that resources.core.remaining
    is unchanged by the call (EP-M3)
P9  pushed_at: compare a repository's pushed_at with its newest activity
    timestamp, including after a branch_creation and a pr_merge (EP-M2)
P10 activity permissions: request the activity listing with a fine-grained
    token holding Metadata read only, and record the status (EP-M3)
```

Gate commands compile Rust through the shared admission queue; a waiting build
is not hung. Record any baseline failure on `main` separately, with evidence,
before treating it as a regression.

## Validation and acceptance

Behavioural acceptance:

- `cargo test -p repovecd --test active_branch_e2e` passes. Its success case
  scripts two repositories; afterwards the state file parses as schema version
  1, lists `main` and the recently pushed branch for the first repository,
  holds the second with `forbidden` and its previous set, has mode `0600`, and
  standard output holds exactly the two summary lines.
- The rejected-authentication case leaves a pre-existing snapshot
  byte-identical and exits with status 7; the rate-limited case exits 0, holds
  the unreached repositories as `not_reached`, and records `resume_after`.
- `make verus` reports every function verified with zero errors; the
  negative-control transcripts show the expected failures.
- Against GitHub, with an operator credential, the example prints one line
  per accessible repository and writes the snapshot; this manual check is
  recorded in `Artefacts and notes` and is not part of the offline gates.

Quality criteria:

- Tests: `make test` passes with the new unit, property, protocol,
  behavioural, snapshot, and end-to-end tests; each new test was observed to
  fail before its implementation.
- Verification: `L-rank-order`, `L-prefix`, and the selection `INV-*` are
  discharged by Verus with an empty trust audit; the bridge tests pass with
  classification evidence recorded.
- Lint and format: `make check-fmt`, `make lint`, `make markdownlint`, and
  `make nixie` pass.
- Security: no environment reads; no secrets or names in telemetry; numeric
  identifiers only in request paths; continuation confinement tightened, not
  loosened.

## Idempotence and recovery

Every step is re-runnable. Gate commands and tests are side-effect free outside
temporary directories. `make install-verus` reinstalls into `.verus/`
idempotently. Publication is atomic and locked, so interrupting the example
never leaves a torn file; re-running overwrites the snapshot only with a newer
one. Each milestone is a set of commits that can be reverted without affecting
earlier milestones; the atomic-file and credential-source moves are separate
commits so they can be reverted independently.

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

Implementation evidence (red and green transcripts, negative controls, probes
P7 to P10, and the manual run) is appended here per milestone.

## Interfaces and dependencies

In `crates/repovec-core/src/git_ref/` (new public module, depends on nothing
else in the crate):

```rust
pub struct BranchName(String); // derives Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash
impl BranchName { pub fn new(value: impl Into<String>) -> Result<Self, BranchNameError>; pub fn as_str(&self) -> &str; }
pub struct CommitId(String);
impl CommitId { pub fn new(value: impl Into<String>) -> Result<Self, CommitIdError>; pub fn as_str(&self) -> &str; }
```

In `crates/repovec-core/src/repository_listing/summary.rs` (additive):
`pub fn with_activity_hints(self, pushed_at: Option<UnixSeconds>,
open_issues_count: Option<u64>) -> Self`,
`pub fn pushed_at(&self) -> Option<UnixSeconds>`, and
`pub fn open_issues_count(&self) -> Option<u64>`. `UnixSeconds(u64)` is defined
in `repository_listing/instant.rs`, because the listing is the first place an
observation instant appears. Module dependencies are therefore one-way:
`active_branches` depends on `repository_listing` and `git_ref`;
`repository_listing` depends on neither; `git_ref` depends on nothing in the
crate.

In `crates/repovec-core/src/active_branches/` (new public module):

```rust
pub struct RecentPushWindow(u16); // days, 1..=364
pub struct BranchCap(core::num::NonZeroU16); // 1..=100
pub struct ActiveBranchPolicy { /* window, cap, include_open_pull_requests */ }
impl ActiveBranchPolicy {
    pub const fn new(window: RecentPushWindow, cap: BranchCap, include_open_pull_requests: bool) -> Self;
    pub fn defaults() -> Self; // 14 days, 10, true
}

pub trait BranchEvidenceSource {
    fn list_branches(&self, repository: RepositoryId) -> Result<Vec<BranchHead>, RepositoryListingError>;
    fn list_open_pull_request_heads(&self, repository: RepositoryId)
        -> Result<Vec<OpenPullRequestHead>, RepositoryListingError>;
    /// Returns at least every branch activity record at or after `since`.
    fn list_branch_activity(&self, repository: RepositoryId, since: UnixSeconds)
        -> Result<Vec<RefActivity>, RepositoryListingError>;
}

pub trait GitHubServiceStatus {
    fn service_status(&self) -> Result<ServiceStatus, RepositoryListingError>; // server time, core quota
}

pub fn evaluate_repository(
    repository: &RepositorySummary,
    evidence: &RepositoryEvidence,
    policy: &ActiveBranchPolicy,
    now: UnixSeconds,
) -> RepositoryEvaluation;

pub enum FailureScope { RunInvalidating, RunStopping, RepositoryScoped }
impl FailureScope { pub fn of(error: &RepositoryListingError) -> Self; }

pub struct DiscoveryControl<'a> { pub should_stop: &'a dyn Fn() -> bool }

pub fn discover_desired_state(
    ports: &DiscoveryPorts<'_>, // listing, evidence, status
    previous: Option<&DesiredState>,
    policy: &ActiveBranchPolicy,
    now: UnixSeconds,
    control: &DiscoveryControl<'_>,
) -> Result<DesiredState, DiscoveryError>;

pub trait DesiredStateStore {
    fn load(&self) -> Result<Option<DesiredState>, DesiredStateStoreError>;
    fn publish(&self, state: &DesiredState) -> Result<(), DesiredStateStoreError>;
}

pub fn refresh_desired_state(
    ports: &DiscoveryPorts<'_>,
    store: &dyn DesiredStateStore,
    policy: &ActiveBranchPolicy,
    now: UnixSeconds,
    control: &DiscoveryControl<'_>,
) -> Result<RefreshReport, RefreshError>;
```

`DesiredState` holds `evaluated_at`, the policy, `listing_shrink_suspected`,
`resume_after: Option<RepositoryId>`, and entries ordered by `RepositoryId`.
Each entry holds owner, name, and
`RepositoryState::{Evaluated(EvaluatedSet),
Held { failure: EvidenceFailure, since: UnixSeconds, last_known:
Option<EvaluatedSet> }, Absent { since: UnixSeconds, last_known:
Option<EvaluatedSet> }}`.
`EvaluatedSet` holds `observed_at`, active branches with their
`ActivationReasons`, and evicted names. `EvidenceFailure` is derived from
`RepositoryListingError` by an exhaustive `TryFrom` for repository-scoped
errors, plus `InconsistentEvidence`, `NotReached(StopReason)`, and
`Unrecognized`. `DiscoveryError` distinguishes listing failure, rejected
authentication, invalid input, and clock skew, each with a stable category.
`RefreshReport` carries the counts for D17.

`discover_desired_state` is a query over ports; `refresh_desired_state` is the
command that loads, discovers, and publishes. Both perform no I/O themselves,
so they belong in the core as application services. Ownership and reuse policy:
`evaluate_repository` and the crate-private `select_active` are the only
implementation of branch selection; the reconciler (2.2.3), webhook handling
(2.4), and the TUI (5.3) call them and must not re-implement policy.

In `crates/repovec-core/src/lib.rs`:
`RuntimePaths::active_branches_snapshot(&self) -> Utf8PathBuf`.

In `crates/repovecd/src/github_rest/` (crate-private):
`ContinuationParameter::{Page, Cursor(&'static str)}` and an optional
newest-first ordering declaration as fields of `EndpointProfile<T>`; an
optional cutoff on `TraversalRunner`; and a single-request status probe.

In `crates/repovecd`: `github_branch_evidence` (private module implementing
`BranchEvidenceSource` and `GitHubServiceStatus` for `GitHubRepositoryClient`,
with `pub struct EvidenceLimits`), `atomic_file`
(`pub(crate) fn write_atomically`), `github_credential_source` (public
`ListingMode`, `UserTokenLoader`), `desired_state_store`
(`pub struct FileDesiredStateStore`), and `active_branch_command`
(`EvaluationRun`, `run_evaluation`, `EvaluationStatus`, `cli::run`).

Dependencies:
`time = { version = "0.3", default-features = false, features =
["std", "parsing"] }`
in `crates/repovecd/Cargo.toml` (runtime, D6); `googletest` and
`pretty_assertions` in `crates/repovec-core` dev-dependencies; workspace
`rust-version = "1.89"` (D16). Verus and `rust-prover-tools` are pinned under
`tools/` and are not Cargo dependencies.

## Revision note

- 2026-11-22: initial draft from Wyvern reconnaissance and live GitHub probes.
- 2026-11-22: applied the community-of-experts design review. Structure and
  contracts: removed the `default_branch` type change and its module cycle (D12,
  `git_ref`), dropped `EndpointPath` (D2), moved the activity period into the
  adapter behind a `since` port, made failure scope an exhaustive match with
  `invalid_input` run-invalidating (D7), wrote the snapshot schema normatively
  (D8), split the core into a discovery query and a refresh command so the
  no-publish invariant is tested where `publish` is called, and shared the
  credential source (D18). Failure modes and scale: replaced "publish nothing"
  with carry-forward, `not_reached`, resumption, and tombstones with a
  mass-shrink flag (D7, D8, D14); added the clock check (D13), the publication
  lock and monotonic check (D16), request economy via `pushed_at` and
  `open_issues_count` (D14), the early-stop cutoff (D3), and churn counts
  (D17). Alternatives: adopted pull request `updated_at` as recency, the
  stale-head rule for cached activity, the 1 to 100 cap range, and kept Verus
  and persistence against the reviewer's advice because the user's requirements
  demand them (D8, D11); the stacked-delivery option is left for the user
  (D19). Verification: replaced the insertion-sort model with a declarative
  selection and the `L-prefix` refinement lemma, added `wf`, restated `INV-lru`
  over fields, made the kernel crate-private behind `CandidateSet`, added the
  evidence-level brute-force property, the `str` ordering axiom and pinned
  `RankKey`, deterministic classification, two negative controls, and a wider
  trust audit. Budgets rose to reflect the added scope. The milestone order is
  unchanged; remaining work awaits approval.
