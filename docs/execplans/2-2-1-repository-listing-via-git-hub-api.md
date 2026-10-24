# List accessible GitHub repositories (2.2.1)

Status: DRAFT — awaiting explicit approval before implementation.

This execution plan is a living document. Keep Constraints, Tolerances, Risks,
Progress, Surprises & discoveries, Decision log, Outcomes & retrospective,
Conformance basis, and Verification plan current at every milestone.

## Purpose and big picture

Implement roadmap item 2.2.1: a caller in `repovecd` can obtain repositories
visible to a supplied authenticated user or GitHub App installation, including
personal and organization repositories, across every page of a successful
GitHub API traversal. Deliver a callable library use case and a runnable
one-shot example, following the existing
`crates/repovecd/examples/device-flow-test.rs` precedent. The example is the
operator-observable acceptance surface; periodic daemon scheduling remains
roadmap item 2.2.3.

A successful result represents all observations from one completed traversal,
not an atomic snapshot of GitHub. An unsuccessful traversal returns a typed
error and no partial list. This distinction protects future reconciliation from
treating a failed scan as evidence that repositories have disappeared.

This pull request contains the plan only. Approval authorizes the
implementation milestones below; it does not mark roadmap item 2.2.1 done.

## Constraints

Keep domain types and the listing port in `repovec-core`, with no HTTP,
filesystem, environment, or GitHub JSON types in the port. Put the HTTP
adapter, credential loading, and example composition in `repovecd`.
Dependencies point inward; adapters do not call one another. The composition
root loads the token and constructs the listing adapter. Do not move unrelated
OAuth code merely to make its existing port placement match the new boundary.

Support user and installation credentials explicitly. Do not infer credential
kind from token prefixes, OAuth scope strings, or the shape of an HTTP error.
Existing encrypted OAuth credentials remain readable without migration. Do not
persist installation credentials in the OAuth credential file.

Do not implement branch listing, active-branch policy, reconciliation,
worktrees, webhook management, token issuance, token refresh, or a TUI/CLI
product command. Those are separate roadmap tasks. HTTP errors must not
initiate login or silently fall back to public-only listing.

Preserve the shared Cargo cache, compiler admission wrappers, warning denials,
and the 400-line code-file limit. Never mutate process-global environment in
tests. Run formatting, format checking, lint, and tests sequentially, logging
through `tee` with pipeline failure propagation. Stop on full storage or
unhealthy compiler admission; do not repair shared services or kill other jobs.

## Tolerances (exception triggers)

Implementation may add the modules, tests, example, and documentation named
below, plus exports and manifest entries needed by those files. New runtime
crates, changes to stored credentials, automatic startup network activity,
additional user-facing commands, and policy filtering require an accepted plan
revision. `googletest` and `pretty_assertions` are authorized test
dependencies; reuse the existing versions of other workspace dependencies.

If GitHub's verified endpoint contract contradicts the proposed mode selection
or continuation validation, record the evidence and options, set status to
BLOCKED, and seek acceptance of the revised contract. The same rule applies if
implementation exposes substantive business logic that requires Verus rather
than the conventional verification selected below. Proof engineering details
may change without approval when the contract and assurance claim stay intact.

There is no elapsed-time limit. Do not weaken gates, add compatibility shims,
or suppress lints to reach a milestone. A failed gate keeps that milestone
open; correct the cause before committing or proceeding.

## Context and orientation

Baseline: commit `7763a99f5dff25547a972888ed26d6381c465bb9`.
`crates/repovec-core/src/github_oauth/mod.rs` owns `AccessToken`, including its
redacted debug representation. `crates/repovecd/src/github_device_flow/mod.rs`
contains OAuth orchestration and port examples. The blocking HTTP adapter in
`crates/repovecd/src/github_oauth_client/mod.rs` already uses `reqwest`,
disabled redirects, a 10-second connection timeout, and a 30-second request
timeout. `crates/repovecd/src/github_token_store/mod.rs` provides
`EncryptedGitHubTokenStore::load_token`; loaded tokens have no scope metadata.
Do not interpret that absence as lack of repository access.

`crates/repovecd/src/main.rs` currently runs startup validation only. There is
no discovery scheduler or repository registry to extend. The daemon library
exports adapters through `crates/repovecd/src/lib.rs`.
`crates/repovecd/tests/github_oauth_client.rs` supplies local HTTP-server test
precedent. `crates/repovec-core/tests/github_oauth_bdd.rs` and
`crates/repovec-core/tests/features/github_oauth.feature` show the current
`rstest-bdd` style. `repovecd` needs BDD and property-test dev-dependencies
explicitly; they are already present in the workspace dependency catalogue.

The pinned compiler is `nightly-2026-03-12`; no `.cargo` configuration exists
at baseline. Borrow-checker posture is unknown, not inferred from the dated
nightly. This synchronous, owned-result design requires no borrow workaround.
If that changes, inspect wrappers and flags and run the `rust-router` canary
before changing API ownership to accommodate a compiler assumption.

## Conformance basis

The baseline revision above applies to all repository references below. There
is no separate Terms of Reference or repository-discovery ADR at baseline. The
authoritative requirement is [roadmap 2.2.1](../roadmap.md), under “Repository
and branch discovery”. The architecture basis is
[the technical design](../repovec-appliance-technical-design.md), particularly
“Authentication: device flow” and “Discovery and continuous monitoring”.

Trace R1 (user and installation enumeration) through M1/M2 to endpoint-mode and
end-to-end tests. Trace R2 (personal and organization access) through M2/M3 to
mixed-owner fixtures and the BDD scenarios. Trace R3 (pagination) through M1/M2
to generated traversal histories and HTTP continuation tests. Trace C1 (inward
dependencies and secret confinement) through every milestone to port review,
redaction tests, and the design-document update. Trace C2 (no partial success)
through M1/M2 to failures after a successful first page.

Read these references before implementation:

- [Rust testing with rstest fixtures](../rust-testing-with-rstest-fixtures.md):
  reusable fallible fixtures and parameterized examples.
- [Rust doctest DRY guide](../rust-doctest-dry-guide.md): small executable
  public examples, `Result` propagation, and external example files when
  appropriate.
- [Dependency injection testing](../reliable-testing-in-rust-via-dependency-injection.md):
  endpoint, credential, and clock seams without global state.
- [Complexity and refactoring](../complexity-antipatterns-and-refactoring-strategies.md):
  separate response decoding, link validation, traversal, and composition.
- [Ortho-config guide](../ortho-config-users-guide.md): configuration
  conventions if a future product integration needs them; this item adds no
  config system.
- [rstest-bdd guide](../rstest-bdd-users-guide.md): observable scenarios and
  fixture wiring, not assertions about private implementation details.
- [Documentation style guide](../documentation-style-guide.md): Oxford British
  English, 80-column prose, documentation indexing, and ADR conventions.

Use the `codegraph-mcp`, `hexagonal-architecture`, and `rust-router` skills;
route verification choices through `rust-verification`, and use `verus` only if
the explicit escalation below is accepted. Use `execplans` to maintain this
document, `firecrawl-mcp` for external contract research, and
`logisphere-experts` for design review. CodeGraph was unavailable during
planning; direct source reads and the shared context pack supplied evidence.

## Behavioural contracts and abstract specification

Define the specification before choosing internal state. A principal is either
an authenticated user or one authenticated App installation. For a fixed
principal and endpoint, the external service supplies a trace of responses. A
successful response contains a finite ordered sequence of repository
observations and either one continuation or no continuation. A failed response
contains an error, not an empty successful page.

Let `P0, ..., Pn` be the accepted page sequences, in request order. The
successful result is their concatenation. Preserve order and multiplicity: if
GitHub repeats a repository ID during concurrent changes, retain that
observation. Do not invent deduplication, sorting, or last-writer conflict
policy in a listing transport. Repository IDs allow a future reconciler to
identify such observations explicitly. No assertion of unique IDs or consistent
metadata across pages is made.

Preconditions are a non-empty credential of the selected mode, a validated
endpoint, and positive resource limits. Each response is untrusted until its
status, body, and continuation are validated. Repository observations include a
positive numeric `RepositoryId`, owner login, name, privacy flag, and optional
default branch. Owner and repository names must be non-empty; they are
metadata, not filesystem paths. An absent or null default branch is valid for
an empty repository. Unknown JSON fields are tolerated; missing or wrongly
typed required fields fail the whole traversal. Do not require owner type to be
`User` or `Organization`, or drop observations based on owner type, fork,
archive status, or privacy.

The representation relationship is direct: domain records represent validated
wire records, the accumulator represents the concatenation of accepted pages,
and visited continuation identities represent requests already made. Domain
records never contain a bearer token or raw response body.

Candidate semantic obligations, stated independently of solver machinery:

- Prefix preservation: accepting a page extends the accumulated sequence by
  exactly that page, preserving every prior observation and its order.
- Terminal correspondence: success occurs only after a valid response without
  a next continuation, and returns precisely the accumulated observations.
- Failure atomicity: failure at any position exposes no successful prefix as a
  completed listing; cancellation or resource exhaustion has the same rule.
- Request confinement and progress: every continuation stays within the selected
  API origin and endpoint, repeats no visited request, and consumes one page
  from a finite request budget. A cycle or exhausted budget yields an error.

These are proof-sketch obligations, not claims that a formal proof exists. They
pressure-test the interface: a streaming iterator that exposes records before
traversal succeeds would not satisfy C2. `Result<Vec<_>, _>` can satisfy C2
only when success is constructed after terminal validation; the type alone does
not prevent an incorrect `Ok(prefix)`. A finite page budget bounds termination
even when GitHub keeps inventing fresh continuations. No implementation can
prove an atomic GitHub snapshot or discover repositories hidden from its
credential; those are explicitly excluded claims.

## Proposed interfaces and implementation design

Add `crates/repovec-core/src/repository_listing/mod.rs` and small sibling files
for model and error definitions. Define `RepositoryId`, `RepositorySummary`,
and semantic `RepositoryListingError` variants. Keep a minimal domain-owned
`RepositoryListing` trait with the operation below; adapter construction binds
credentials, endpoint, and limits, so the port exposes no URLs or HTTP pages.

```rust
pub trait RepositoryListing {
    fn list_repositories(
        &self,
    ) -> Result<Vec<RepositorySummary>, RepositoryListingError>;
}
```

Use a numeric newtype for repository identity and explicit optional default
branch; avoid introducing a generic entity framework. Errors distinguish
invalid input, authentication rejection, forbidden access, rate limiting,
transport failure, unexpected status, invalid response, invalid continuation,
cycle, and resource limit. Rate-limit errors carry optional validated retry
metadata, not arbitrary response text. Do not attach a raw `reqwest::Error`,
URL, JSON body, or credential as a printable source where it could leak secrets.

Add `crates/repovecd/src/github_repository_client/mod.rs`, with `wire.rs`,
`pagination.rs`, and `tests.rs` or smaller feature-oriented siblings. Reuse the
existing blocking `reqwest` dependency and `serde`. Introduce an explicit
credential enum in this adapter: `User(AccessToken)` and
`Installation(InstallationAccessToken)`. The latter is a redacted, non-empty
bearer wrapper, with no falsely inferred OAuth scopes. It represents an already
issued token; minting an App token and renewing it remain out of scope.

Use `https://api.github.com` in production. User mode requests
`GET /user/repos` with `visibility=all`,
`affiliation=owner,collaborator,organization_member`, `sort=full_name`,
`direction=asc`, and `per_page=100`. Do not combine `type` with `visibility` or
`affiliation`: GitHub rejects that combination. Installation mode requests
`GET /installation/repositories?per_page=100`, decoding the `repositories`
envelope; `total_count` is informational and never a termination condition. A
user response is a bare JSON array. Do not enumerate organizations separately
or use `/users/{name}/repos`, which cannot replace authenticated visibility.
Each installation scan covers exactly its supplied installation.

Send bearer authorization, `Accept: application/vnd.github+json`, a non-empty
`User-Agent` identifying repovecd, and an explicit `X-GitHub-Api-Version` of
`2026-03-10`, matching the documentation checked during planning. Keep
redirects disabled and the existing 10/30-second connection/request timeouts.
No automatic retries: surface 401, 403, 429, server errors, and transport
failures to the caller. Classify 403 as rate-limited only with rate-limit
evidence, otherwise forbidden. A 304 is an unexpected status because no
conditional cache is used.

Follow the `Link` header's unique `rel="next"`; absence means completion even
for a full page. A short or empty page with a next link must continue. Parse
multiple header values and relation token lists, including other relations
alongside `next`; malformed or ambiguous continuation is an error. A short
handwritten comma split is insufficient for quoted link parameters. Use a small
parser for the RFC 8288 Link field grammar, with quoted-string escapes and
separators recognized by parser state. Preserve all link-values before
selecting a continuation; an `anchor` parameter on a next link is an
unsupported-context error, not a link to silently ignore. For duplicate `rel`
parameters within one link-value, use only the first as RFC 8288 specifies.
Reject multiple next link-values rather than arbitrarily selecting one.

The considered `parse_link_header` 0.4.1 API returns relation-keyed maps, which
lose the duplicate-relation evidence required here. Its relative-link support
does not remove that mismatch. Prefer the scoped parser over a new dependency
plus a second ambiguity detector. Restrict it to Link grammar; use the existing
URL library for URL resolution, and test the RFC examples and adversarial
quoted parameters. If its grammar cannot be implemented clearly in small
functions, reopen the dependency decision rather than growing a generic parser.

Resolve relative references against the current request, then require the same
scheme, host, effective port, and listing path as the validated initial
request. Reject userinfo, fragments, HTTPS downgrade, off-origin links, and
endpoint changes before sending a credential. Canonicalize URL serialization
and query ordering for cycle identity; do not use raw text equality alone.
Preserve endpoint filter semantics across continuation; a next link may change
paging parameters but may not change or remove the initial
visibility/affiliation/sort filters. Compare parsed query multisets, reject
duplicate paging/filter keys, and allow only `page` to vary while retaining
`per_page=100` and the other initial query values. This is an endpoint-specific
profile, not a universal GitHub pagination engine. If live documented links
require additional paging keys, revise this profile and its tests explicitly.
Loopback HTTP is permitted only through an explicitly injected test endpoint
constructor; production construction remains fixed to GitHub HTTPS.

Use validated `ListingLimits` with defaults of 10,000 pages, 1,000,000 observed
records, 8 MiB per decoded response body, and 64 MiB of cumulative accepted
response-body bytes. Counting all body bytes conservatively bounds retained
metadata without relying on remote string lengths; document that allocator and
record overhead mean this is not a 64 MiB resident-memory guarantee. Enforce
body size while reading, including chunked responses, before JSON allocation;
bound Link header length to 16 KiB, and cap canonical continuation identities
at 16 KiB each before retaining them. Count accepted bytes and observations
before append with checked arithmetic; a page that would cross either aggregate
limit fails without append. Reaching a limit is a typed failure, never
truncation reported as success. A terminal response exactly at a limit is
valid. Process pages sequentially with one reused HTTP client; memory is
bounded by the cumulative body budget, the record budget, the current page, and
bounded visited-link storage. There is no total elapsed-time deadline in this
item: 10,000 requests at 30 seconds can take about 83 hours. Document that
bound and leave whole-scan scheduling/deadlines to the future scheduler;
per-request timeouts and finite budgets are the current termination controls.
The example exposes these defaults in its help/documentation rather than
silently claiming unlimited enumeration.

Add `crates/repovecd/examples/list_github_repositories.rs` as the composition
root, calling shared `crates/repovecd/src/repository_listing_command.rs`
composition with injected credential reader, endpoint, input, output, and error
streams. With `--user-store /etc/repovec`, load the existing encrypted token
through `EncryptedGitHubTokenStore`; with `--installation-token-stdin`, read an
issued installation token from standard input without echoing it. Read at most
16 KiB plus one sentinel byte, reject oversize, empty, invalid UTF-8, control
characters, or interior whitespace, and remove at most one trailing line
ending. User-store credentials receive the same non-empty/header-safety
validation before a request. Require exactly one mode. Never accept a literal
token argument or write installation tokens to disk. A small example-only
argument parser is sufficient. Print one `owner/name` line per observation only
after full success, using fallible output writes. Acquisition, validation, and
traversal failures print a sanitized diagnostic to stderr and exit non-zero
with no repository stdout. Output I/O failure also exits non-zero but may leave
partial stdout; output cannot be rolled back after a pipe closes. Test a writer
that fails after one line. The example is explicitly a diagnostic/library
demonstration, not a new `repovectl` command. Shared composition code must be
callable with injected credentials and endpoints for offline end-to-end tests.

Emit structured start, completion, and failure events with principal kind,
accepted page/record counts, elapsed duration, and sanitized error category. Do
not record repository names, tokens, raw URLs, or bodies by default. Test
captured events for useful counts and redaction. Diagnostics should direct user
credentials rejected with 401 to reauthentication, installation rejection to
obtaining a fresh installation token, forbidden access to access/organization
policy checks, rate limiting to validated retry metadata, and transport failure
to connectivity checks. Limit errors name the exhausted budget; the library
caller can explicitly increase validated limits. The example retains documented
fixed defaults rather than introducing additional configuration flags.

## Verification plan

The feature is principally HTTP integration with standard sequence append, not
new contractual business policy. Use conventional protocol tests and generated
traversal tests for the obligations above. No Verus implementation milestone is
selected: proving a mirror of vector append would not establish Link parsing,
credential confinement, or GitHub completeness. No custom deduplication or
branch policy is introduced to manufacture a proof target. Kani adds little
over generated histories for this I/O boundary and is not selected.

If implementation introduces custom aggregation, policy, or a stable pure
kernel requiring an unbounded guarantee, revise the plan before implementing
it. State its abstract model, premises, postconditions, representation bridge,
and major semantic lemmas first. Obtain approval of that substantive change,
then use Verus for exhaustive proofs from satisfiable premises. Record the
runtime-to-proof relationship in the design document, pin the tool and
checksums, and make the new milestone require
`prover-tools verus run --repo-root . --proof-file verus/repository_listing.rs`
as well as the ordinary gates. Neither `assume`, `admit`, external/trusted
proof bodies, nor a property equivalent to the conclusion in the premises is
acceptable. Unproved statements stay in documentation; do not create
speculative compiling proof scaffolding. Defer induction helpers and solver
tuning until actual verification conditions exist.

External assumptions are that GitHub enforces credential authorization and
reports valid continuation links under normal operation; the network and TLS
library implement their documented interfaces; Rust collections preserve
sequence operations; and operating-system I/O can fail. Tests cannot prove
these third-party internals. They must exercise repository-owned configuration
against a faithful HTTP boundary. Changing permissions or repositories during a
scan can cause omissions or repetitions even with a correct traversal.

For prefix preservation and terminal correspondence, add
`crates/repovecd/tests/repository_listing_properties.rs`. Generate 1–32
responses (empty success is one empty terminal page), 0–100 records per page,
repeated IDs, arbitrary page partitions, empty middle pages, and terminal pages
of exactly 100 records. Compare the real traversal's output with independent
flattened fixtures. Include witnesses for empty, single-page, and non-empty
multi-page results. Seed a temporary defect that stops on a short page and an
omitted-append defect; each must fail the intended property. Record non-empty,
repeated-ID, empty-middle, and exact-boundary class coverage and avoid
rejecting generated inputs. These are sampled tests over explicit finite
ranges, not exhaustive proofs.

For failure atomicity, add parameterized tests in
`crates/repovecd/tests/github_repository_client.rs` for every error category,
including errors on page one and after accepted pages. Assert `Err`, no further
requests, and no successful output. Include 401, ordinary 403, rate-limited
403/429, 404, 422, 500, timeout, truncated JSON, wrong envelope, invalid
required fields, and body/header/aggregate-byte/record/page limit boundaries,
including exact-boundary success and rejection after earlier accepted pages. A
fault that returns the collected prefix on error must be rejected by a test
with a non-empty prefix. Test optional/null default branch and unknown JSON
fields as success cases.

For confinement/progress, use local recording HTTP servers on distinct ports.
Assert exact method, path, query, bearer header, API version, and user agent.
Cases cover absolute and relative next links, multiple relations and headers,
quoted parameters, missing next, multiple next targets, repeated and two-page
cycles, reordered queries, off-origin and downgraded URLs, changed paths,
userinfo, and changed filters. The second server must observe zero requests for
a rejected continuation or redirect. Generate continuation histories that
include cycles and fresh paging query values on the same permitted path to the
page budget, preserving filters; assert a finite request count and typed
failure. A temporary bypass of origin validation must make the second-server
test fail. Use fake credentials exclusively.

Use `rstest` for the finite case tables and shared fallible setup. Add
`googletest` and `pretty_assertions` as dev-dependencies at workspace level and
in crates that use them: match structured errors with `googletest`, and compare
expected repository sequences with `pretty_assertions`. Use existing `proptest`
for generated histories. Use `rstest-bdd` for the following feature at
`crates/repovecd/tests/features/repository_listing.feature`, bound by
`crates/repovecd/tests/repository_listing_bdd.rs`:

```gherkin
Feature: Discover repositories visible to an authenticated principal
  Scenario: A user sees personal and organization repositories across pages
    Given a user credential accepted by the repository service
    And a personal repository on the first page
    And an organization repository on a later page
    When repository listing completes
    Then both repository observations are returned in page order

  Scenario Outline: An installation sees repositories belonging to its owner
    Given an installation credential for a <owner> account
    And repositories for that account on two pages
    When repository listing completes
    Then both repository observations for that account are returned
    Examples:
      | owner        |
      | personal     |
      | organization |

  Scenario: A failed later page cannot masquerade as a complete listing
    Given a successful first page containing a repository
    And the next page rejects the credential
    When repository listing is attempted
    Then authentication failure is reported without a completed listing

  Scenario: An empty page can have a continuation
    Given an empty first page with a next link
    And a repository on the final page
    When repository listing completes
    Then the final repository is returned
```

Add `crates/repovecd/tests/repository_listing_e2e.rs` to drive the real adapter
and shared command composition through loopback HTTP. Cover user-token loading
via the existing encrypted store with an injected encryptor and temporary
capability directory, installation stdin input, output-after-success, no stdout
on late traversal failure, output I/O failure, and credential redaction. These
tests call the same composition function as the example with injected endpoints
and streams, asserting its returned exit status.

Actual-example child-process tests cover invalid/mutually exclusive arguments
and empty, invalid, or oversize stdin, all rejected before networking. Build it
first with `cargo build -p repovecd --example list_github_repositories`; derive
the executable path from Cargo metadata's `target_directory` plus
`debug/examples/list_github_repositories`, respecting custom target
directories. Pass that explicit path into the subprocess fixture, without
changing process environment. For full-gate integration, extend the existing
Rust test runner only if it does not build the executable already; update its
command-contract tests in the same commit. An unresolved example path must
fail, never skip. Successful offline HTTP subprocess testing is not claimed:
loopback success runs through the shared composition function. Live GitHub
smoke runs remain opt-in and do not replace offline acceptance. No production
endpoint-override flag or environment backdoor is introduced.

There is one plain listing output format, so multiformat `insta` snapshots are
not warranted. Assert typed results and exact short output directly. If an
approved revision introduces multiple public renderings, add reviewed `insta`
snapshots for each rendering, alongside semantic assertions. Existing snapshot
redaction utilities may be reused for diagnostics if that improves clarity;
never snapshot secrets or accept snapshots blindly.

## Milestones and plateaus

### M1: Domain contract and failing protocol evidence

After explicit approval, add the core model, port, semantic errors, and their
unit tests and Rustdoc examples. Add fixture builders and the BDD
specification. Use a minimal fake implementation to test consumer success/error
handling; do not claim it proves HTTP integration. Capture an expected failing
focused adapter test before adding production HTTP behaviour in M2. Commit only
the coherent passing domain/test-support changes; do not commit an unexplained
failing suite. R1/C1/C2 are specified here, while network behaviour remains
open.

Acceptance: model validation tests, public doctests, and all ordinary gates
pass. Review dependency direction and ensure new abstractions model only
listing. A rollback reverts this commit without persisted-state changes. No
compatibility layer is needed for this pre-1.0 internal API.

### M2: Both endpoint modes and complete traversal

Implement the adapter in test-driven slices: first a single authenticated page,
then both wire shapes, then continuations and failures, then limits and
redaction. Before each slice, run the smallest relevant test and record its
expected failure; implement only enough behaviour to pass, then refactor.
Exercise the actual traversal in property tests, not a second test-only loop.
Record temporary mutation failures and remove mutations before gating.

Acceptance: R1/R2/R3/C1/C2 protocol and property evidence passes, every BDD
scenario is bound and passes, and the full gates pass. Reconcile API claims
with the verified GitHub references. No credential format or existing startup
behaviour changes. The callable library is a complete plateau even if example
publication follows later. Revert M2 to recover; future scheduler integration
remains out of scope.

### M3: Runnable composition and documentation

Implement the one-shot example, end-to-end tests, and public usage examples.
Update `docs/users-guide.md` with both modes, required visibility, installation
credential provisioning prerequisites, limits, empty success versus failure,
exit/output semantics, and the non-snapshot guarantee. Update
`docs/developers-guide.md` with the port, fixture reuse strategy, test
commands, credential seams, and proof-selection rationale. The component
architecture currently lives in `docs/repovec-appliance-technical-design.md`;
add the listing boundary, principal distinction, traversal contract, and
representation there. If a substantive accepted decision needs a separate ADR,
allocate the next unused `docs/adr-NNN-*.md` number at implementation time and
reference it from the design and `docs/contents.md`.

Acceptance: the runnable workflow and offline end-to-end evidence succeed; all
code and documentation gates pass; reviewer findings are reconciled. Only then
mark 2.2.1 `[x]` with a `Status: done` note in `docs/roadmap.md`. Do not mark
2.2.2 or 2.2.3 complete. Update plan evidence and conformance links before
setting status COMPLETE. Revert the example/docs commit if needed; no stored
state migration or live GitHub mutation is involved.

At every milestone, record requirements discharged, remaining gaps, test logs,
and a focused conformance check: dependency direction, external assumptions,
unchanged credential persistence, approved interfaces/dependencies, and current
trace links. Any substantive departure follows the tolerance procedure rather
than silently changing the approved design. Gate each atomic commit; use
separate gated commits for later refactoring and retain rollback points.

## Concrete steps and acceptance commands

Run from the repository root on the approved feature branch. Before code edits,
inspect `AGENTS.md`, branch/worktree state, the live plan, and current
dependency versions. Run a baseline `make check-fmt`, `make lint`, and
`make test` in that order. Keep the shared cache and wrappers. The actual
Makefile also checks Rustdoc, Whitaker when installed, Python test-runner
contracts, and doctests; do not replace its targets with narrower cargo
commands as full gates.

Use focused commands during red/green cycles, then delegate full sequential
gates to `scrutineer`. Representative focused commands once the named targets
exist are:

```bash
cargo test -p repovec-core repository_listing
cargo test -p repovecd --test github_repository_client
cargo test -p repovecd --test repository_listing_properties
cargo test -p repovecd --test repository_listing_bdd
cargo test -p repovecd --test repository_listing_e2e
```

An initial missing-symbol/target failure is only scaffold evidence; capture a
behavioural assertion failure once the surface compiles. Green means each named
test passes, no ignored acceptance scenario remains, and each temporary seeded
fault was caught for the intended reason. No exact test count is predicted.

After every major milestone, run the following commands sequentially with
`set -euo pipefail` in the same shell. Use a milestone and branch-specific log
suffix when recording evidence; repeat the full set after any code change that
invalidates earlier evidence.

```bash
set -euo pipefail
make fmt 2>&1 | tee /tmp/fmt-repovec-2-2-1.out
make check-fmt 2>&1 | tee /tmp/check-fmt-repovec-2-2-1.out
make lint 2>&1 | tee /tmp/lint-repovec-2-2-1.out
make test 2>&1 | tee /tmp/test-repovec-2-2-1.out
make markdownlint 2>&1 | tee /tmp/markdownlint-repovec-2-2-1.out
make nixie 2>&1 | tee /tmp/nixie-repovec-2-2-1.out
```

Stop at a failing command; do not run the next gate until the failure is
understood and corrected. Inspect formatter changes before staging. Never
hand-edit generated `typos.toml`; unrelated regeneration requires a separate
base change per repository policy. The plan-only commit runs documentation
checks and format checking, with no claim of implemented behaviour.

Observable manual user-mode acceptance after M3:

```bash
cargo run -p repovecd --example list_github_repositories -- --user-store /etc/repovec
```

With a previously provisioned credential, expect one `owner/name` per observed
repository and exit zero; no repositories means empty stdout and exit zero. For
installation mode, securely pipe an already issued installation credential to
the example with `--installation-token-stdin`; do not paste tokens into command
arguments, logs, or the plan. Acquisition or traversal failure produces stderr
and non-zero exit without repository stdout; a subsequent output-write failure
can leave partial stdout and still exits non-zero. These live commands require
operator-provided credentials and are supplemental, not prerequisites for
deterministic CI.

## Idempotence and recovery

Listing performs only HTTP GETs, with no GitHub mutation, filesystem
persistence, or cached partial result. Rerunning after a failure starts from
page one with fresh caller-supplied credentials. Resource-limit failures
require a deliberate larger validated budget or a later scoped-discovery
design, never partial success. A timeout or process interruption discards the
local accumulator.

Use ordinary revert commits for accepted implementation changes that need
rollback; do not reset another agent's worktree or rewrite an existing
divergent remote branch. Re-run relevant gates after recovery. Keep staged and
unstaged diffs scoped to the milestone and preserve existing uncommitted work.

## Risks

High impact, medium likelihood: access changes during pagination produce a
non-snapshot observation list. Document this limit and leave reconciliation
policy to its own task; do not claim lossless global enumeration under churn.

High impact, low likelihood: malicious continuation or diagnostics expose a
bearer credential. Enforce origin/path validation before sending and test a
second server plus captured stdout, stderr, `Debug`, and error chains.

Medium impact, medium likelihood: 2.1.2/2.1.3 remain incomplete and
installation token issuance does not exist. Supply explicit credential kinds,
surface errors, and document provisioning prerequisites without implementing
those workstreams.

Medium impact, medium likelihood: parser edge cases or large responses consume
excessive resources. Test protocol grammar and finite limits; fail explicitly
at boundaries. Do not add background retries or parallel page fetching.

## Progress

- [x] (2026-10-20) Read repository guidance, rename branch, and load skills.
- [x] (2026-10-20) Gather architecture and testing evidence with two Wyvern
      agents.
- [x] (2026-10-20) Check GitHub endpoint and pagination documentation with
      Firecrawl.
- [x] (2026-10-20) Complete and reconcile the six-expert design review.
- [x] (2026-10-20) Pass plan-only documentation gates. Publication is recorded
      in the draft PR metadata.
- [ ] Explicit approval of this plan.
- [ ] M1: domain contract and test support.
- [ ] M2: endpoint modes and complete traversal.
- [ ] M3: observable composition, documentation, and roadmap completion.

## Surprises & discoveries

The OAuth credential store intentionally discards scope metadata. Repository
listing cannot use its empty scope list as an authorization decision. GitHub
App installation authentication is absent, but the listing endpoint can still
support an explicitly supplied, already-issued installation token.

Firecrawl's structured extraction mixed unrelated endpoint details in one
response, including an incorrect installation-token claim for `/user/repos`.
Reading the full official endpoint section resolved the ambiguity: installation
listing uses its distinct endpoint and response envelope. Verify critical
contracts against source sections rather than trusting extraction summaries.

## Decision log

2026-10-20, planning: propose separate explicit user and installation modes,
with caller-owned token provisioning. This satisfies R1 without expanding into
an App authentication implementation. Approval of this plan accepts this scope.

2026-10-20, planning: reuse blocking `reqwest` instead of adding `octocrab`.
Only two read endpoints are required, and the existing runtime already uses
this HTTP stack. Keep all HTTP and pagination details behind the domain port.

2026-10-20, planning: preserve page observations rather than deduplicate or
invent conflict policy. This retains evidence during upstream changes and keeps
listing separate from future desired-state construction. Conventional
protocol/property tests match the actual assurance boundary; formal
verification is reserved for a separately approved substantive pure policy
kernel.

2026-10-20, expert review: Pandalump and Wafflecat requested realistic
installation scenarios, a concrete subprocess boundary, and parser prior art.
Telefono and Dinolump corrected the type-level atomicity claim, empty-success
model, and generated paging histories. Buzzy Bee and Doggylump required
aggregate resource accounting, bounded stdin, honest write-failure semantics,
observability, and actionable recovery. All are incorporated above. All six
perspectives accepted conventional verification for the scoped transport;
retaining observation multiplicity trades uniqueness for preserving evidence.

## Outcomes & retrospective

The two Wyvern reconnaissance agents and six expert perspectives completed
planning review. Findings were incorporated: aggregate memory accounting,
realistic installation scenarios, honest stdout guarantees, parser grammar and
prior-art comparison, explicit subprocess limits, useful sanitized events, and
non-vacuous property domains. No feature code, tests, proof artefacts, or
roadmap completion have been implemented. The plan awaits explicit user
approval. Delivery evidence will be recorded here after each milestone.

Plan-only validation on 2026-10-20 passed sequentially: `make fmt`,
`make check-fmt`, `make markdownlint` (including spelling), `make nixie`, and
`git diff --check`. Logs are `/tmp/<gate>-repovec-2-2-1-plan.out`, with
`diff-check` for the final Git check. Spelling regenerated no tracked changes.
No Rust runtime code changed, so `make lint` and `make test` were not run for
this documentation commit; they remain mandatory implementation milestones.

## Research references

Official documentation and parser API checked with Firecrawl on 2026-10-20:

- [Repositories for the authenticated user](https://docs.github.com/en/rest/repos/repos#list-repositories-for-the-authenticated-user):
  user visibility, affiliation, filters, and array response.
- [Repositories accessible to an installation](https://docs.github.com/en/rest/apps/installations#list-repositories-accessible-to-the-app-installation):
  installation credential scope and repository envelope.
- [REST pagination](https://docs.github.com/en/rest/using-the-rest-api/using-pagination-in-the-rest-api):
  next-link traversal and page sizing.
- [RFC 8288 Web Linking](https://www.rfc-editor.org/rfc/rfc8288.html): Link
  grammar, relation parameters, relative references, and context anchors.
- [parse_link_header API](https://docs.rs/parse_link_header/0.4.1/parse_link_header/):
  considered parser; relation-keyed output conflicts with ambiguity detection.

Revision note (2026-10-20): initial plan records contracts before
implementation, explicit credential modes, proportionate verification, and
gated delivery. Revision note (2026-10-20): incorporated all six experts'
findings. The changes strengthen resource and error contracts without expanding
lifecycle scope. Documentation validation passed; the draft PR records
publication, while implementation remains subject to approval.
