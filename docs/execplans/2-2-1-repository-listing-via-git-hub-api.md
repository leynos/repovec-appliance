# List accessible GitHub repositories (2.2.1)

Status: COMPLETE — implemented 2026-11-19; every milestone and gate passed.

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

Honour the environment access policy in `docs/developers-guide.md`, section 8.
`clippy.toml` disallows `std::env::var`, `var_os`, `vars`, `vars_os`, `set_var`,
`remove_var`, and `set_current_dir`, and the workspace denies
`clippy::disallowed_methods`. This plan reads no environment variable at all:
credentials arrive through the store or standard input, and the endpoint is
injected. Process arguments (`std::env::args_os`) remain permitted at the
example's entry point. A child process receives an explicit environment and
working directory through `Command::env_clear`, `Command::env`, and
`Command::current_dir`. The workspace also denies `print_stdout`,
`print_stderr`, `missing_docs_in_private_items`, `expect_used`, and
`allow_attributes`: write through the injected streams, document private items,
return `Result` from fixtures and helpers, and use an item-scoped `#[expect]`
with a reason if a suppression is ever unavoidable.

Use the development build standard as it stands: the LLVM backend, the pinned
`mold` linker, and the parallel `rustc` frontend from `.cargo/config.toml`.
Cranelift is out of scope. It remains an unsupported compile-time option for a
later experiment, so no dependency choice, test structure, or feature gate in
this work may be made for the sake of Cranelift compatibility beyond ordinary
Rust hygiene. Do not name a codegen backend in any profile; a workflow contract
refuses one.

## Tolerances (exception triggers)

Implementation may add the modules, tests, example, and documentation named
below, plus exports and manifest entries needed by those files. New runtime
crates, changes to stored credentials, automatic startup network activity,
additional user-facing commands, and policy filtering require an accepted plan
revision. `nom-rfc8288` is the one authorized new runtime dependency, used only
for Link field parsing. `googletest` and `pretty_assertions` are authorized
test dependencies; reuse the existing versions of other workspace dependencies.

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

Baseline: commit `df8c445b3a85b4be01da2c8b3e71f82a21e1082c`, the `main` head
this branch was rebased onto on 2026-11-18. The plan was first drafted against
`7763a99f5dff25547a972888ed26d6381c465bb9`; the orientation below was checked
again at the new baseline. `crates/repovec-core/src/github_oauth/mod.rs` owns
`AccessToken`, including its redacted debug representation.
`crates/repovecd/src/github_device_flow/mod.rs` contains OAuth orchestration
and port examples. The blocking HTTP adapter in
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

The pinned compiler is `nightly-2026-11-13`. `.cargo/config.toml` now exists:
it sets `-Zthreads=8`, links with the pinned `mold` on Linux, and builds
dependencies without embedded metadata. It names no codegen backend. Run
`make install-build-tools` once before the first local gate. `rstest` is at
0.28.0 in the workspace catalogue. Borrow-checker posture is unknown, not
inferred from the dated nightly. This synchronous, owned-result design requires
no borrow workaround. If that changes, inspect wrappers and flags and run the
`rust-router` canary before changing API ownership to accommodate a compiler
assumption.

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

At completion, the Progress entries name the test suites and gate logs that
discharge each trace, the Outcomes entry maps R1, R2, R3, C1, and C2 to that
evidence, and the design document, ADR 0002, users guide, developers guide, and
roadmap status note record the delivered state.

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
- [Developers' guide](../developers-guide.md): “Development build standard”,
  “Fetching dependencies before tests”, “Local quality gates”, and “Environment
  access policy”. These postdate the first draft and bind this work.

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

Let `page_0, ..., page_n` be the accepted page sequences, in request order. The
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

Split the adapter in two. Add `crates/repovecd/src/github_rest/mod.rs`, with
`traversal.rs`, `profile.rs`, `link.rs`, `limits.rs`, and tests in
feature-oriented siblings. It owns paginated GET traversal for any GitHub REST
listing and knows nothing about repositories. Add
`crates/repovecd/src/github_repository_client/mod.rs`, with `wire.rs` and
tests. It owns the two repository endpoint profiles, the wire records, their
validation into domain records, and the `RepositoryListing` implementation.
Reuse the existing blocking `reqwest` dependency and `serde`. Introduce an
explicit credential enum in this adapter: `User(AccessToken)` and
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

### Shared traversal and endpoint profiles

Branch listing and open pull request listing (2.2.2) page the same way as
repository listing, once per repository. Build the traversal once, generic over
the item type, and let each endpoint describe itself with a small profile:

```rust
pub(crate) struct EndpointProfile<T> {
    /// Path below the API origin, for example `/user/repos`.
    path: &'static str,
    /// Query pairs fixed for the whole traversal, including `per_page=100`.
    fixed_query: &'static [(&'static str, &'static str)],
    /// Where a page keeps its items.
    items: PageItems,
    /// Marks the wire record type decoded from each item.
    record: PhantomData<fn() -> T>,
}

pub(crate) enum PageItems {
    /// The body is a JSON array of items.
    BareArray,
    /// The body is an object holding the items under this key.
    Envelope(&'static str),
}
```

The traversal takes a client, a credential, validated limits, and a profile. It
returns every decoded wire record from one completed traversal, with the
accepted page, record, and byte counts, or a typed error and nothing else. The
exact signature may change during implementation; the three profile fields and
the all-or-nothing result may not. The repository client then validates wire
records into domain records, so an invalid record still fails the whole listing
before `Ok` is constructed.

Scope and reuse policy: `github_rest` is private to `repovecd`. It is the only
place that follows a GitHub `Link` continuation, and every GitHub REST listing
in the daemon must go through it instead of writing its own loop. A new listing
adds a profile and a wire type, not traversal code. The module owns Link
selection, continuation confinement, cycle identity, limits, and status
classification. It does not own domain types, retries, scheduling, or a
whole-scan budget across many traversals; 2.2.2 and the scheduler decide those.
This item ships two profiles, user repositories and installation repositories,
which exercise both `PageItems` variants. It is not a general pagination
abstraction: no trait is introduced, because every known consumer uses the same
protocol. Extract a trait only when a second protocol exists to shape it.

No equivalent exists to reuse. `github_oauth_client` makes single form-encoded
POSTs and follows no continuation, and no workspace dependency offers one.

### Link continuation

Follow the `Link` header's unique `rel="next"`; absence means completion even
for a full page. A short or empty page with a next link must continue. Parse
the Link field with `nom-rfc8288` 0.4.0, using
`nom_rfc8288::complete::link_strict`, and keep the crate behind `link.rs` so
nothing else names its types. The parser returns every link-value with its
target and its parameters in order, which keeps the evidence this design needs.
Do not write a Link grammar by hand.

Continuation selection stays ours and is small. Parse each `Link` header value
separately and concatenate the results, so multiple headers and one
comma-joined header behave alike. A parse error, or an empty list element, is
an invalid-continuation error. For each link-value, read only the first `rel`
parameter as RFC 8288 specifies, and split it on spaces into relation types, so
other relations alongside `next` are accepted. An `anchor` parameter on a next
link is an unsupported-context error, not a link to silently ignore. Reject
multiple next link-values instead of arbitrarily selecting one. Use the
existing URL library for URL resolution.

`parse_link_header` 0.4.1 was considered first and rejected: it returns
relation-keyed maps, which lose the duplicate-relation evidence required here.
`octocrab` 0.54.9 was also examined at source. It confines the credential by
host and classifies rate limits well, but it splits the Link field on commas,
lets the last `next` win, reads each body unbounded, and has no page, record,
or cycle limit. Adopting it would mean giving up those contracts, so the
blocking `reqwest` adapter stays. `page-turner` 1.0 abstracts only how the next
request is derived, which is identical for every GitHub listing, and it is
async-only.

Resolve relative references against the current request, then require the same
scheme, host, effective port, and listing path as the validated initial
request. Reject userinfo, fragments, HTTPS downgrade, off-origin links, and
endpoint changes before sending a credential. Canonicalize URL serialization
and query ordering for cycle identity; do not use raw text equality alone.
Preserve endpoint filter semantics across continuation; a next link may change
paging parameters but may not change or remove the initial
visibility/affiliation/sort filters. Compare parsed query multisets, reject
duplicate paging/filter keys, and allow only `page` to vary while retaining
`per_page=100` and the other initial query values. The initial values are the
profile's `fixed_query`, so the rule is stated once and holds for every
profile. If live documented links require additional paging keys, revise the
traversal and its tests explicitly; do not loosen the rule for one profile.
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

Add `crates/repovecd/examples/list_github_repositories.rs` as the process
boundary, calling the shared `crates/repovecd/src/repository_listing_command/`
composition with injected credential reader, endpoint, input, output, and error
streams. With `--user-store /etc/repovec`, load the existing encrypted token
through `EncryptedGitHubTokenStore`; with `--installation-token-stdin`, read an
issued installation token from standard input without echoing it. Read at most
16 KiB plus one sentinel byte, reject oversize, empty, invalid UTF-8, control
characters, or interior whitespace, and remove at most one trailing line
ending. User-store credentials receive the same non-empty/header-safety
validation before a request. Require exactly one mode. Never accept a literal
token argument or write installation tokens to disk. A small argument parser is
sufficient; it lives in the library beside the composition
(`repository_listing_command::cli`), so the command line is tested in-process
and the example supplies only the process boundary. Print one `owner/name` line
per observation only after full success, using fallible output writes.
Acquisition, validation, and traversal failures print a sanitized diagnostic to
stderr and exit non-zero with no repository stdout. Output I/O failure also
exits non-zero but may leave partial stdout; output cannot be rolled back after
a pipe closes. Test a writer that fails after one line. The example is
explicitly a diagnostic/library demonstration, not a new `repovectl` command.
Shared composition code must be callable with injected credentials and
endpoints for offline end-to-end tests.

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
failure. Those generated histories live in
`crates/repovecd/tests/repository_listing_pagination_properties.rs`, run over
both shipped profiles, and assert exact request counts as well as the typed
outcome. A temporary bypass of origin validation must make the second-server
test fail. Use fake credentials exclusively.

Classify a `403` by evidence rather than by the presence of any rate-limit
header: `Retry-After` or an exhausted `x-ratelimit-remaining` is a rate limit,
while a reset time alone is not, because GitHub sends `x-ratelimit-*` on
ordinary responses. A `403` with quota remaining is an access refusal.

Test Link selection at the `link.rs` boundary, not the third-party parser
itself. Feed it the RFC 8288 examples, a quoted parameter containing a comma
and an escaped quote, a relation list such as `rel="next last"`, a repeated
`rel` parameter, an `anchor` on the next link, two next link-values in one
header and across two headers, an empty list element, malformed input, and
mixed-case spellings of the parameter name and of the registered `next`
relation, which RFC 8288 section 2.1.1 requires to compare case-insensitively.
These tests pin the behaviour this design relies on, so a parser upgrade that
changes it fails here.

Exercise the shared traversal through both shipped profiles, and once through a
test-only profile with a different path, fixed query, and envelope key. That
third profile is the evidence that the traversal carries no repository
knowledge; a traversal that hard-codes `repositories` or `/user/repos` must
fail it.

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

The command-line surface is library code, so its cases are unit tests rather
than child-process tests. `repository_listing_command::cli` owns `CliRun`, the
argument parser, the usage text, and the exit-status mapping, and the unit
tests drive `cli::run` in-process over injected streams and a fake credential
loader. They cover invalid and mutually exclusive arguments, a missing argument
value, non-UTF-8 arguments, the usage text, a failed usage or help write, an
unavailable user store, and empty, invalid, or oversize standard input, all
rejected before networking.

This supersedes the revision note's child-process design, whose premises did
not hold. `make test` passes `--all-targets`, but that builds the example only
as a libtest harness whose binary ignores the example's arguments, and on a
clean tree it produces no runnable example for a test to start; hosted CI run
44612002870 failed exactly there. The fallback this plan permitted — a nested,
offline `cargo build --example` joined to the `nested-cargo-builds` group — was
implemented and also failed: on a cold tree the first test in the binary paid
for a full compile that waited behind compile admission and exceeded its
300-second allowance, against a 10-minute run ceiling. Moving the command line
into the library keeps every assertion, removes the build coupling, and leaves
the example a thin adapter that `cargo clippy --all-targets` still compiles.

The `test` recipe and `scripts/run_rust_tests.py` stay unchanged.

Successful offline HTTP subprocess testing is not claimed: loopback success
runs through the shared composition function. Live GitHub smoke runs remain
opt-in and do not replace offline acceptance. No production endpoint-override
flag or environment backdoor is introduced.

Every test must fit the nextest budget in `.config/nextest.toml`: a test is
terminated after 180 seconds, and the whole run after 10 minutes. The
production 10/30-second timeouts are therefore injected values; timeout and
page-budget cases construct the adapter with short test timeouts and small
limits, never by waiting out a production deadline. Bound any future child
process with a deadline and kill it on expiry.

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

Implement the adapter in test-driven slices: first a single authenticated page
through the shared traversal and one profile, then the second profile and wire
shape, then Link selection and continuations, then failures, then limits and
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
`docs/developers-guide.md` with the port, the `github_rest` traversal's scope
and reuse policy, how to add an endpoint profile, fixture reuse strategy, test
commands, credential seams, and proof-selection rationale. The component
architecture currently lives in `docs/repovec-appliance-technical-design.md`;
add the listing boundary, principal distinction, traversal contract, and
representation there. If a substantive accepted decision needs a separate ADR,
allocate the next unused `docs/adr-NNNN-*.md` number at implementation time and
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
dependency versions. Run `make install-build-tools` once if
`make check-build-tools` reports a missing prerequisite. Run a baseline
`make check-fmt`, `make typecheck`, `make lint`, and `make test` in that order.
Keep the shared cache and wrappers. `make lint` runs interrogate over the
Python sources, then Rustdoc and Clippy, then the Whitaker Dylint suite.
Whitaker is a binding gate: a missing suite fails the target. `make test` runs
the gate-runner script tests, then the unit tests and the doctests. Do not
replace these targets with narrower cargo commands as full gates.

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
make typecheck 2>&1 | tee /tmp/typecheck-repovec-2-2-1.out
make lint 2>&1 | tee /tmp/lint-repovec-2-2-1.out
make test 2>&1 | tee /tmp/test-repovec-2-2-1.out
make markdownlint 2>&1 | tee /tmp/markdownlint-repovec-2-2-1.out
make nixie 2>&1 | tee /tmp/nixie-repovec-2-2-1.out
```

`make markdownlint` runs `make spelling` first, so the spelling gate is
covered. Stop at a failing command; do not run the next gate until the failure
is understood and corrected. Inspect formatter changes before staging. Never
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
excessive resources. Bound the Link field before parsing it, test the selection
boundary and finite limits, and fail explicitly at boundaries. Do not add
background retries or parallel page fetching.

Medium impact, low likelihood: `nom-rfc8288` is a small crate with one
maintainer. It is confined to `link.rs`, pinned by a caret requirement and the
lock file, and covered by the boundary tests above, so it can be replaced
without touching the traversal. If it proves wrong on a case this design needs,
record the evidence and reopen the dependency decision.

Low impact, medium likelihood: `main` moves again before approval, and the
lint, build, or environment policy this plan cites changes. Re-check the
baseline, the gate list, and the cited guide sections at the start of M1, and
record any difference as a revision note before writing code.

## Progress

- [x] (2026-10-20) Read repository guidance, rename branch, and load skills.
- [x] (2026-10-20) Gather architecture and testing evidence with two Wyvern
      agents.
- [x] (2026-10-20) Check GitHub endpoint and pagination documentation with
      Firecrawl.
- [x] (2026-10-20) Complete and reconcile the six-expert design review.
- [x] (2026-10-20) Pass plan-only documentation gates. Publication is recorded
      in the draft PR metadata.
- [x] (2026-11-18) Rebase onto `main` at `df8c445` without conflicts, and
      revise the plan for the current build, lint, and environment policy.
- [x] (2026-11-19) Review third-party GitHub and generic clients; adopt
      `nom-rfc8288` and a profile-driven shared traversal.
- [x] (2026-11-19) Explicit approval of this plan; status set APPROVED.
- [x] (2026-11-19) M1: domain contract and test support. Added
      `repovec_core::repository_listing` (identity, summary, semantic errors,
      port), shared fixture builders in `repovec-test-helpers`, and the BDD
      specification file. 24 unit tests, 6 fixture contract tests, and 22
      doctests pass.
- [x] (2026-11-19) M2: endpoint modes and complete traversal. Added the
      crate-private `github_rest` traversal (profile, Link selection and
      confinement, budgets, status classification) and the
      `github_repository_client` adapter with both endpoint profiles. Evidence:
      102 unit tests, and integration suites covering protocol, continuation
      confinement, failures, budgets, generated page histories, a third
      non-repository profile, and all five BDD scenarios.
- [x] (2026-11-19) M2 gates. All six deterministic gates passed sequentially on
      the M2 tree: `make check-fmt`, `make typecheck`, `make lint` (interrogate
      100%, `cargo doc`, Clippy, Whitaker), `make test` (790 unit tests, 136
      doctests, 13 script tests), `make markdownlint` (including spelling), and
      `make nixie`. Logs are `/tmp/m2-check-fmt.out`, `/tmp/m2-typecheck.out`,
      `/tmp/m2-lint.out`, `/tmp/m2-test.out`, `/tmp/m2-markdownlint.out`, and
      `/tmp/m2-nixie.out`. M2 is committed at `0bae1bf`.
- [x] (2026-11-19) M3: observable composition and documentation. Added
      `repository_listing_command` (one credential acquisition, documented
      run statuses and exit codes, sanitized diagnostics, all-or-nothing
      stdout), the `list_github_repositories` example, eight offline
      end-to-end tests, and seven subprocess tests of the built example.
      Updated the users guide, developers guide, technical design, contents
      index, and roadmap 2.2.1, and recorded the shared traversal decision in
      `docs/adrs/adr-0002-shared-github-rest-traversal.md`.
- [x] (2026-11-19) M3 gates. All seven checks passed sequentially on the M3
      tree: `make fmt` (no tracked change), `make check-fmt`, `make
      typecheck`, `make lint` (interrogate 100%, `cargo doc`, Clippy,
      Whitaker), `make test` (813 nextest tests, 140 doctests, 13 script
      tests), `make markdownlint` (including spelling), and `make nixie`.
      Logs are `/tmp/m3-fmt.out`, `/tmp/m3-check-fmt.out`,
      `/tmp/m3-typecheck.out`, `/tmp/m3-lint.out`, `/tmp/m3-test.out`,
      `/tmp/m3-markdownlint.out`, and `/tmp/m3-nixie.out`. M3 is committed at
      `a36bd25`.
- [x] (2026-11-19) M3 repair: the example's child-process tests assumed that
      `cargo test --all-targets` builds a runnable example. It builds a libtest
      harness instead, so hosted CI run 44612002870 failed all seven tests on a
      clean runner. The command line moved into
      `repository_listing_command::cli` (`CliRun` plus `run`), the example
      became a thin process-boundary adapter, and the seven subprocess tests
      were replaced by fourteen in-process unit tests over injected streams.
      `.config/nextest.toml` needed no change; the earlier nested-build
      override was reverted.
- [x] (2026-11-19) Assessment A1 repairs. The hosted completeness assessment
      (comment 7023827653) reported three correctness defects and several
      verification gaps; all were reproduced against source before repair:
      (a) `next` relations were matched case-sensitively, so RFC 8288 permits a
      valid listing to end early and report completion — now compared
      case-insensitively with three unit cases and an end-to-end continuation
      case; (b) any `x-ratelimit-reset` header turned a `403` into a rate limit,
      hiding an access refusal — now only `Retry-After` or an exhausted
      `x-ratelimit-remaining` counts as evidence, with a regression case; (c)
      the completion event was emitted before domain validation and before the
      output was written — completion now follows every successful observation,
      and failure events carry the accepted page and record counts through a
      new `AcceptedWork` value. Verification gaps closed: generated histories
      now span 1–32 pages, and
      `crates/repovecd/tests/repository_listing_pagination_properties.rs` adds
      generated fresh and cyclic histories with exact request counts over both
      profiles. Both plan-mandated mutation witnesses were executed against the
      repaired tree and reverted. The example now installs a structured
      `INFO` subscriber, so its documented events are observable.

## Surprises & discoveries

2026-11-19, assessment A1: `nom-rfc8288` hands back parameter names and
relation values exactly as the server spelled them. RFC 8288 section 2.1.1
requires registered relation types to compare case-insensitively, and HTTP
field parameter names are conventionally case-insensitive, so the platform
crate deliberately does no canonicalization. Matching only `rel="next"` was
therefore a silent completeness defect: an otherwise valid listing ended at the
first capitalized link and was reported as complete, which no typed error or
count revealed.

2026-11-19, assessment A1: GitHub documents `x-ratelimit-*` as ordinary
response headers, not as rate-limit-only signals, and its secondary-limit
signal is `Retry-After`. Inferring "rate limited" from a reset timestamp
therefore reclassified a permission refusal as a wait-and-retry condition, and
the operator would have waited for a remedy that could not arrive.

2026-11-19, assessment A1: `clippy::too_many_arguments` applies to private
helpers as well, so threading accepted page and record counts into the failure
event needed a value type rather than two more parameters. `AcceptedWork`
became that value, and it also gives the success path one place to state what a
completed traversal accepted.

2026-11-19, M3 repair: `cargo test --all-targets` does not build a runnable
example. It compiles `examples/<name>.rs` as a libtest harness — a binary that
ignores the example's arguments and carries no tests — so a clean target
directory holds no `examples/<name>` executable at all. The subprocess tests
had passed locally only because an earlier manual `cargo build --example` had
left the hardlink behind; on a cold hosted runner every one failed with "no
example binary found beside …". Removing the hardlink and re-running
`cargo test --all-targets --no-run` never recreated it, while
`cargo build --example` did.

2026-11-19, M3 repair: the plan's sanctioned fallback — a nested
`CARGO_NET_OFFLINE=true` build of the example inside the test — is not viable
on this host. `cargo build --profile test --example` compiles the whole
dependency graph in a profile the enclosing gate has not built, and on a cold
tree that compile waits behind the shared compile-admission queue; the first
test to run exceeded its 300-second allowance while the nextest run ceiling is
10 minutes. Focussed `cargo test -p repovecd --test repository_listing_example`
therefore reported `TIMEOUT [300.004s] (1/7)` before any assertion ran.

2026-11-19, M3: `clippy::cognitive_complexity` counts each `tracing::info!` or
`tracing::error!` expansion at roughly ten points against a threshold of 9, so
at most one such call fits in a function and one cannot share a function with a
`match`. `run_listing` was split into `record_start`, `record_completion`,
`write_output`, `report_failure`, `write_diagnostic`, and `diagnostic_status`,
with a `RunContext` holding the span and start instant to stay inside the
four-argument limit. An experiment that removed only the two `info!` calls
brought the original function back under the threshold, confirming the cost is
the macro expansion rather than the matching logic.

2026-11-19, M3: `allow-expect-in-tests` does not cover integration-test helper
functions, because they are outside a `#[cfg(test)]` module, while
`clippy::panic_in_result_fn` forbids assertions in a `Result`-returning test.
The tests therefore return `()` and unwrap only inside the `#[test]` body, and
every helper propagates with `?` instead of calling `expect`. A blanket
replacement applied to the wrong region produced four `expect_used` errors in
exactly those helpers.

2026-11-19, M3: with `[unstable] embed-metadata = false`, the test binary does
not live in `target/debug/deps`; it sits under
`target/debug/build/<package>/<hash>/out`. The example's tests walk the test
binary's ancestors to find `examples/list_github_repositories`, which keeps the
2026-11-18 decision to avoid a nested `cargo` build and stays correct under a
custom target directory. **Superseded the same day:** that lookup works only on
a target directory where the example has already been built by hand. A clean
tree has no runnable example, so the command line moved into the library; see
the M3 repair entries below.

2026-11-19, M2: the continuation identity is checked before the request is
sent, so a continuation-identity budget too small for the initial request stops
the traversal with zero requests. This is deliberate: a request whose identity
cannot be retained could never be recognized if it recurred. The test records
this ordering.

2026-11-19, M2: Rust integration tests share a `tests/common` module per test
binary, so any helper a binary does not use is dead code, and the workspace
denies `allow`. The fixtures now live in `repovec-test-helpers` (a library,
where unused public items never warn) and each remaining shared helper sits in
its own module of `tests/common`, declared by every binary that needs it.

2026-11-19, M2: Whitaker's `module_max_lines` counts a module's file, so the
scripted server had to be split. `recording_http` now keeps the listener and
its worker thread, with `expectation` holding the scripted responses and
recorded requests and `wire` owning request parsing and response writing. Each
file stays well below the 400-line cap.

2026-11-19, M2: the installed Whitaker suite is a rolling release, so a gate
that passed when `main` merged can fail afterwards without any source change.
At the start of the M2 gate run the tip flagged two pre-existing functions in
`repovec-ci`; that repair belongs to its own change (PR #265), and this branch
merges it rather than duplicating it. The same run flagged one branch cluster
in this milestone's `link` module, which now compares origins as one derived
tuple.

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

2026-11-19, assessment A1: compare link parameter names and the registered
`next` relation case-insensitively, and keep the comparison on the
repository-owned side of the `nom-rfc8288` boundary. The parser owns the
grammar; deciding which relation names a continuation is this crate's policy,
and RFC 8288 makes case-insensitivity part of that policy. The alternative —
patching or wrapping the third-party parser — would put a repository rule in a
dependency and still need this comparison.

2026-11-19, assessment A1: classify a `403` from positive rate-limit evidence
only. `Retry-After` is GitHub's secondary-limit signal and an exhausted
`x-ratelimit-remaining` is its primary one; a reset time accompanies ordinary
responses and proves nothing about the refusal. The plan required "rate-limit
evidence" without saying which headers supply it; the reset timestamp was read
as sufficient, which was too weak to distinguish a refusal from a wait, so the
stronger rule is recorded here and in the verification plan.

2026-11-19, assessment A1: emit the run's completion event after the domain
conversion and after the output write, and give every failure event the
accepted page and record counts. A completion event that precedes validation or
a failed write describes a run that produced no usable output, and it was the
only place the plan's event contract could be satisfied without reporting a
false success. For a traversal failure the counts come from the adapter that
owns the traversal; the composition's failure event reports the sanitized
category and elapsed time, because the domain port deliberately returns no
partial records.

2026-11-19, assessment A1: two review suggestions are recorded as justified
non-changes. Trybuild coverage is not applicable: this feature introduces no
macro, no feature-gated public contract, and no compile-fail surface, and the
only feature is a crate-internal `test-support` re-export exercised by
integration tests. The `UnexpectedStatus { status }` variant keeps its numeric
status because it is the bounded, documented error surface the plan specifies
for unmodelled responses, and operators need the status to diagnose a proxy or
gateway in the path.

2026-11-19, M3 repair: keep the operator command line in the library.
`repository_listing_command::cli` owns `CliRun`, the argument parser, the usage
text, and the exit-status mapping, and the example supplies only the process
boundary and the production credential loader. This keeps every argument,
usage-text, and exit-status assertion that the child-process tests made, while
removing all build coupling: no test compiles or starts a binary, so a clean
target directory, a custom target directory, and the coverage lane's
`llvm-cov-target` behave identically. It also restores the plan's original "the
test does not start a nested `cargo`" constraint, which the superseded fallback
had relaxed. `cargo clippy --workspace --all-targets` still type-checks the
example, so it cannot rot silently, and the documented
`cargo run -p repovecd --example list_github_repositories` surface is unchanged.

2026-11-19, M3: `run_listing` returns a `RunStatus` rather than an error, so
one place maps every outcome to a documented exit code: 0 success, 3 credential
failure, 4 listing failure, 5 output failure, 6 diagnostic failure, with 2
reserved for a usage error the caller decides before composing a run. A failed
diagnostic write is its own status because the operator has lost the message
that explained the earlier failure.

2026-11-19, M3: the composition injects the API endpoint, the limits, both
output streams, standard input, and the credential loader, so the runnable
example and the end-to-end tests call the same function. Installation mode
receives a loader that the test can make fail on any call, which proves the
mode never reads the user store rather than merely observing that it did not
this time.

2026-11-19, M2: expose the generic traversal to integration tests through a
`test-support` Cargo feature and a self dev-dependency
(`repovecd = { path = ".", features = ["test-support"] }`). The third
non-repository profile must run through the real traversal, and only
`cfg(any(test, feature = "test-support"))` can make crate-private code visible
to an integration test without widening the production API. The feature adds a
re-export of crate-private seams only; a plain `cargo build --lib` enables
nothing.

2026-11-19, M2: `repository_listing_wire` in `repovec-test-helpers` owns the
wire fixtures, the documented request targets, and the fake credentials. Both
the adapter tests and future listing tests (2.2.2) need the same protocol
vocabulary, so it is shared rather than copied per binary.

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

2026-11-18, rebase: Cranelift is out of scope. The experiment found that its
compile-time gain was outweighed by a threefold increase in binary size and
slower test execution. It stays an unsupported compile-time option, to be
revisited in about six months (May 2027). This work targets the LLVM backend
only and makes no accommodation for Cranelift beyond ordinary Rust hygiene.

2026-11-18, rebase: locate the example binary from the test binary's own path
instead of building it with a nested `cargo` and reading Cargo metadata. `main`
now bounds nested `cargo` spawns in tests, requires them offline, and holds the
`test` recipe to one runner command. `--all-targets` already builds the
example, so the nested build bought nothing and would have contended for the
package-cache lock. **Superseded 2026-11-19:** `--all-targets` builds the
example only as a libtest harness, so no runnable example exists on a clean
tree. The command line is now library code tested in-process; see the M3 repair
entry above.

2026-11-18, rebase: read no environment variable in this feature. The workspace
now denies seven `std::env` methods outside an annotated composition root.
Store path, token input, and endpoint already arrive by argument, stream, or
injection, so no exception is needed.

2026-11-19, review: use `nom-rfc8288` for the Link field grammar instead of a
handwritten parser. The grammar was the part of this plan most likely to hide
defects, and the crate returns all link-values and parameters, which is what
ambiguity detection needs. Selection, confinement, and budgets stay ours
because no examined client provides them: `octocrab` lacks body, page, and
cycle limits and parses the Link field loosely, and no generic consumer
bundling these concerns was found.

2026-11-19, review: make the traversal one function generic over the item type,
driven by an endpoint profile of path, fixed query, and item location. Roadmap
item 2.2.2 needs the same traversal for branches and open pull requests, per
repository, so a repository-specific loop would be duplicated within one
roadmap step. This reverses the earlier "endpoint-specific, not a pagination
engine" position. A `page-turner`-style trait was rejected: all known consumers
share one protocol, so a trait would spread the same continuation logic across
implementations and share none of the safety.

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

Rebase on 2026-11-18: the two plan commits replayed onto `main` at `df8c445`
without conflicts, and `git range-diff` reported both unchanged. The spelling
gate on the new baseline rejected the page-sequence symbols in the abstract
specification, so they were renamed `page_0` to `page_n`. The plan was then
revised as the revision note below records. Documentation gates passed on the
revised plan: `make fmt`, `make markdownlint` (including spelling), and
`make nixie`. Still no feature code has been written.

Implementation completed on 2026-11-19. M1 delivered the domain contract and
shared fixtures, M2 the confined profile-driven traversal and both endpoint
profiles, and M3 the one-shot composition, runnable example, offline end-to-end
evidence, and documentation. R1 is discharged by the user-store and
installation end-to-end cases, R2 by mixed-owner fixtures and the BDD
scenarios, R3 by generated traversal histories and HTTP continuation tests, C1
by port review, credential-leak assertions, and the design update, and C2 by
the late-failure cases that leave stdout empty. All seven deterministic gates
passed on the M3 tree, and roadmap item 2.2.1 is marked complete with its
status note.

Formal verification remained not applicable for this transport and
representation boundary, as the plan anticipated; the rationale is recorded in
ADR 0002 and section 9.6 of the developers guide. Branch listing (2.2.2) and
periodic scheduling (2.2.3) are untouched and remain open. The example is the
documented operator surface until the TUI and `repovectl` compose the same
module.

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

Checked with Firecrawl on 2026-11-19:

- [nom-rfc8288 API](https://docs.rs/nom-rfc8288/0.4.0/nom_rfc8288/complete/index.html):
  adopted parser; `link_strict` returns each link-value with ordered
  parameters. Version 0.4.0, MIT or Apache-2.0.
- [octocrab pagination source](https://github.com/XAMPPRocky/octocrab/blob/v0.54.9/src/page.rs):
  Link splitting, unbounded body collection, and the envelope key list.
- [octocrab authorization layer](https://github.com/XAMPPRocky/octocrab/blob/v0.54.9/src/service/middleware/auth_header.rs):
  credential attached only for the configured base and upload hosts.
- [page-turner API](https://docs.rs/page-turner/1.0.0/page_turner/): generic
  pagination trait; async-only, no traversal budgets.

Revision note (2026-10-20): initial plan records contracts before
implementation, explicit credential modes, proportionate verification, and
gated delivery. Revision note (2026-10-20): incorporated all six experts'
findings. The changes strengthen resource and error contracts without expanding
lifecycle scope. Documentation validation passed; the draft PR records
publication, while implementation remains subject to approval.

Revision note (2026-11-18): rebased onto `main` at `df8c445`. Updated the
baseline, compiler pin, and gate list; adopted the environment access policy
and the nextest budgets; replaced the nested example build with a path derived
from the test binary; and recorded that Cranelift is out of scope. The
behavioural contract, interfaces, and milestones are unchanged, so the plan
still awaits approval as before.

Revision note (2026-11-19): replaced the handwritten Link grammar with
`nom-rfc8288`, and restructured the adapter into a shared, profile-driven
`github_rest` traversal plus a repository client holding two profiles. The
port, the behavioural contract, the limits, and the milestones are unchanged.
This adds one runtime dependency and one internal module boundary, both
recorded in the decision log, and remains subject to approval.
