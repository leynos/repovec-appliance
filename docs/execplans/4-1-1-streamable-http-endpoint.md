# Implement the Streamable HTTP endpoint

This ExecPlan is a living document. Its `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be updated as the work
proceeds.

Status: DRAFT — awaiting explicit approval before implementation.

## Purpose / big picture

This change gives `repovec-mcpd` a single `/mcp` Streamable HTTP endpoint for
the Model Context Protocol (MCP). The endpoint accepts POST. It answers each
JSON Remote Procedure Call (JSON-RPC) request with one JSON object or with a
Server-Sent Events (SSE) stream scoped to that request. A caller can observe
both response modes, version negotiation, and the refusal of removed mechanisms
through a real loopback HTTP client.

The foundation is MCP revision 2026-07-28, which is stateless: there is no
protocol session, no standalone GET stream, and no stream replay. Clients that
speak an earlier Streamable HTTP revision are answered through `initialize` on
the same endpoint and served without sessions. The protocol is implemented by
RMCP, the official Rust Software Development Kit (SDK).

The endpoint remains closed to real MCP work in the shipped composition until
authentication, Origin policy, and a real backend are wired by their owning
roadmap items. Approval of this plan must explicitly accept the decisions in
[`docs/adrs/adr-0003-mcp-transport-baseline.md`](../adrs/adr-0003-mcp-transport-baseline.md)
and the direction in
[`docs/adrs/adr-0004-grepai-backend-bridge.md`](../adrs/adr-0004-grepai-backend-bridge.md).
This milestone provides transport foundations; it does not complete public,
authenticated corpus access.

An earlier draft of this plan pinned MCP 2025-11-25 and designed owned GET
streams, a documented replay gap, and work that outlived a disconnect. The
review of 2026-11-26 found that no RepoVec capability needs those mechanisms.
"Surprises & discoveries" and the "Decision log" record the evidence.

## Constraints

- Implement only roadmap item 4.1.1: one `/mcp` endpoint accepting POST;
  `application/json` and `text/event-stream` response modes; `server/discover`;
  and stateless compatibility with earlier Streamable HTTP revisions. Keep
  request admission limits, ownership tests, and cancellation propagation in
  4.1.2, Origin policy in 4.1.3, the grepai bridge in 4.2, and tokens in 4.3.
- Use MCP 2026-07-28 wire semantics as the foundation. Serve the earlier
  Streamable HTTP revisions that the verified RMCP release supports, from
  2025-03-26 to 2025-11-25, through `initialize` and without sessions. Do not
  offer the HTTP+SSE transport of 2024-11-05.
- Issue no `Mcp-Session-Id`, open no standalone stream, and offer no replay.
  GET and DELETE on `/mcp` return 405. Ignore a received `Mcp-Session-Id` or
  `Last-Event-ID` header.
- Advertise tools only. Do not advertise resources, prompts, list-change
  notifications, or the tasks extension.
- Use RMCP for the protocol and its Streamable HTTP service. Do not write a
  bespoke JSON-RPC, SSE, or version-negotiation implementation. If RMCP fails a
  behaviour this plan requires, stop; do not patch around it.
- Keep HTTP, Tower, and Axum types in the inbound adapter. Application ports
  may use RMCP's protocol model types, which ADR 0003 treats as the shared MCP
  vocabulary, but no transport type.
- Keep production composition fail-closed. Without accepted authentication,
  Origin policy, and backend dependencies, the daemon must not accept MCP work
  or report a successful JSON-RPC response. A loopback listener may report
  unavailable service (503); test composition may inject fake dependencies. The
  shipping binary has no switch that opens the endpoint.
- Preserve the current startup contract: run synchronous startup checks before
  constructing the async serving runtime. Do not nest runtimes or weaken
  existing startup tests.
- Each response reaches only the caller whose request produced it. Do not
  introduce shared anonymous channels between requests.
- Assert wire behaviour with an HTTP client that is not RMCP's, so that an
  RMCP client and server cannot agree on a shared defect.
- Do not add user-facing CLI/configuration or dependency APIs without updating
  the relevant design documents and recording the decision. Use caret
  dependency requirements. Do not edit generated `typos.toml` by hand.
- Build and test on the LLVM backend that the repository gates use. Cranelift
  is out of scope: it remains an unsupported compile-time option, and this work
  must not spend effort on Cranelift compatibility beyond normal Rust code
  hygiene. Do not choose, reject, or reshape a dependency, feature, or test
  because of its behaviour under Cranelift, and do not add a codegen backend to
  `.cargo/config.toml`.
- Follow the repository rules in `AGENTS.md`. Read no process environment
  outside a composition root, and never mutate the parent environment or
  working directory in a test; inject values through a seam as "Environment
  access policy" in `docs/developers-guide.md` requires. Keep each code file
  within 400 lines. `make lint` runs the Whitaker Dylint suite and the rustdoc
  lint table, so new modules and private items need documentation. Whitaker's
  `macro_aware_cognitive_complexity` holds every function to a complexity of 9
  (`dylint.toml`). Every `assert!`, `assert_eq!`, and `assert_ne!` needs a
  diagnostic message, and a lint may be relaxed only with an item-scoped
  `#[expect(lint, reason = "...")]`.
- Instrument request handling with `tracing` spans and structured fields, as
  the "Observability" section of `AGENTS.md` requires. Do not hold a span guard
  across an `.await`, do not install a global recorder or subscriber outside
  the binary's composition root, and keep request bodies, credentials, and
  unbounded values out of events and metric labels.
- Before adding a port, helper, or test fixture, sweep the workspace for
  equivalent behaviour, as the "Abstraction / port / helper policy" in
  `AGENTS.md` requires, and record the scope and reuse policy of anything new in
  `docs/repovec-mcpd-architecture.md`.
- Keep each test well inside the nextest budget in `.config/nextest.toml`: the
  default profile terminates a test after one 180-second period. A test that
  starts a nested Cargo or `rustc` build needs an exact-name override, the
  `nested-cargo-builds` test group, and a warm-up setup script in that file.
- ADRs live in `docs/adrs/` with four-digit numbers, as the documentation
  style guide requires.
- Do not introduce `assume`, `admit`, trusted proof bodies, or equivalent
  shortcuts. Do not mark roadmap item 4.1.1 done until implementation and its
  required documentation have been completed and validated.

## Tolerances (exception triggers)

- If implementation needs to change the protocol foundation, the legacy
  compatibility range, the single endpoint contract, production fail-closed
  behaviour, or the 4.1.2/4.1.3/4.2/4.3 ownership boundary, stop and update
  this plan and ADR 0003 for explicit approval.
- If RMCP fails any check in EP-M1, stop and report the failing check with its
  evidence. Do not start a bespoke adapter without renewed approval.
- The expected new dependencies are `rmcp` with its server and Streamable HTTP
  server features, `axum`, further `tokio` features, and one non-RMCP HTTP
  client for tests. If another dependency is needed, record its exact role,
  version, and alternatives in ADR 0003 and stop for approval before adding it.
- The anticipated change scope is `crates/repovec-mcpd`, workspace
  `Cargo.toml`/`Cargo.lock`, `docs/roadmap.md`, this plan,
  `docs/repovec-mcpd-architecture.md`, the technical design, ADR 0003, the
  developer guide, `docs/contents.md`, and the users guide. A loopback test
  client shared with other crates may also touch `crates/repovec-test-helpers`.
  Other paths or a materially larger diff require explicit review before
  proceeding.
- Any unresolved ambiguity in a status code, the legacy compatibility
  behaviour, or shipped startup behaviour is a stop condition before
  implementation of that contract.

## Risks

- **SDK conformance — high severity, medium likelihood.** This plan relies on
  RMCP behaviour taken from its documentation. EP-M1 observes each relied-upon
  behaviour over a real socket before any dependency is committed, and the
  contract tests keep observing it afterwards.
- **Premature exposure — high severity, medium likelihood.** A listener can
  accidentally become an unauthenticated query endpoint. Keep the shipped
  composition unavailable until all policies and the backend are explicitly
  injected; prove this over a loopback socket. RMCP's Origin validation is
  disabled by default, so the closed admission layer, not an RMCP default, is
  the guarantee.
- **Legacy client expectations — medium severity, medium likelihood.** A
  legacy client may attempt a GET stream or expect a session. It receives 405
  and no session header. RepoVec emits nothing such a stream would carry, but a
  strict client could treat the refusal as fatal. Test with RMCP's client in
  its legacy lifecycle, and record any real client found to fail.
- **Specification movement — medium severity, high likelihood.** The next MCP
  revision is targeted for 2027-03-30. Advertise the smallest capability set
  and pin behaviour with contract tests, so that an RMCP upgrade which changes
  the wire fails a test.
- **Dependency weight — low severity, medium likelihood.** RMCP brings
  authentication, client, and macro features this work does not need. Enable
  only the server and Streamable HTTP server features, and record the resulting
  additions to `Cargo.lock`.
- **Cross-request leakage — high severity, low likelihood.** Concurrent
  JSON-RPC requests can carry equal IDs. Each POST has its own response in the
  stateless model; one loopback test with equal IDs guards the property here,
  and 4.1.2 owns the fuller isolation evidence.

## Progress

- [x] (2026-10-20) Read repository instructions, ExecPlan template, roadmap,
  technical design, and finalized reconnaissance packs.
- [x] (2026-10-20) Draft the first plan, pinned to MCP 2025-11-25, and take it
  through six expert reviews.
- [x] (2026-10-20) Validate formatting, Markdown, spelling, and Mermaid gates.
- [x] (2026-11-18) Rebase onto `main` at `df8c445` with no conflicts. Record
  the Cranelift exclusion, the newer repository rules, and `make typecheck` as
  a milestone gate.
- [x] (2026-11-24) Rebase onto `main` at `e8ed310` with no conflicts. Renumber
  the ADR to 0003 and move it to `docs/adrs/`. Recheck every observation,
  reference, and blocker against that tree.
- [x] (2026-11-26) Reopen the architectural decision. Collect evidence on the
  2026-07-28 revision, client support, RMCP, and grepai through Firecrawl.
  Rewrite ADR 0003, add ADR 0004, and revise the technical design, the roadmap,
  and this plan around a stateless foundation.
- [ ] Receive explicit user approval of this plan and ADR 0003, including the
  stateless foundation, RMCP, legacy cancellation on disconnect, and the
  fail-closed delivery boundary.
- [ ] After approval only: implement milestones EP-M1 through EP-M4 and keep
  this plan current at each stopping point.

## Surprises & discoveries

- **Observation:** The 2026-07-28 revision replaces the mechanisms the first
  draft preserved. **Evidence:** The
  [changelog](https://modelcontextprotocol.io/specification/2026-07-28/changelog)
  removes `Mcp-Session-Id`, the `initialize` handshake, the GET endpoint, and
  `Last-Event-ID` replay; it adds per-request metadata, `server/discover`,
  `subscriptions/listen`, and Multi Round-Trip Requests. **Impact:** The first
  draft's reason for pinning 2025-11-25 was the roadmap's wording, not a
  product need. The roadmap has been revised.
- **Observation:** RepoVec needs none of the removed mechanisms. **Evidence:**
  Its tools are read-only queries. It sends no unsolicited notifications and no
  server-to-client requests, and the only use of a session was to key a grepai
  subprocess, which ADR 0004 replaces. **Impact:** Owned GET streams, the
  replay gap, and work that outlives a disconnect leave this plan.
- **Observation:** A dual-era server is specified and necessary.
  **Evidence:** The
  [versioning specification](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)
  permits a server to implement both, and that legacy clients "have no
  fall-forward mechanism". A comment of 2026-10-04 on
  [openai/codex#37417](https://github.com/openai/codex/issues/37417) reports
  Codex 0.155 to 0.160 negotiating 2025-06-18. **Impact:** Serving only
  2026-07-28 would refuse clients in current use, so legacy negotiation is in
  scope. The compatibility range starts below 2025-11-25.
- **Observation:** Some claims in the review brief could not be confirmed.
  **Evidence:** No Codex issue opened on 2026-11-20 about modern tool-list
  subscriptions was found; #37417 was opened on 2026-08-07. Anthropic's
  [announcement](https://claude.com/blog/bringing-mcp-2026-07-28-to-claude) of
  2026-07-28 says support is "rolling out", and no later statement of Claude
  Code's status was found. **Impact:** The decision does not depend on either
  claim. Both eras are served, so it holds whichever clients have moved.
- **Observation:** RMCP documents the behaviour this plan needs.
  **Evidence:**
  [RMCP 3.11.0](https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.11.0)
  was released on 2026-11-17. Its README says it "serves the `2026-07-28`
  draft statelessly automatically: no `Mcp-Session-Id`, no standalone
  GET/DELETE stream, and no `Last-Event-ID` resumption", and that disabling
  `legacy_session_mode` serves legacy versions statelessly as well. Its
  `StreamableHttpService` "is a Tower service" that mounts on an Axum router.
  `StreamableHttpServerConfig` documents `max_request_body_bytes` with a 413
  response, `allowed_hosts` defaulting to loopback, `allowed_origins`
  defaulting to no validation, and `json_response`, which falls back to SSE
  when a handler emits a notification first. **Impact:** The bespoke adapter
  comparison in the first draft is replaced by verification of RMCP.
- **Observation:** grepai's `root` argument makes transparent proxying unsafe.
  **Evidence:** The grepai 0.37.0 changelog says project-scoped tools accept
  "an optional `root` parameter with an absolute project path", which "takes
  precedence over a server started with `--workspace`". **Impact:** Outside
  this item, but it shapes the port: the backend receives an authenticated
  caller with every call, so 4.2.2 can authorize arguments. See ADR 0004.
- **Observation:** The daemon currently validates startup and exits; it has no
  listener or protocol handler. **Evidence:** `crates/repovec-mcpd/src/main.rs`
  calls `repovec_core::appliance::daemon_startup::run_daemon_startup()` and
  then returns. Rechecked at `e8ed310`: the crate has no `src/lib.rs` and no
  `tests/` directory. **Impact:** The first runtime change must preserve the
  synchronous startup check, then create one persistent async serving runtime;
  it must not nest runtimes.
- **Observation:** `main` published a different ADR under the number this
  plan's ADR used. **Evidence:**
  `docs/adrs/adr-0002-shared-github-rest-traversal.md` landed with roadmap item
  2.2.1. **Impact:** This branch's ADR was renumbered to 0003 and moved to
  `docs/adrs/`. Open pull request #100 proposes unrelated ADRs numbered 003 to
  005 under the older naming; whichever branch merges second must renumber.
- **Observation:** The workspace lockfile already resolves much of the HTTP
  stack. **Evidence:** At `e8ed310`, `Cargo.lock` holds `axum` 0.8.9, `hyper`
  1.10.1, and `tokio-stream` 0.1.18, through `tonic` (from `qdrant-client`) and
  the `oauth2-test-server` development dependency. `repovec-mcpd` itself
  declares only `repovec-core` and `tracing-subscriber`, plus
  `repovec-test-helpers` for tests. **Impact:** EP-M1 records whether RMCP's
  HTTP stack unifies with these versions or compiles a second copy.
- **Observation:** The workspace already carries the async runtime and every
  authorized test tool. **Evidence:** The root `Cargo.toml` declares `tokio`
  1.48.0 with `macros`, `rt-multi-thread`, and `time`; `rstest` 0.28.0;
  `rstest-bdd` 0.5.0; `googletest` 0.14.4; `pretty_assertions` 1.4.1; `insta`
  1.47.2; `proptest` 1.11.0; and `trybuild` 1.0.117. **Impact:** These are
  manifest additions to `repovec-mcpd`, not new workspace dependencies. Any
  further `tokio` feature, such as `net` or `signal`, is a workspace manifest
  change to record in the ADR.
- **Observation:** A loopback HTTP test fixture exists, but it faces the other
  way. **Evidence:** `repovec_test_helpers::recording_http` is a scripted
  server that records requests from a blocking client. `repovecd` uses
  `reqwest` 0.12 with its blocking client, declared in that crate and not in
  the workspace table. **Impact:** The MCP tests need a client that drives this
  daemon's listener, including a streamed SSE body. Follow the fixture's
  conventions (ephemeral loopback port, origin injected into the code under
  test), and decide in EP-M1 whether a test client belongs in
  `repovec-test-helpers` or stays local to `repovec-mcpd`.
- **Observation:** No test exercises the `repovec-mcpd` binary. **Evidence:**
  The startup contract is tested in
  `crates/repovec-core/src/appliance/daemon_startup/` (`tests.rs` and
  `tests_live.rs`). **Impact:** The serve handoff needs its own evidence, and
  the startup tests in `repovec-core` must keep passing unchanged.

## Decision log

- **Decision:** Adopt MCP 2026-07-28 as the foundation and serve earlier
  Streamable HTTP revisions statelessly on the same endpoint. **Rationale:**
  The newer revision keeps every capability RepoVec uses and removes mechanisms
  it does not need; the versioning specification defines dual-era servers; and
  legacy clients remain in use. This is a proposal until the user approves ADR
  0003. **Date/author:** 2026-11-26, planning agent, on the user's review.
- **Decision:** Use RMCP's Streamable HTTP service on Axum, subject to the
  EP-M1 checks. **Rationale:** RMCP 3.11.0 documents both protocol eras,
  stateless serving, body limits, and Host and Origin validation. Writing the
  same protocol by hand would duplicate a maintained implementation.
  **Date/author:** 2026-11-26, planning agent.
- **Decision:** Treat a closed response stream as cancellation for both eras.
  **Rationale:** 2026-07-28 requires it. For legacy clients, 2025-11-25 says a
  disconnect should not be read as cancellation, but without replay the result
  cannot be delivered, and every tool is a read-only query. The deviation is
  recorded in ADR 0003 for approval. **Date/author:** 2026-11-26, planning
  agent.
- **Decision:** Move admission limits, ownership evidence, and cancellation
  propagation to roadmap item 4.1.2. **Rationale:** With sessions gone, 4.1.2
  was redefined as request ownership, admission, and cancellation. Keeping
  4.1.1 to the endpoint and its negotiation gives the smallest useful
  increment; the shipped composition stays closed throughout. **Date/author:**
  2026-11-26, planning agent.
- **Decision:** Allow RMCP's model types in application ports. **Rationale:**
  The backend bridge in 4.2 will be an RMCP client, so a second, owned model of
  tools and results would only be translated back. Transport types stay in the
  adapter. **Date/author:** 2026-11-26, planning agent.
- **Decision:** Keep the shipped daemon fail-closed, while exposing the
  transport to real loopback end-to-end tests with injected fake backend and
  admission policies. **Rationale:** Authentication and Origin enforcement have
  separate roadmap owners, and the grepai bridge belongs to item 4.2.
  **Date/author:** 2026-10-20, planning agent; reaffirmed 2026-11-26.
- **Decision:** No Verus milestone is proposed. **Rationale:** The contracts
  are finite negotiation and dispatch partitions over a third-party protocol
  implementation. `rstest` tables and real HTTP tests exercise them directly.
  **Date/author:** 2026-10-20, planning agent; reaffirmed 2026-11-26.
- **Decision:** Treat Cranelift compatibility as out of scope for this work.
  **Rationale:** The Cranelift experiment found that its compile-time gain was
  outweighed by a threefold increase in binary size and slower test execution.
  Cranelift is retained only as an unsupported compile-time option, and the
  experiment is due to be revisited in about six months (May 2027).
  **Date/author:** 2026-11-18, planning agent, on the user's instruction.
- **Superseded (2026-11-26):** The first draft proposed pinning MCP
  2025-11-25, comparing a thin Axum adapter with the SDK, owning a GET stream
  per connection, rejecting `Last-Event-ID` as a documented replay gap, and
  letting POST work outlive a disconnect. ADR 0003 records that proposal as
  Option A and the evidence against it.

## Outcomes & retrospective

Planning delivered as a draft for approval, then reopened on 2026-11-26 before
any implementation. The lesson recorded so far is that the first draft accepted
the roadmap's mechanisms as requirements and designed carefully around them;
the review asked which capabilities those mechanisms served and found none. No
code, dependency, or endpoint behaviour has been changed. At completion, record
the delivered endpoint behaviour, all accepted deviations, gate results, and
any remaining limitation here.

## Context and orientation

`crates/repovec-mcpd` is the daemon binary. Its current `main.rs` initializes
tracing, calls synchronous startup validation in `repovec-core`, and exits. The
implementation retains these startup checks, then creates one persistent async
runtime and a loopback listener at `127.0.0.1:8080`; it must not create a
nested runtime. The listener returns 503 while backend or security policies are
absent. Its manifest currently has no HTTP framework, no MCP dependency, and no
async runtime: `tokio` is a workspace dependency that this crate does not yet
use. `Cargo.toml` holds workspace dependency versions.

The terms used below:

- A _modern_ client speaks MCP 2026-07-28. Each of its requests carries the
  protocol version, client identity, and client capabilities, in headers and in
  the JSON-RPC `_meta` field. It may call `server/discover` first.
- A _legacy_ client speaks an earlier revision. It begins with an
  `initialize` request and expects a negotiated version in the reply.
- A _dual-era_ server answers both. RMCP gates each behaviour on the version
  a request negotiates.
- An _application port_ is a Rust trait owned by the daemon's application
  layer and implemented by an adapter or a test fake.
- _Admission_ is the decision, made before a request body is read, whether a
  request may proceed and on whose behalf.

Read these references before implementation:

- [Technical design: remote endpoint and MCP transport/security
  invariants](../repovec-appliance-technical-design.md).
- [ADR 0003: MCP transport baseline](../adrs/adr-0003-mcp-transport-baseline.md).
- [ADR 0004: Terminate MCP and manage grepai backends](../adrs/adr-0004-grepai-backend-bridge.md).
- [Roadmap items 4.1.1–4.2.3](../roadmap.md).
- [Rust testing with rstest fixtures](../rust-testing-with-rstest-fixtures.md).
- [Rust doctest and DRY guide](../rust-doctest-dry-guide.md), for any public
  Rust API examples (not for private handler behaviour).
- [Dependency-injection testing guide](../reliable-testing-in-rust-via-dependency-injection.md).
- [Complexity antipatterns and refactoring strategies](../complexity-antipatterns-and-refactoring-strategies.md).
- [rstest-bdd user's guide](../rstest-bdd-users-guide.md).
- Skills: `hexagonal-architecture`, `rust-router`, `execplans`,
  `firecrawl-mcp`, plus the routed web-service, async, testing, and
  verification references selected during implementation.

Protocol and library references, checked through Firecrawl on 2026-11-26:

- [MCP 2026-07-28 Streamable HTTP](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
  defines the POST endpoint, request metadata headers, response modes,
  cancellation by stream closure, and how a modern-only server answers legacy
  traffic.
- [MCP 2026-07-28 versioning](https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning)
  defines the era model, `server/discover`, and the compatibility matrix.
- [MCP 2025-11-25 transports](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
  defines the legacy behaviour that compatibility must not contradict.
- [RMCP README at 3.11.0](https://github.com/modelcontextprotocol/rust-sdk/blob/rmcp-v3.11.0/README.md)
  and
  [`StreamableHttpServerConfig`](https://docs.rs/rmcp/latest/rmcp/transport/streamable_http_server/tower/struct.StreamableHttpServerConfig.html)
  document the server transport this plan uses.

## Conformance basis

- `docs/roadmap.md`, item 4.1.1, as revised on 2026-11-26: one endpoint
  accepting POST; JSON single-response and SSE modes; `server/discover`; legacy
  clients served through `initialize` without sessions; GET and DELETE refused
  with 405; no `Mcp-Session-Id`. Items 4.1.2, 4.1.3, 4.2, and 4.3 remain
  explicitly out of scope.
- `docs/repovec-appliance-technical-design.md`, “MCP transport and security
  invariants” and “Bridging to grepai MCP tools”.
- `docs/adrs/adr-0003-mcp-transport-baseline.md`, proposed decision: modern
  stateless foundation, stateless legacy compatibility, RMCP, and a fail-closed
  shipping composition.
- `docs/adrs/adr-0004-grepai-backend-bridge.md`, proposed direction, for the
  shape of the backend port only.
- Repository baseline: `e8ed31007f784742620c8a73959c998964fae871`, inspected
  on 2026-11-24. This identifies the inspected source tree, not a specification
  revision.
- Governing standard: MCP Streamable HTTP 2026-07-28, with the earlier
  revisions' behaviour for clients that negotiate them.
- Any new durable architectural choice must update ADR 0003 and the technical
  design before the affected milestone is accepted.

Trace links:

```plaintext
ROADMAP-4.1.1 -> TDD-MCP-TRANSPORT -> ADR-0003 -> EP-M1..EP-M4 -> HTTP contract tests
ROADMAP-4.1.2/4.1.3/4.2/4.3 -> explicit exclusions -> no limits/origin/bridge/token work
```

## Verification plan

The contract has two owners. MCP defines the protocol outcomes, and RMCP
implements them; this plan pins them with tests so that a change in either is
noticed. RepoVec defines admission and the closed production composition, and
implements them in a layer placed in front of the RMCP service.

Admission runs first and reads no request body. Closed production admission
takes precedence over every other `/mcp` outcome. In an injected open
composition the order is Origin, then authentication, then the protocol.
Unknown paths return 404; no second MCP route is introduced.

| Input or outcome                                           | HTTP result                                                       | Owner         |
| ---------------------------------------------------------- | ----------------------------------------------------------------- | ------------- |
| Production admission is closed                             | `503` for every `/mcp` method, before the body is read            | RepoVec       |
| Injected Origin policy denies a request                    | `403`, before the backend is called                               | RepoVec, MCP  |
| Injected authentication policy denies a request            | `401`, before the backend is called                               | RepoVec       |
| GET or DELETE on `/mcp`                                    | `405`                                                             | MCP, ADR 0003 |
| Modern request whose backend replies without notifications | `200`, `application/json`, one result object                      | MCP           |
| Modern request whose backend emits progress first          | `200`, `text/event-stream`, notifications then one final response | MCP           |
| `server/discover`                                          | `200`, supported versions, tools capability, server identity      | MCP           |
| Modern request with a version the server does not support  | `400`, `UnsupportedProtocolVersionError` listing supported ones   | MCP           |
| Modern request whose header and `_meta` versions differ    | `400`, `HeaderMismatch` error                                     | MCP           |
| Modern request for a method the server does not implement  | `404`, JSON-RPC error `-32601`                                    | MCP           |
| Legacy `initialize`                                        | `200`, negotiated legacy version, no `Mcp-Session-Id` header      | ADR 0003      |
| Legacy tool call after `initialize`                        | `200`, served without a session                                   | ADR 0003      |
| Request carrying `Mcp-Session-Id` or `Last-Event-ID`       | The header is ignored; no session header is returned              | MCP           |
| Request body larger than the configured limit              | `413`                                                             | RMCP          |
| Any path other than `/mcp`                                 | `404`                                                             | RepoVec       |

Caption: Status contract for the endpoint, and who defines each row.

Rows owned by MCP cite the 2026-07-28 transport and versioning pages. EP-M1
observes every row against the real RMCP crate. If RMCP's behaviour differs
from a row that MCP defines, stop: that is a conformance defect to report
upstream and to record in ADR 0003, not a row to edit quietly. Behaviour the
specification leaves open, such as a missing `Accept` header, is observed in
EP-M1 and added to this table before EP-M2 begins.

| Obligation               | What must hold                                                                                                                                                                               | Evidence                                                                                  |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| V-1 Negotiation          | Every era, version, and method partition in the table above produces its stated result. No partition is left to an untested default.                                                         | `rstest` cases over a loopback socket with a non-RMCP client                              |
| V-2 No session artefacts | No response in any era carries `Mcp-Session-Id`. GET and DELETE never open a stream. A legacy client completes `initialize` and a tool call with no session.                                 | Loopback tests; an RMCP client in its `Initialize` lifecycle as an interoperability smoke |
| V-3 Wire representation  | JSON mode is one complete JSON object carrying the request's ID. SSE mode is valid event framing, each data payload a JSON-RPC message, ending with the one final response for that request. | Loopback tests; `insta` snapshots of normalized JSON and SSE bodies                       |
| V-4 Response ownership   | Two concurrent requests with equal JSON-RPC IDs each receive the result of their own call.                                                                                                   | One loopback test with a fake backend that returns a caller-specific marker               |
| V-5 Production gate      | The shipped composition answers 503 to every method without reading a body or calling a backend, and has no configuration that opens it.                                                     | Loopback test against the production composition function; a recording fake backend       |
| V-6 Contract examples    | The observable endpoint supports the scenarios listed under "Plan of work".                                                                                                                  | `rstest-bdd` scenarios backed by a real loopback server                                   |
| V-7 Body bound           | A body over the limit is refused with 413 whether it arrives with `Content-Length` or chunked.                                                                                               | Loopback tests with both encodings                                                        |
| V-8 Startup and shutdown | Startup checks run before the runtime starts; a failed check starts no listener. Shutdown stops accepting and ends open streams.                                                             | Composition-function tests with injected startup checks and an ephemeral listener         |

Caption: Verification obligations and their executable evidence.

Each obligation needs a plausible wrong implementation that its test rejects.
For V-2, a server left in RMCP's default legacy session mode returns a session
header. For V-5, a composition that consults the admission policy after the
body is read lets an oversized body reach the 413 path. For V-4, a fake that
ignores the caller returns the same marker twice. Record the seeded fault and
the failing output for each obligation in this plan.

No Verus proof is selected. These obligations concern finite negotiation
partitions and the behaviour of a third-party protocol implementation over a
network boundary, and tests at that boundary are the appropriate evidence. Do
not add proof files, a stub gate, or solver plumbing. The repository has no
`tools/verus/` or `verus/` directory at this baseline.

External axioms are limited to the MCP specification's normative contract; the
documented RMCP, Axum, and Tokio APIs; and the operating system's loopback
socket behaviour. Do not attempt to prove third-party internals. Test the
repository-owned composition against those actual interfaces.

## Plan of work

The work is test-first. Each implementation stage starts with the smallest
failing contract test, makes the smallest change that turns it green, then
refactors while retaining the same focused evidence. `repovec-mcpd` gains a
`src/lib.rs` target so that integration tests can drive the same composition
function the binary uses.

Use stable scenario and test names and injected clocks; never mutate the parent
process environment. Preserve the existing startup tests in
`crates/repovec-core/src/appliance/daemon_startup/`, and test the new serve
handoff through the production composition function with injected startup
checks and an ephemeral listener.

The structure has three parts:

- an application layer that owns the `ToolBackend` and `Admission` ports and a
  small RMCP server handler that advertises tools only and delegates to the
  backend port;
- an inbound adapter that builds RMCP's Streamable HTTP service with legacy
  session mode disabled, sets the body limit, places the admission layer in
  front of it, and mounts both at `/mcp` on an Axum router; and
- a composition root that runs the startup checks, builds the runtime, binds
  the listener, and wires either the closed production policies or, in tests,
  injected fakes.

Use these initial defaults unless EP-M1 gives a reason to change them: loopback
bind `127.0.0.1:8080`, and a 1 MiB POST body limit. Concurrency limits and
request deadlines belong to item 4.1.2.

The behavioural scenarios are:

```gherkin
Feature: Streamable HTTP endpoint

  Scenario: Answer a modern tool call with JSON
    Given the MCP test server has an accepting fake backend
    When a modern client posts a tool call
    Then the response is one JSON result with status 200

  Scenario: Stream progress before the result
    Given the MCP test server has a fake backend that reports progress
    When a modern client posts a tool call with a progress token
    Then the response is an event stream ending with the result

  Scenario: Describe the server before any other request
    Given the MCP test server has an accepting fake backend
    When a modern client posts server/discover
    Then the response lists the supported versions and the tools capability

  Scenario: Serve a legacy client without a session
    Given the MCP test server has an accepting fake backend
    When a legacy client initializes and then posts a tool call
    Then both responses succeed and neither carries a session header

  Scenario: Refuse the removed stream endpoint
    Given the MCP test server has an accepting fake backend
    When a client sends GET to the MCP endpoint
    Then the response status is 405

  Scenario: Keep the shipped daemon closed
    Given the MCP daemon has no backend or accepted security policies
    When a client posts a tool call
    Then the response status is 503 and no backend is called
```

Use `googletest` matchers, `pretty_assertions` for structured values, `rstest`
tables for the negotiation partitions, and `insta` snapshots only for stable
multivariant wire output with nondeterministic fields normalized. Do not use
external grepai, TLS, or a public service in these tests.

Update `docs/repovec-appliance-technical-design.md` and ADR 0003 with the
verified RMCP version and features, the listener, the body limit, and the
observed legacy behaviour. Add `docs/repovec-mcpd-architecture.md` to document
the ports, their ownership, and their reuse rules. Update
`docs/developers-guide.md` with the adapter and test-injection convention.
Update `docs/users-guide.md` because the daemon changes from startup-and-exit
to a persistent loopback service returning 503 while disabled; state the bind
address, status behaviour, startup and shutdown behaviour, the protocol
revisions served, and that public authenticated access is not yet available.
Mark roadmap item 4.1.1 done only in the completed implementation milestone,
after documentation and all gates pass.

## Milestones and plateaus

### EP-M1 — RMCP verification

**Requirements and gaps:** ROADMAP-4.1.1 and the ADR-0003 choice of RMCP. **End
state:** A disposable prototype in repository scratch space, never a `/tmp`
build target, mounts RMCP's Streamable HTTP service with a trivial handler, and
each check below is observed over a loopback socket with a non-RMCP client.
Record the exact crate version, features, and observations in ADR 0003, then
delete the prototype before the milestone commit. The checks:

1. A modern tool call succeeds, and `server/discover` reports the supported
   versions.
2. With `legacy_session_mode` disabled, `initialize` and a following tool
   call succeed for 2025-11-25, 2025-06-18, and 2025-03-26, and no response
   carries `Mcp-Session-Id`.
3. GET and DELETE return 405; `Mcp-Session-Id` and `Last-Event-ID` request
   headers change nothing.
4. Every row of the status contract that MCP or RMCP owns behaves as stated.
5. A handler that emits a notification before its result switches the
   response to SSE; one that does not replies with JSON.
6. Closing the response stream cancels the handler, for a modern and for a
   legacy request.
7. A Tower layer in front of the service can answer 503, 401, or 403 without
   the body being read, and can pass an authenticated caller to the handler.
8. A server handler can be written by hand, without RMCP's macros, and passes
   this repository's Clippy and Whitaker configuration.
9. RMCP's HTTP stack unifies with the `axum` and `hyper` versions already in
   `Cargo.lock`, or the cost of a second copy is recorded.

Run `make check-fmt`, `make typecheck`, `make lint`, and `make test`
sequentially; the retained repository must be green. **Acceptance evidence:** A
table in ADR 0003 giving the observed result of each check. **Conformance
check:** Requirements match ADR 0003; no public interface, trust boundary, or
persisted format is introduced; trace links are current. **Recovery:** Delete
the disposable prototype and repeat. If any check fails, stop and report it;
the tolerance above forbids a bespoke adapter without renewed approval.
**Remaining gaps:** No shipped listener or application backend. **Compatibility
decision:** none.

### EP-M2 — application ports and server handler

**Requirements and gaps:** The tool catalogue, tool calls, and capability
advertisement for ROADMAP-4.1.1. **End state:** Add failing tests, capture the
intended red evidence, then add `src/lib.rs`, the `ToolBackend` and `Admission`
ports, a fake backend for tests, and the RMCP server handler that advertises
tools only and delegates to the port. Add the `rmcp` dependency with the
features verified in EP-M1. No ignored, expected-failure, or staged-away tests
are permitted at this milestone boundary. Run `make check-fmt`,
`make typecheck`, `make lint`, and `make test` sequentially and commit only the
green plateau. **Acceptance evidence:** Handler tests show that the advertised
capabilities are tools only, that the catalogue and call results come from the
port, and that a backend error becomes an MCP error and never a fabricated
success. **Conformance check:** Recheck ROADMAP-4.1.1 clauses, port ownership,
dependency changes, and trace links. Ports are owned by the daemon application
and carry no transport type. **Recovery:** Revert the complete milestone commit
if necessary; preserve red and green transcripts in this plan. **Remaining
gaps:** Executable HTTP endpoint and socket evidence. **Compatibility
decision:** none.

### EP-M3 — HTTP endpoint, admission, and composition

**Requirements and gaps:** One `/mcp` endpoint, both response modes,
negotiation in both eras, the body bound, the closed production composition,
and runtime wiring. **End state:** The injected test composition serves the
status contract over loopback; the production composition answers 503. The
daemon's startup checks still run synchronously before the serve runtime
starts, and shutdown ends open streams. Run `make check-fmt`, `make typecheck`,
`make lint`, and `make test` sequentially after the milestone. **Acceptance
evidence:** Obligations V-1 to V-8 pass, each with its recorded seeded fault.
**Conformance check:** Confirm the wire contract in both eras; no session,
concurrency-limit, Origin-policy, bridge, or token implementation leaked into
this milestone; no unapproved public API, dependency, trust boundary, or
persisted format; keep trace links current. **Recovery:** Revert adapter wiring
while retaining tests and port contracts. **Remaining gaps:** Request limits
and cancellation propagation (4.1.2), Origin allowlist (4.1.3), grepai bridge
(4.2), tokens (4.3), HTTPS deployment, and remote corpus queries.
**Compatibility decision:** none.

### EP-M4 — documentation and acceptance

**Requirements and gaps:** Design traceability and complete observable
acceptance for ROADMAP-4.1.1. **End state:** Design, ADR, and developer
documentation reflects implementation; the users guide documents the persistent
listener and its unavailable response; roadmap item 4.1.1 is marked done only
after all implementation evidence passes. Run `make fmt`, `make markdownlint`,
and `make nixie` sequentially after documentation changes. Then run
`make check-fmt`, `make typecheck`, `make lint`, and `make test` sequentially.
**Acceptance evidence:** All gate transcripts are recorded in this plan, the
roadmap checkbox and design are accurate, and a loopback client can exercise
the injected transport while the shipped composition remains closed.
**Conformance check:** Reconcile discoveries against the roadmap, technical
design, ADR 0003, and the specification; verify all trace links and boundaries.
**Recovery:** Keep the implementation complete but leave the roadmap unchecked
if any required gate fails; correct the evidence and rerun the affected
sequential gates. **Remaining gaps:** Explicitly list the later roadmap items
above. **Compatibility decision:** none.

## Concrete steps

Run repository commands from the root of the candidate worktree. Do not start
implementation commands before this plan is explicitly approved. Delegate gates
to the `scrutineer` agent, the exclusive full-gate runner for this candidate.
Run formatting before format checks, lint, and tests; never overlap gates. At
each major implementation milestone, run the four project code gates
sequentially in this order; do not run them concurrently:

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/repovec-check-fmt-4-1-1.out
set -o pipefail
make typecheck 2>&1 | tee /tmp/repovec-typecheck-4-1-1.out
set -o pipefail
make lint 2>&1 | tee /tmp/repovec-lint-4-1-1.out
set -o pipefail
make test 2>&1 | tee /tmp/repovec-test-4-1-1.out
```

For documentation changes, also run in sequence:

```sh
set -o pipefail
make fmt 2>&1 | tee /tmp/repovec-fmt-4-1-1.out
set -o pipefail
make markdownlint 2>&1 | tee /tmp/repovec-markdownlint-4-1-1.out
set -o pipefail
make nixie 2>&1 | tee /tmp/repovec-nixie-4-1-1.out
```

The anticipated focused command is `cargo test -p repovec-mcpd`; the full
`make test` target is authoritative. Do not invent a passing test count: record
actual counts and concise output after execution. Expected acceptance
transcripts will include examples such as:

```plaintext
POST /mcp tools/call, 2026-07-28, backend replies directly -> 200 application/json
POST /mcp tools/call, 2026-07-28, backend reports progress -> 200 text/event-stream
POST /mcp server/discover                                  -> 200, tools capability only
POST /mcp initialize, 2025-11-25                           -> 200, no Mcp-Session-Id
GET  /mcp                                                  -> 405
production composition, any method                         -> 503, no backend call
```

## Validation and acceptance

Acceptance is behavioural. A loopback client that is not RMCP's can complete a
modern tool call in both response modes, call `server/discover`, complete a
legacy `initialize` and tool call without receiving a session, and be refused
on GET; and the shipped binary refuses MCP work until its real backend and
security policies are wired. An RMCP client completes one call in each of its
`Discover` and `Initialize` lifecycles as an interoperability smoke. The
protocol revisions served and the limitations must be stated accurately in the
design and ADR.

## Idempotence and recovery

Contract tests and loopback servers use ephemeral ports and injected
dependencies, so repeated test runs create no persistent state. Keep the `rmcp`
dependency and lockfile change only after EP-M1 passes. If a test process stops
midway, rerun the focused command; do not reuse a stale listener. If a
milestone fails, keep the last coherent commit and plan evidence, correct the
smallest failed contract, and rerun that milestone's gates sequentially.

## Artefacts and notes

Commit each coherent green milestone after its gates; never commit a red
intermediate state. Record conformance evidence and rollback commit IDs here.

Expected implementation artefacts include handler unit tests, the BDD feature
and step bindings, stable normalized snapshots for the JSON and SSE variants,
and a loopback end-to-end test. Exact module paths may be refined in EP-M1, but
that choice must not change port ownership or protocol contracts.

Planning validation (2026-10-20): `make fmt`, `make check-fmt`,
`make markdownlint` (including spelling), `make nixie`, and `git diff --check`
passed.

Rebase validation (2026-11-18 and 2026-11-24): the planning commits replayed
onto `main` at `df8c445` and then `e8ed310` without conflict.

Design review evidence (2026-11-26): sources were read through Firecrawl. The
MCP changelog, transport, and versioning pages, the RMCP release notes, README,
and configuration reference, the grepai changelog and `mcp-serve` reference,
and the MCP release-cycle announcement are linked from ADR 0003 and ADR 0004.
Nothing was executed against RMCP or grepai; EP-M1 and the ADR 0004 spike own
that evidence.

## Interfaces and dependencies

Create `crates/repovec-mcpd/src/application/` for the ports and server handler,
and `src/transport/` for the Axum and RMCP wiring. Use these conceptual
signatures, refining the async spelling and the RMCP type names to the verified
release without changing their contracts:

```rust,ignore
/// Supplies the tool catalogue and executes tool calls for one caller.
trait ToolBackend {
    async fn catalogue(&self, caller: &Caller) -> Result<Vec<Tool>, BackendError>;
    async fn call(&self, caller: &Caller, call: ToolCall, context: CallContext)
        -> Result<ToolOutcome, BackendError>;
}

/// Decides, before a body is read, whether a request may proceed.
trait Admission {
    fn admit(&self, head: &RequestHead) -> Result<Caller, Denial>;
}
```

`Tool`, `ToolCall`, and `ToolOutcome` stand for RMCP's tool, call-parameter,
and call-result model types. `Caller` is an opaque value produced only by
admission; it identifies who is asking and will carry the authorized corpus
scope once item 4.3 defines it. Every backend call receives it, which is what
lets item 4.2.2 authorize tool arguments. `CallContext` carries the request's
cancellation signal and a sink for progress notifications, so item 4.1.2 can
propagate cancellation without changing the port. `RequestHead` is an
application-owned view of the method, path, and headers; it is not an HTTP
framework type. `BackendError` and `Denial` are semantic error enums, not
opaque reports.

The production composition supplies an `Admission` that denies every request
with "service unavailable", and no backend. Tests supply an accepting admission
and a fake backend. There is no other way to open the endpoint.

`rstest`, `rstest-bdd`, `googletest`, `pretty_assertions`, `insta`, and
`proptest` are authorized test tools for the specified cases. All six are
already workspace dependencies; add to the `repovec-mcpd` manifest only what
the tests require.

## Revision note

Initial draft (2026-10-20): added the approved-scope candidate, versioned
transport contract, verification sketch, test-first milestones, and explicit
approval gate, pinned to MCP 2025-11-25.

Revision (2026-11-18): rebased onto `main`, recorded the Cranelift exclusion
and the newer repository rules, and added `make typecheck` to each milestone.

Revision (2026-11-24): rebased onto `main` at `e8ed310`; renumbered the ADR to
0003 under `docs/adrs/`; refreshed the repository baseline and observations.

Revision (2026-11-26): reopened the architectural decision on the user's
review. Replaced the 2025-11-25 pin with a 2026-07-28 stateless foundation and
stateless legacy compatibility; replaced the adapter comparison with
verification of RMCP; removed GET streams, the replay gap, and work that
outlives a disconnect; moved request limits and cancellation propagation to
item 4.1.2; and aligned the plan with ADR 0004 and the revised roadmap. The
blocker is unchanged in kind: this plan, ADR 0003, and ADR 0004 await explicit
approval, and no implementation has started.
