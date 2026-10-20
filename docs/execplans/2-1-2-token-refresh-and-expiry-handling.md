# Implement token refresh and expiry handling (2.1.2)

## Status and purpose

Status: DRAFT — awaiting explicit approval before implementation.

This plan extends GitHub authentication so an expired credential is refreshed
when possible and otherwise starts one new device authorization flow. Token
lifecycle events reach journald without exposing credentials. Success is
observable through the application use case and an executable local-server
workflow that exercises login, expiry, refresh, restart, and reauthentication.
The daemon and TUI are currently skeletons; this task delivers the reusable
control-plane operation and test executable, not the future TUI or discovery
scheduler. Approval includes this delivery boundary.

Only this plan and its documentation-index entry are changed during planning.
The roadmap checkbox remains open until the feature and its acceptance evidence
are complete. Neither publishing nor merging this plan authorizes
implementation.

## Constraints

Preserve the existing browserless-on-VM device flow, encrypted storage under
`/etc/repovec/`, redacted secret types, and application-owned ports. Pure
policy belongs in `repovec-core`; HTTP, filesystem, clocks, and tracing belong
in `repovecd`. No adapter calls another adapter: application orchestration
connects them. Keep device-code expiry separate from access-token expiry.

Use the existing `oauth2`, `reqwest`, `monotony`, `cap_std`, and `camino`
boundaries. No client secret, new OAuth scope, live GitHub credentials, daemon
service changes, or automatic browser launch is required. Preserve post-restart
scope revalidation; stored scopes must not become authorization evidence. Never
log access tokens, refresh tokens, device codes, user codes, complete HTTP
bodies, authorization headers, or credential plaintext. Present the user code
only through the existing prompt callback.

All code files remain below 400 lines. Use typed errors and injected time and
I/O, with no parent-process environment mutation. No `assume`, `admit`, trusted
proof body, or equivalent proof shortcut is permitted, even temporarily.
Unproved statements stay in this plan until a substantive proof is possible.

## Tolerances (exception triggers)

Stop implementation and record a proposed deviation before adding another
credential provider, database, service, daemon scheduler, user-facing command,
or public network endpoint. Also stop if the approved persistence
representation or proof claim must change, if the pinned verifier cannot
establish the selected properties, or if a client secret is required by the
actual application setup. Record affected requirements and options; await
explicit approval of the change.

Small private helper splits, test fixture arrangements, and names may change
without approval if all contracts and trace links remain intact. There is no
permission to weaken tests, suppress warnings, bypass compiler admission, or
claim a skipped gate passed. Stop on full disk; do not clean unrelated work.

## Risks

Refresh rotates both credentials. A lost response or failed local commit can
leave the previous pair unusable. The design therefore records refresh intent
before sending the request and treats interrupted rotation as requiring login.
This deliberately favours safe recovery over transparent retries.

Wall time can move backwards and process-local monotonic time cannot survive a
restart. Store absolute expiry metadata, use monotonic elapsed time in-process,
and fail closed on detected inconsistent time. Undetectable clock error remains
an external assumption; an authenticated rejection still invalidates the token.

The old credential contains bare token bytes. A versioned replacement must not
misread corrupt new records as legacy tokens. Existing deployments need a
one-way reader migration, but private source APIs do not need compatibility
wrappers. Downgrading after migration requires login again with the old binary.

CodeGraph MCP was unavailable during planning after tool discovery. Targeted
reads and a context pack supplied the evidence. Recheck availability during
implementation; do not claim semantic-index evidence that was not obtained.

## Progress

- [x] 2026-10-20: Read repository instructions and rename the branch.
- [x] 2026-10-20: Complete two Wyvern reconnaissance tasks and protocol
      research.
- [x] 2026-10-20: Draft contracts, proof sketch, and implementation milestones.
- [x] 2026-10-20: Complete six-perspective expert review and reconcile findings.
- [ ] Validate and publish the plan in a draft pull request.
- [ ] Obtain explicit approval before executing EP-M1.
- [ ] EP-M1: Establish policy, contracts, and Verus proof.
- [ ] EP-M2: Deliver metadata-preserving HTTP and encrypted storage adapters.
- [ ] EP-M3: Deliver lifecycle orchestration and journal events.
- [ ] EP-M4: Demonstrate workflows, reconcile documentation, mark 2.1.2 done.

## Surprises & discoveries

The baseline token type carries only bearer text and scopes. The wire adapter
discards expiry/refresh metadata, and the credential adapter persists only the
bearer. No current daemon entry point consumes a token lifecycle operation.
Existing device-code polling already uses an injected monotonic clock.

Current GitHub documentation describes expiring OAuth-app tokens and refresh
for device-flow grants.[^1] A structured web extraction confused device-code
lifetime with access-token lifetime; reading the full source resolved this. The
implementation must use returned metadata, not hard-coded eight-hour or
15-minute assumptions and not token-prefix guessing.

## Decision log

- 2026-10-20: Proposed capability-driven refresh. The presence of a usable
  refresh credential, not application branding or token prefix, determines
  whether refresh is attempted. Keep requested scopes unchanged.
- 2026-10-20: Proposed one serialized lifecycle operation per credential store.
  A store-wide exclusive lease spans load, decision, network exchange, and
  commit; competing operations return a typed busy result. This avoids stale
  writers without introducing distributed generations or background actors.
- 2026-10-20: Proposed encrypted refresh-intent record before network I/O.
  Restart after an uncertain rotation requires device authorization. No retry
  of a potentially consumed refresh credential is automatic.
- 2026-10-20: Select Verus only for the pure transition model's safety over
  arbitrary histories. HTTP, journal delivery, cryptography, and filesystem
  durability remain integration contracts tested at their boundaries.

## Outcomes & retrospective

Planning only. No feature or proof has been implemented, no implementation gate
has been claimed, and the roadmap task is not complete. Fill this section with
actual evidence after each approved milestone, including residual limitations.

## Context and orientation

Baseline: commit `7763a99f5dff25547a972888ed26d6381c465bb9`.
`crates/repovec-core/src/github_oauth/mod.rs` defines `AccessToken`,
`DeviceAuthorization`, and `TokenPollOutcome`. The authorization's `expires_in`
limits device-code polling, not the issued access token.

`crates/repovecd/src/github_device_flow/mod.rs` defines `DeviceFlowApi`,
`TokenStore`, `Sleeper`, `DeviceFlowRuntime`, and `complete_device_flow`.
`polling.rs` in that directory enforces device-code deadlines. Extend the
application boundary coherently, updating every private caller and doctest. Do
not add aliases just to preserve pre-1.0 internal signatures.

`crates/repovecd/src/github_oauth_client/wire.rs` maps token HTTP responses;
`mod.rs` owns `GitHubOAuthClient`. `github_token_store/mod.rs` owns
`EncryptedGitHubTokenStore::store_token` and `load_token`; its
`atomic_write.rs` already implements replace-and-sync persistence. Preserve the
credential name `repovec-github-oauth-token` and filename
`github-oauth-token.cred`.

Existing executable evidence lives in
`crates/repovecd/examples/device-flow-test.rs`. Existing tests include
`crates/repovecd/tests/github_oauth_client.rs`, the device-flow and token-store
`tests.rs` modules, and
`crates/repovec-core/tests/features/github_oauth.feature`. The daemon's
`main.rs` currently initializes tracing and runs startup validation only.

## Conformance basis

The upstream basis is `docs/roadmap.md` section 2.1.2 and
`docs/repovec-appliance-technical-design.md`, sections “Control plane” and
“Authentication: device flow”, at the baseline above. There is no separate
Terms of Reference or authentication ADR. Existing ADR 001 concerns directory
provisioning and continues to govern directory ownership.

Use these trace identifiers:

- R-EXP: detect expiry, including known deadlines and authenticated rejection;
  EP-M1/EP-M3, evidence in policy and lifecycle tests.
- R-RECOVER: refresh where available, otherwise initiate device flow;
  EP-M2/EP-M3/EP-M4, evidence in protocol fixtures and executable scenarios.
- R-JOURNAL: log lifecycle events to journald;
  EP-M3/EP-M4, evidence in captured tracing and an actual journal smoke test.
- A-BOUNDARY: inward dependencies, encrypted persistence, restart permission
  policy, and no secret disclosure; all milestones, adapter tests and review.
- P-SAFETY: never release a known expired, invalidated, or uncommitted token;
  EP-M1 proof, EP-M2 persistence tests, EP-M3 transition tests.

Required reading before implementation:
`docs/rust-testing-with-rstest-fixtures.md` for fallible fixtures;
`docs/rstest-bdd-users-guide.md` for behavioural runners;
`docs/reliable-testing-in-rust-via-dependency-injection.md` for clock and I/O
seams; `docs/rust-doctest-dry-guide.md` for `.rsdoc` examples;
`docs/complexity-antipatterns-and-refactoring-strategies.md` for small cohesive
modules; and `docs/ortho-config-users-guide.md` if configuration wiring
changes. Also read `docs/documentation-style-guide.md`, `docs/users-guide.md`,
and the GitHub OAuth section of `docs/developers-guide.md`.

Load `codegraph-mcp`, `hexagonal-architecture`, and `rust-router` skills. Use
`rust-verification` and `verus` for EP-M1, `firecrawl-mcp` for external
contract gaps, and `execplans` to maintain this document. The pinned nightly is
`nightly-2026-03-12`; borrow-checker posture is unverified, so no ownership
design should depend on an assumed Polonius mode. No ownership workaround is
proposed.

## Behavioural contracts and abstract specification

Define the specification before writing the implementation. A credential has an
identity, optional access expiry, optional refresh credential and its expiry,
and a status. A credential identity is an abstract label, not a logged secret
or token prefix. Absent expiry means “no advertised deadline”, not “cannot be
revoked”. Legacy records have unknown expiry and unknown permissions.

The states are `Empty`, `Ready`, `Refreshing`, and `NeedsDeviceFlow`. `Ready`
means durably stored, not necessarily unexpired. The pure decision receives
state, time evidence, and any authoritative rejection. It produces `Use`,
`Refresh`, `BeginDeviceFlow`, or a typed error/wait outcome. A token may be
released only from `Ready`, with no rejection and no reached known deadline.
Equality at a deadline counts as expiry. Invalid metadata and arithmetic
failure never turn into an unlimited lifetime.

Before fallback after authoritative rejection, durably commit `NeedsDeviceFlow`
so denial, cancellation, or restart cannot restore `Ready`. Before refresh, the
`Refreshing` commit serves the same retirement purpose. If retirement
persistence fails, return a storage error and release no token; revalidate
every loaded `Ready` record on restart, including known-deadline records.
Crash-safe retirement is proved for successful durable retirement commits;
failed persistence cannot prove knowledge survived a crash. A reload validation
must independently reject the old credential before it can be used.

Refresh is selected only for an expired/rejected access credential with an
available refresh credential whose known deadline has not been reached. Missing
or expired refresh capability starts device authorization. Persist `Refreshing`
before sending refresh; this state releases neither old token. A validated
successful response becomes `Ready` only after atomic persistence. Recheck both
deadline constraints immediately before publication, after all network and
storage work. A replacement already expired at that point returns a typed
unusable-credential error, never success or another recursive refresh. A
terminal `bad_refresh_token` response, a lost response, or a restart from
`Refreshing` requires device authorization. Transport and server errors must
not be described as token expiry; uncertain refresh outcomes still retire the
pair conservatively. Configuration errors end the operation with instructions
rather than requesting device codes repeatedly.

Each invocation performs at most one refresh and one fallback device flow.
Denied or expired device authorization ends the invocation. Subsequent calls
may try again explicitly. A transient validation failure returns a retryable
error without erasing the credential or starting login. An authenticated 401
means rejected credential, not proven expiry; 403, 429, 5xx, and network
failure must not become automatic reauthentication. A 403 may indicate
permissions or rate limits. Do not automatically replay a caller's failed
business operation.

On reload, validate every candidate `Ready` token using a minimal GitHub
authenticated identity request before `Use`. Successful validation does not
invent an expiry. Scope-derived permissions remain unknown unless actually
revalidated. Freshness of permission data is separate from token usability.

## Verification plan and proof sketch

P-SAFETY is the formal target because the no-reuse property spans arbitrarily
many refresh, failure, and restart transitions. Start with an independent
mathematical transition relation over credential identities, persisted state,
clock observations, and outputs. The environment can reject a token at any
time. Initial state is empty or a decoded valid record; recovery maps an
interrupted refresh to `NeedsDeviceFlow`. External success supplies a
replacement identity and metadata, but does not itself imply local persistence
success.

The proposed semantic lemmas are hypotheses to refine during implementation:

1. Release safety: every `Use(id)` output refers to the current committed
   credential and satisfies the access-deadline predicate at that observation.
2. Retirement preservation: after a successful durable refresh-intent or
   rejection commit retires an identity, no transition emits it again without a
   new accepted authorization and durable commit. Crash/reload cannot reverse
   retirement.
3. Rotation publication: refresh success followed by failed persistence cannot
   produce `Use` in that invocation; committing a replacement cannot publish a
   mixture of old and new credentials.
4. Trace preservation: initialization establishes the safety predicate and
   each permitted transition preserves it; induction on trace length proves the
   property for all finite histories, without an arbitrary test bound.

Preconditions include exclusive ownership of the store for the operation,
well-formed decoded metadata, and faithful reports of durable commit success.
They must not assume release safety or retirement preservation. Postconditions
constrain both next state and emitted action. The representation relation maps
Rust enum variants, optional deadlines, checked integer seconds, and encrypted
record status to abstract states; secrets map only to opaque identities.
Checked deadline arithmetic must refine mathematical addition or return error.

Implement `verus/github_token_lifecycle.rs` with complete proof bodies,
including initialization and preservation cases. Keep a documented executable
transition mirror and prove its refinement to the independent relation. Map
every mirrored branch to `repovec_core::github_oauth::lifecycle` and test the
actual Rust kernel against an independently written model over generated
traces. This establishes an exhaustive theorem for the model; the production
bridge is reviewed and tested, not a claim that Verus verifies HTTP or all
compiled Rust. Any desire for a stronger shared-source proof requires an
explicit plan revision.

Witnesses must include empty login, deadline equality, a usable refresh pair, a
failed commit after refresh, and crash after intent. A deliberate boundary
mutation (`<` to `<=` in the usability check) and a transition that releases
the old token after intent must be rejected by the proof or its refinement
checks. No false precondition or unreachable transition may discharge an
obligation. Record the failing negative-control output, restore the mutation,
then record the passing verifier output. Defer triggers and auxiliary solver
lemmas until real verification conditions exist.

The external assumptions are GitHub's token/rotation semantics, correct TLS and
HTTP library behaviour, truthful clock adapters, exclusive-lock semantics, and
filesystem durability after successful sync. Systemd encryption and journal
delivery are not proved. A persistence error is not an axiom that disk was
unchanged. The abstract commit operation permits old or new complete visible
records after a failed rename/sync sequence; neither permits publication by the
failing invocation. After restart, a complete new `Ready` record may be
accepted only after live validation and a new freshness check. A visible
retirement record requires reauthorization; unreadable or corrupt data fails
closed. Crash consistency tests must exercise each of these outcomes, and the
proof recovery relation must include them.

Clock rollback detectable from recorded observations must fail closed;
undetectable reboot-time skew and server revocation between check and request
remain explicit limits of any local freshness guarantee.

Conventional evidence complements the proof:

- `rstest` tables cover all decision partitions, deadline equality/zero,
  omitted fields, malformed/overflowing durations, rollback, and typed errors.
  Put policy cases in `github_oauth/lifecycle/tests.rs` and adapter-specific
  cases beside the corresponding modules. Use `googletest` assertions
  (supported alongside `rstest`)[^2] and `pretty_assertions` for structured
  equality; add caret dev-dependencies.
- `proptest` generates valid states, boundary-biased times, and sequences of
  length 0–128 against the independent model. Record coverage of every state
  and action, with no filtering that removes failure paths. This tests the
  Rust-to-model bridge; it does not replace the unbounded Verus theorem.
- HTTP contract tests in `crates/repovecd/tests/github_oauth_client.rs` verify
  method, form body, endpoint, no secret in URL, returned metadata, 401 versus
  403/429/5xx, malformed success, and `bad_refresh_token`. Retain
  `oauth2-test-server`; use a small scripted loopback fixture where its GitHub
  error/rotation support is insufficient.
- Storage tests exercise round trips, legacy migration, unknown versions,
  truncation, decrypt failure, lock contention, and failure before/after each
  rename/sync boundary. Restart from refresh intent must never send the old
  pair. Tests must use real temporary capability directories; injected command
  runners cover deterministic `systemd-creds` failures.
- Application tests check exact port-call order, single refresh/fallback bounds,
  failed validation, prompt before polling, and persistence before publication.
  Mutating the ordering must fail these tests. A separate process test proves
  that two processes cannot both refresh the same stored credential.
- `insta` snapshots cover redacted typed error output and equivalent lifecycle
  events rendered by text and JSON tracing subscribers. Explicit assertions
  independently scan both formats for sentinel secrets. Normalize only time and
  nondeterministic identifiers, not event kinds or failure categories.

No Verus obligation is assigned to diagnostics formatting, HTTP plumbing,
cryptography, or journald. Kani adds no distinct assurance for the selected
unbounded model and adapter contracts, so it is not planned.

## Interfaces and implementation design

Add `crates/repovec-core/src/github_oauth/lifecycle/` for pure lifetime and
transition types. Use a redacted `RefreshToken` and a `TokenCredential`
aggregate containing `AccessToken`, access lifetime, optional refresh
capability, and issuance evidence. Model correlated status as an enum, not
independent flags. A pure
`decide_lifecycle(state, observation) -> LifecycleAction` function must remain
independent of clocks, `serde`, tracing, and I/O.

Add `crates/repovecd/src/github_token_lifecycle/` as the application owner of
`ensure_usable_token(runtime, request, present_prompt)`. Its ports expose typed
`refresh`, `validate`, credential `load`/`commit`, exclusive lease acquisition,
and wall-clock observations. Reuse `MonotonicClock`, `Sleeper`, and the device
prompt callback. Lease acquisition returns a lease-owned commit capability,
preventing unlocked writes in initial login and every other application path.
The result contains a usable credential or typed busy, retryable,
configuration, storage, or terminal-login error. Never expose a candidate
before persistence. Update existing device-flow storage and result contracts
together so initial login also retains metadata.

The HTTP adapter requests JSON and posts `client_id`,
`grant_type=refresh_token`, and `refresh_token` as form data to the token
endpoint. Device-flow refresh requires no client secret.[^1] Do not send a
scope override. Parse access and refresh lifetimes independently; refresh
expiry without a refresh token is invalid. An absent access expiry remains
unknown, a zero lifetime is immediately expired, and a duration that cannot be
represented is a typed protocol error. Never return a token with a non-bearer
token type or conflicting success/error fields. Reject missing `token_type` as
malformed success, and update old mock fixtures to supply the required bearer
type. This changes only the wire decoder; legacy stored bare tokens remain
readable through their separate migration path.

Capture time conservatively before the token request; derive absolute expiry
with checked addition so network latency cannot extend usable lifetime. During
a process, both known wall deadline and monotonic elapsed deadline constrain
use. On restart, load absolute deadlines and reject clocks earlier than stored
issuance/observation evidence. Unknown-expiry credentials require validation on
reload and must still respond to subsequent authoritative rejection.

Version the encrypted plaintext with an unambiguous magic prefix and schema
version, then serialize the record as JSON. Store access/refresh secrets,
deadlines, clock evidence, and `Ready`, `Refreshing`, or `NeedsDeviceFlow`
status together in the existing encrypted credential file. Do not persist
granted scopes as authority. Accept old non-empty bare bearer records only
through an explicit legacy reader; malformed prefixed records never fall back
to that reader. A successful live validation migrates the legacy record
atomically. Apply a 64 KiB plaintext record limit and bounded HTTP response
reads; legacy input must be a non-empty single-line bearer with no control
characters. Reject ambiguous input without printing it. Unsupported versions
and corrupt files fail closed with a recovery message, without overwriting
evidence.

Acquire a stable sibling lock file, not the replaceable credential inode. Use a
capability-rooted file and an OS advisory exclusive lock; select the smallest
supported Rust locking API during EP-M2. Hold the lease through the bounded
operation and release it on every return. Initial login, explicit re-login,
migration, and refresh all use this same lease; there is no public unlocked
write path. Refactor device-flow orchestration to accept the held lease
internally, avoiding recursive acquisition. Set explicit HTTP connect and total
request timeouts (5 and 30 seconds), and a 30-second credential command timeout
that terminates and reaps only the child it created. Bound device authorization
by the smaller of the advertised lifetime and 15 minutes, and allow no
automatic refresh retries. Test timeout exits release the lease. Busy is
immediate and asks the caller to retry after the active operation finishes; no
hidden waiting queue is added. Cancellation drops the lease and leaves the last
persisted state recoverable. Test holder-process termination and initial-login
versus refresh contention. Locks are cooperative: document that all appliance
writers use this port. No unsafe filesystem escape or global mutex workaround
is permitted.

Lifecycle events use existing `tracing` output captured by systemd. Emit
`token_loaded`, `token_expired`, `token_rejected`, `refresh_started`,
`refresh_succeeded`, `refresh_failed`, `reauthentication_required`, and
`token_stored` with stable `event`, `reason`, and `outcome` fields from a
bounded vocabulary. Use info for normal transitions and warn for rejected
credentials, failed refresh, and failed persistence. Emit intent only after its
durable commit; emit success only after replacement commit and final usability
check. Journal unavailability does not roll back a committed credential or
change the authentication result. Success means the replacement was committed.
Do not emit one event per polling iteration as a lifecycle transition, and do
not embed upstream error bodies in messages.

## Milestones and plateaus

### EP-M1 — Specify and verify the pure policy

After approval, first add the smallest failing expiry-equality and retirement
regressions; run them before production edits. Implement the pure model, then
unit/property tests and substantive Verus proof. Add pinned
`tools/verus/VERSION`, `tools/verus/SHA256SUMS`, and
`tools/rust-prover-tools/REF`, following the `verus` skill's project on-ramp
and the upstream runner.[^3] Add `make install-verus` and `make verus`
delegating to pinned `prover-tools`; no shell downloader and no successful skip
stub counts as acceptance.

Record the architectural decisions in proposed
`docs/adr-002-github-token-lifecycle.md` and link it from the technical design.
Explain proof scope and the production representation relation there. Check
whether ADR 002 has been allocated by concurrent work before choosing the name.
This plateau adds tested policy without changing current runtime behaviour.
R-EXP and P-SAFETY are advanced; adapter and end-to-end obligations remain.
Accept only after negative controls, complete proofs, and ordinary code gates
pass. Recovery is a focused revert of this unconsumed policy commit.

### EP-M2 — Preserve metadata and make rotation recoverable

Write failing wire, credential round-trip, legacy, and lock-contention tests.
Extend `github_oauth_client` and `github_token_store`, splitting codecs and
lease handling into cohesive files. Promote `serde_json` to a runtime
dependency only in the adapter crate if required. Update every initial-login
caller, fixture, and `.rsdoc` example atomically. Retain existing data-file
compatibility only for credentials written by 2.1.1; do not preserve obsolete
internal APIs.

The plateau allows device login to persist complete metadata and exposes tested
refresh/validation adapters. R-RECOVER and A-BOUNDARY advance; automatic
orchestration remains EP-M3. Accept round-trip/migration/failure evidence and
all gates. Recover by reverting code before deployment; after format migration,
re-login on downgrade rather than presenting JSON as a legacy bearer.

### EP-M3 — Orchestrate one lifecycle operation and emit events

Write failing application tests for expired-with-refresh and expired-without-
refresh cases, then implement `ensure_usable_token`. Use the exclusive lease,
refresh-intent commit, one refresh, and at most one device-flow fallback.
Refactor `complete_device_flow` as necessary to share policy through
application functions, not adapter-to-adapter calls. Add redacted events and
the text/JSON snapshot suite. Retain the existing flow's poll intervals and
denial/expiry semantics. No recursive retry loop is allowed.

This plateau delivers the callable control-plane operation. Accept port-order,
crash/reload, competing-process, snapshot, and property tests plus ordinary and
Verus gates. R-EXP/R-RECOVER/P-SAFETY are connected to executable behaviour;
actual journal transport and operator evidence remain EP-M4. Recovery is a
focused revert; credentials in `Refreshing` still require login, never replay.

### EP-M4 — Demonstrate workflows and complete documentation

Add `crates/repovecd/tests/github_token_lifecycle_bdd.rs` and
`crates/repovecd/tests/features/github_token_lifecycle.feature` using
`rstest-bdd`. Add a sibling `token-lifecycle-test` example and process-level
runner that uses the real application, HTTP adapter, and encrypted store with a
local scripted server. Keep the fixture deterministic and independent of live
GitHub accounts. Include real `systemd-creds` coverage on a supported Linux
host; a fake encryptor alone is insufficient end-to-end persistence evidence.

Update `docs/users-guide.md` with expiry, refresh, prompt, failure, migration,
and journal-reading behaviour. Update `docs/developers-guide.md` with port
ownership, clock assumptions, lock discipline, proof commands, fixtures, and
reuse of `ensure_usable_token` by future discovery/TUI work. The technical
design serves as the component architecture document: record the lifecycle
interfaces and reference the ADR. Update `docs/contents.md` for all new
documents.

Run actual journal smoke evidence in an isolated transient unit, never by
altering the host appliance services. A new example mode exercises refresh or
fallback and sends tracing to the journal; `journalctl` must contain the
expected redacted events. Exercise both committed refresh success and
fallback/storage failure; assert event order and scan actual journal text for
all four sentinel secret kinds. Record the exact unit name, commands, and
observed events. For example, after building the new example, run these on a
host with a user systemd manager:

```sh
systemd-run --user --wait --collect --unit=repovec-token-lifecycle-proof \
  --property=StandardOutput=journal --property=StandardError=journal \
  "$PWD/target/debug/examples/token-lifecycle-test" --scenario journal
journalctl --user -u repovec-token-lifecycle-proof --no-pager -o cat
```

Use the resolved Cargo target directory if it differs from `target/`. The
`journal` scenario uses only local fixtures and emits both required outcomes.
If a working systemd journal is unavailable, report an unfulfilled acceptance
item rather than treating captured stdout as journal proof.

Accept all workflow evidence, gates, and documented limitations; then mark
roadmap 2.1.2 checked and explicitly `done`. Reconcile all trace links and ADR
claims before setting this plan COMPLETE. Recovery before acceptance is a
focused revert of documentation/example changes; do not mark completion early.

At every plateau, record requirement evidence, continued boundary conformance,
upstream assumptions, approved dependencies and persisted formats, and
remaining gaps. An unapproved departure sets status BLOCKED pending explicit
approval. Commit each coherent passing change; keep behaviour-preserving
refactors in separate gated commits after functional commits.

## Behavioural specification

The new feature file must include these scenarios and expand the same matrix
for malformed metadata, access denial, transient validation failure, concurrent
requests, and secret redaction:

```gherkin
Feature: Recover GitHub authentication when credentials expire
  Scenario: Rotate an expired access token
    Given a stored expired access token and a usable refresh credential
    When the lifecycle operation receives a successful refresh response
    Then the replacement pair is committed before it is returned
    And no device authorization is requested

  Scenario: Reauthorize without refresh capability
    Given a stored expired access token without a refresh credential
    When the lifecycle operation runs
    Then one device authorization prompt is presented before polling
    And approval commits the new credential before success

  Scenario: Recover from an interrupted refresh
    Given the durable credential state records an unfinished refresh
    When the application restarts and the lifecycle operation runs
    Then the old pair is neither returned nor submitted for refresh
    And one new device authorization flow is started

  Scenario: Reject publication after a storage failure
    Given GitHub has returned a replacement token pair
    When committing the replacement fails
    Then the operation returns a storage error without a token
    And no refresh success event is emitted
```

## Concrete steps and validation

Run commands at the repository root on the approved task branch. Establish
baseline gates before edits. Within each milestone, run the new focused test
before implementation and record its expected assertion failure, then the same
command after implementation and after refactoring. A compile error alone is
not the desired behavioural red evidence once the test seam exists.

Planned focused commands (new targets do not exist during planning):

```sh
cargo test -p repovec-core github_oauth::lifecycle
cargo test -p repovecd --test github_oauth_client
cargo test -p repovecd github_token_lifecycle
cargo test -p repovecd --test github_token_lifecycle_bdd
cargo run -p repovecd --example token-lifecycle-test
make install-verus
make verus
```

`make verus` must invoke the equivalent pinned command:

```sh
prover-tools verus run --repo-root . --proof-file verus/github_token_lifecycle.rs
```

A successful proof has zero verification errors and non-zero verified
obligations. Record the exact release and summary, not invented counts. Every
proof-changing milestone, including EP-M1, requires this check in addition to
ordinary gates. Missing tooling, timeouts, and FORMAL-SKIP are not success.
Keep proof execution separate from ordinary Cargo targets initially; document
an on-demand continuous integration job with the same pins.

Delegate full commit gates to `scrutineer`, once per candidate, sequentially.
Use `set -o pipefail` and `tee` for each command; for example:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/fmt-repovec-2-1-2.out
make check-fmt 2>&1 | tee /tmp/check-fmt-repovec-2-1-2.out
make lint 2>&1 | tee /tmp/lint-repovec-2-1-2.out
make test 2>&1 | tee /tmp/test-repovec-2-1-2.out
make markdownlint 2>&1 | tee /tmp/markdownlint-repovec-2-1-2.out
make nixie 2>&1 | tee /tmp/nixie-repovec-2-1-2.out
make verus 2>&1 | tee /tmp/verus-repovec-2-1-2.out
```

Stop after any failed command and inspect its log; the block is a sequence of
individual gates, not permission to continue after failure. `make lint`
includes rustdoc, Clippy, and Whitaker; `make test` includes the Python
runner's tests, Rust targets, and doctests. Preserve compiler admission
wrappers and the shared Cargo cache. Do not use `/tmp` for build output. The
documentation-only planning commit requires format, Markdown/spelling, and
diagram validation; it does not execute these future feature/proof milestones.

## Idempotence and recovery

Tests use isolated credential directories and local servers. No test rotates a
real operator token. Retrying a failed read or validation is safe; retrying an
uncertain refresh with the old pair is not. Persisted `Refreshing` state makes
that distinction survive crashes. Storage failure before intent means no
refresh request; failure after intent means no usable result until successful
new authorization and commit. A failed device login leaves the operation
terminal and requires explicit retry, avoiding prompt storms.

Do not overwrite unknown/corrupt credentials during normal load. Provide an
explicit application recovery operation
`recover_credential(runtime, request, present_prompt)` using the same lease. It
renames the encrypted credential to an exclusively created sibling backup,
syncs the directory, then starts one device flow; failure to preserve the file
prevents login. Never overwrite an existing backup. Test this through
`token-lifecycle-test --scenario corrupt-recovery`. The users guide must state
that production invocation awaits the future TUI/control endpoint; the example
is a local workflow demonstrator, not a supported production administration
command. This task must not claim production UI recovery is available. Missing
`systemd-creds` host key material remains a storage/configuration error:
restore the correct key or explicitly recover through the application
operation; do not mislabel it as revoked authentication or overwrite the
encrypted evidence. A rollback cannot restore GitHub's invalidated token pair.
Finish each commit with an inspected diff and passing applicable gates.

## Artefacts and notes

Wyvern reconnaissance used context pack `pk_m4xjknkh`; this plan includes the
necessary findings so the expiring pack is not a dependency for execution. The
community-of-experts team reviewed six perspectives: Pandalump and Wafflecat
(architecture and alternatives), Telefono and Dinolump (contracts and
maintainability), and Buzzy Bee and Doggylump (observability and recovery).
Their findings added durable rejection, validation on every reload, expiry
checks at publication, uncertain-write recovery, lease-owned writes, bounded
I/O, and journal failure-path evidence. The trade-off is deliberate: a single
lease may remain busy during device authorization, avoiding stale writers.
Separate persisted refresh and rejection states are retained because their
reasons and normal-process transitions differ, although both recover through
login. A follow-up contracts review found the substantive findings resolved.
Documentation-gate evidence is recorded in the draft pull request.

[^1]: GitHub,
      [Authorizing OAuth apps](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps),
    device-flow and refresh sections, retrieved through Firecrawl on
    2026-10-20. The device authorization lifetime is distinct from token
    lifetime; refresh rotation invalidates the old pair. Validate actual
    protocol fixtures against this source during implementation.

[^2]: [Googletest test attribute](https://docs.rs/googletest/latest/googletest/attr.test.html),
    compatibility example retrieved through Firecrawl on 2026-10-20.

[^3]: [rust-prover-tools](https://github.com/leynos/rust-prover-tools),
    install/run interface checked through Firecrawl on 2026-10-20. Pin the
    actual runner commit, Verus release, and archive checksum in EP-M1.

Revision note (2026-10-20): Initial plan records contracts and proof
obligations before implementation, with separate adapter and end-to-end
evidence.

Revision note (2026-10-20): Expert review tightened crash-safe retirement,
publication freshness, uncertain persistence, locking, and operator evidence.
Implementation remains approval-gated; no proof or feature work has begun.
