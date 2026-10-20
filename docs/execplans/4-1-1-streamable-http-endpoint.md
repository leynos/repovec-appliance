# Implement the Streamable HTTP endpoint

This ExecPlan is a living document. Its `Constraints`,
`Tolerances (exception triggers)`, `Risks`, `Progress`,
`Surprises & discoveries`, `Decision log`, `Outcomes & retrospective`,
`Conformance basis`, and `Verification plan` must be updated as the work
proceeds.

Status: DRAFT — awaiting explicit approval before implementation.

## Purpose / big picture

This change gives `repovec-mcpd` a single `/mcp` Streamable HTTP endpoint that
accepts Model Context Protocol (MCP) messages encoded with JSON Remote
Procedure Call (JSON-RPC) over POST and can open a Server-Sent Events (SSE)
stream over GET. A caller can observe negotiated JSON or SSE responses through
a real loopback HTTP client. The endpoint remains closed to real MCP work in
the shipped composition until authentication, origin policy, and a real backend
are wired by their owning roadmap items.

The plan pins MCP protocol revision 2025-11-25 because the requested GET
endpoint, sessions, and disconnect semantics are incompatible with the current
2026-07-28 revision. Approval of this plan must explicitly accept the pin and
the fail-closed delivery boundary recorded in
[`docs/adr-002-mcp-transport-baseline.md`](../adr-002-mcp-transport-baseline.md).
This milestone provides transport foundations; it does not complete public,
authenticated corpus access.

## Constraints

- Implement only roadmap item 4.1.1: one `/mcp` endpoint, GET and POST, and
  `application/json` or `text/event-stream` response modes. Keep session
  creation and lifecycle in 4.1.2, Origin policy in 4.1.3, and grepai stdio
  bridging in 4.2.
- Use MCP 2025-11-25 wire semantics, including the specified fallback to
  2025-03-26 only when `MCP-Protocol-Version` and negotiated context are
  absent. Accept explicit 2025-03-26 and 2025-11-25 for this transport subset;
  reject other versions. Do not claim compatibility with 2026-07-28.
- Keep HTTP framework types in the inbound adapter. Express transport and
  response-routing contracts behind application-owned types or ports; the
  domain/core must not depend on Axum or Hyper.
- Keep production composition fail-closed. Without accepted authentication,
  origin policy, and backend dependencies, the daemon must not accept MCP work
  or report a successful JSON-RPC response. A loopback listener may report
  unavailable service (503); test composition may inject fake dependencies.
- Preserve the current startup contract: run synchronous startup checks before
  constructing the async serving runtime. Do not nest runtimes or weaken
  existing startup tests.
- Never route responses through a shared anonymous broadcast channel. Each
  request and GET stream has an injected, explicit owner. A backend reply with
  no matching request owner is an internal protocol error and must not be
  emitted to another caller.
- Bound request-body size, concurrent work, response buffering, and stream
  lifetime. Use deadlines and orderly shutdown. A client disconnect does not
  cancel POST work under the pinned 2025-11-25 contract.
- Do not add user-facing CLI/configuration or dependency APIs without updating
  the relevant design documents and recording the decision. Use caret
  dependency requirements. Do not edit generated `typos.toml` by hand.
- Do not introduce `assume`, `admit`, trusted proof bodies, or equivalent
  shortcuts. Do not mark roadmap item 4.1.1 done until implementation and its
  required documentation have been completed and validated.

## Tolerances (exception triggers)

- If implementation needs to change the pinned wire revision, the single
  endpoint contract, production fail-closed behaviour, or the 4.1.2/4.1.3/4.2
  ownership boundary, stop and update this plan and the ADR for explicit
  approval.
- If a new dependency is needed beyond an HTTP/SSE stack and the requested test
  dependencies, record its exact role, version, and alternatives in the ADR and
  stop for approval before adding it.
- If the official Rust MCP SDK cannot pass the pinned contract suite without
  pulling later session, subprocess, or security behaviour into this milestone,
  discard the spike and use the thin adapter path. Do not expand scope to make
  the SDK fit.
- The anticipated change scope is `crates/repovec-mcpd`, workspace
  `Cargo.toml`/`Cargo.lock`, `docs/roadmap.md`, this plan,
  `docs/repovec-mcpd-architecture.md`, the technical design, ADR 002, the
  developer guide, `docs/contents.md`, and the users guide. Other paths or a
  materially larger diff require explicit review before proceeding.
- Any unresolved ambiguity in normative status codes, GET stream ownership,
  response correlation, body limits, or shipped startup behaviour is a stop
  condition before implementation of that contract.

## Risks

- **Protocol drift — high severity, medium likelihood.** SDK defaults now target
  the newer protocol. Pin tests and inspect the SDK's version behaviour during
  the spike; do not infer conformance from compilation.
- **Premature exposure — high severity, medium likelihood.** A listener can
  accidentally become an unauthenticated query endpoint. Keep the shipped
  composition unavailable until all policies and the backend are explicitly
  injected; prove this over a loopback socket.
- **Cross-request response leakage — high severity, low likelihood.** Concurrent
  JSON-RPC requests can have equal IDs. Give each request its own response
  channel and test interleaved replies with duplicate IDs.
- **Resource exhaustion — high severity, medium likelihood.** Open streams,
  oversized bodies, or slow clients can consume unbounded resources. Define
  limits at the adapter boundary and test rejection, timeout, and shutdown.
- **Protocol/design mismatch — high severity, high likelihood.** The existing
  technical design referenced the transport without a version. ADR 002 and the
  linked design update propose resolving this by pinning 2025-11-25; this
  remains a proposal until the user approves the plan.
- **GET event ownership before sessions — medium severity, medium likelihood.**
  The 2025 contract permits GET-initiated server messages. Inject a stream
  source associated with the accepted connection in tests; do not simulate it
  with global broadcast state. Session identity and reconnect replay remain out
  of scope.

## Progress

- [x] (2026-10-20) Read repository instructions, ExecPlan template, roadmap,
  technical design, and finalized reconnaissance packs.
- [x] (2026-10-20) Verify current and pinned MCP transport text via Firecrawl;
  record protocol drift and Axum SSE as a bounded implementation candidate.
- [x] (2026-10-20) Draft requirements, abstract contracts, proof sketch,
  test strategy, and milestone boundaries.
- [ ] Receive explicit user approval of this plan, including the 2025-11-25
  protocol baseline and fail-closed delivery boundary.
- [x] (2026-10-20) Receive all six community-of-experts perspectives and
  revise lifecycle, resource, protocol, milestone, and documentation contracts.
- [x] (2026-10-20) Complete final expert re-review and reconcile the
  protocol-version summaries with the detailed contract.
- [x] (2026-10-20) Validate formatting and Markdown/spelling gates.
- [x] (2026-10-20) Validate Mermaid diagrams and the diff; documentation-only
  gates pass. Publication is recorded in the draft PR rather than feature
  completion status.
- [ ] After approval only: implement milestones EP-M1 through EP-M4 and keep
  this plan current at each stopping point.

## Surprises & discoveries

- **Observation:** The current MCP revision changes both the requested GET
  endpoint and session model. **Evidence:** The current specification says GET
  stream endpoints and protocol-level sessions were removed in 2026-07-28;
  revision 2025-11-25 defines GET SSE and session management. **Impact:** An
  unversioned interpretation would change roadmap scope. The proposed baseline
  is explicit and awaits approval in ADR 002.
- **Observation:** The daemon currently validates startup and exits; it has no
  listener or protocol handler. **Evidence:** `crates/repovec-mcpd/src/main.rs`
  calls `repovec_core::appliance::daemon_startup::run_daemon_startup()` and
  then returns. **Impact:** The first runtime change must preserve the
  synchronous startup check, then create one persistent async serving runtime;
  it must not nest runtimes.

## Decision log

- **Decision:** Propose MCP 2025-11-25 as the transport baseline, with absent
  version fallback to 2025-03-26 only without negotiated context. Accept
  explicit 2025-03-26 and 2025-11-25 for the documented transport subset.
  **Rationale:** It preserves the specified GET and later session requirements;
  current 2026-07-28 semantics remove both. This is a proposal, not an accepted
  product decision, until the user approves this plan. **Date/author:**
  2026-10-20, planning agent.
- **Decision:** Propose a thin Axum/Tokio HTTP/SSE adapter behind application
  ports; compare it with the official Rust MCP SDK in one bounded spike.
  **Rationale:** Axum exposes the needed endpoint and SSE primitives while
  keeping transport choices in the adapter. SDK support and defaults must be
  checked against the pinned protocol before selection. **Date/author:**
  2026-10-20, planning agent.
- **Decision:** Keep the shipped daemon fail-closed, while exposing the
  transport to real loopback end-to-end tests with injected fake backend and
  admission policies. **Rationale:** Authentication and Origin enforcement have
  separate roadmap owners, and the stdio backend belongs to item 4.2. A
  test-only fake exercises observable HTTP behaviour without presenting an
  unauthenticated production service as complete. **Date/author:** 2026-10-20,
  planning agent.
- **Decision:** Omit event IDs, replay, and the recommended empty priming
  event in this milestone. Do not accept `Last-Event-ID` as a replay promise;
  return 400 in the injected accepting composition when it is supplied.
  **Rationale:** A cursor without a retained event log would advertise recovery
  the transport cannot supply. This deliberate deviation from the 2025-11-25
  priming SHOULD is part of the approval request. SSE comments may keep a
  connection alive without introducing event IDs. Session/replay design is
  separate; session management alone does not imply replay support.
  **Date/author:** 2026-10-20, planning agent.
- **Decision:** No Verus milestone is proposed in the initial design.
  **Rationale:** The transport contracts are finite dispatch partitions and
  bounded request/response transitions. `rstest`, property tests, and real HTTP
  tests can exercise those contracts directly. If design review identifies a
  compact pure kernel with an unbounded property, stop, add its independent
  model and representation relation to this plan, and obtain approval before
  selecting Verus or adding proof plumbing. **Date/author:** 2026-10-20,
  planning agent.

## Outcomes & retrospective

Planning delivered as a draft for approval. Two Wyvern agents supplied
repository evidence; Pandalump, Wafflecat, Telefono, Dinolump, Buzzy Bee, and
Doggylump reviewed the design. Their revisions established green milestones,
precise protocol rules, owned bounded streams, and mandatory operator docs. No
code, dependency, roadmap status, or endpoint behaviour has been changed. At
completion, record the delivered endpoint behaviour, all accepted deviations,
gate results, and any remaining limitation here.

## Context and orientation

`crates/repovec-mcpd` is the daemon binary. Its current `main.rs` initializes
tracing, calls synchronous startup validation in `repovec-core`, and exits. The
implementation retains these startup checks, then creates one persistent async
runtime and a loopback listener at `127.0.0.1:8080`; it must not create a
nested runtime. The listener returns 503 while backend or security policies are
absent. Its manifest currently has no HTTP framework or SSE dependency.
`Cargo.toml` holds workspace dependency versions. The design and security
boundary are in `docs/repovec-appliance-technical-design.md`; adjacent roadmap
items separate session management (4.1.2), Origin policy (4.1.3), and the
grepai stdio bridge (4.2).

The HTTP handler is a driving adapter: it receives network requests, validates
and parses the transport envelope, then calls application-owned ports. An
application port is a Rust trait owned by the application boundary and
implemented by an infrastructure adapter. SSE is Server-Sent Events, a
text-based HTTP event stream. The endpoint's `/mcp` path and socket binding
should be injectable for tests, while the shipped default remains loopback
until the deployment/security work explicitly changes it.

Read these references before implementation:

- [Technical design: remote endpoint and MCP transport/security
  invariants](../repovec-appliance-technical-design.md).
- [ADR 002: MCP transport baseline](../adr-002-mcp-transport-baseline.md).
- [Roadmap items 4.1.1–4.2](../roadmap.md).
- [Rust testing with rstest fixtures](../rust-testing-with-rstest-fixtures.md).
- [Rust doctest and DRY guide](../rust-doctest-dry-guide.md), for any public
  Rust API examples (not for private handler behaviour).
- [Dependency-injection testing guide](../reliable-testing-in-rust-via-dependency-injection.md).
- [Complexity antipatterns and refactoring strategies](../complexity-antipatterns-and-refactoring-strategies.md).
- [Ortho-config user's guide](../ortho-config-users-guide.md), as the project
  precedent for observable CLI/service behaviour and configuration docs.
- [rstest-bdd user's guide](../rstest-bdd-users-guide.md).
- Skills: `codegraph-mcp`, `hexagonal-architecture`, `rust-router`,
  `execplans`, `firecrawl-mcp`, plus the routed HTTP/service, async,
  state-machine, testing, and verification references selected during
  implementation.

Protocol references checked through Firecrawl:

- [MCP 2025-11-25 transport specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports)
  defines one GET/POST endpoint; POST request responses as JSON or SSE; GET
  SSE; response correlation; and the distinction between disconnect and
  cancellation. It says an SSE stream SHOULD be primed for reconnect and allows
  clients to reconnect with `Last-Event-ID`.
- [MCP 2026-07-28 Streamable HTTP specification](https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http)
  removes GET stream endpoints and protocol sessions, and makes closing an SSE
  response stream cancellation. It is not the proposed target for this item.
- [Axum SSE response documentation](https://docs.rs/axum/latest/axum/response/sse/index.html)
  is candidate adapter documentation, not a reason to select an unverified
  dependency version.

## Conformance basis

- `docs/roadmap.md`, item 4.1.1: one endpoint supporting GET and POST; JSON
  single-response and SSE modes. Adjacent items 4.1.2, 4.1.3, and 4.2 remain
  explicitly out of scope.
- `docs/repovec-appliance-technical-design.md`, “MCP transport and security
  invariants” and “Bridging to grepai MCP tools”: transport and security
  architecture; current planned linkage to ADR 002.
- Repository baseline: `7763a99f5dff25547a972888ed26d6381c465bb9`; this
  identifies the inspected source tree, not a specification revision.
- `docs/adr-002-mcp-transport-baseline.md`, proposed decision: pin 2025-11-25,
  keep test dependencies injected, and keep shipping composition fail-closed.
- Governing standard: MCP Streamable HTTP 2025-11-25 at the dated specification
  URL linked above, with its specified 2025-03-26 fallback when the
  protocol-version header and negotiated context are absent. The 2026-07-28
  revision is documented as a non-target pending a separate architecture
  decision.
- No Terms of Reference or other ADR specific to transport exists beyond ADR
  002. Any new durable architectural choice must update that ADR and the
  technical design before the affected milestone is accepted.

Trace links:

```plaintext
ROADMAP-4.1.1 -> TDD-MCP-TRANSPORT -> ADR-002 -> EP-M1..EP-M4 -> HTTP contract tests
ROADMAP-4.1.2/4.1.3/4.2 -> explicit exclusions -> no session/origin/stdio implementation
```

## Verification plan

The abstract model is independent of Axum. A request is a method, headers,
body-length class, and optionally one JSON-RPC message. An admitted exchange
has an owner token mapped to one unique bounded receiver, a server-selected
response mode, a bounded lifetime, and messages delivered only to that owner.
The adapter refines this model by parsing HTTP into the abstract request,
calling one injected port, and encoding the result as JSON or SSE.

Invalid input remains in the total model and must be rejected; it is not
excluded by a proof precondition. For valid POST requests, one backend
operation is called and exactly one terminal response preserves the typed
JSON-RPC id. The POST SSE sequence may contain related server requests or
notifications before that terminal response; JSON mode contains only the
terminal response. Accepted notifications and JSON-RPC responses use a separate
one-way backend operation and return 202 with no body. GET returns an
owner-scoped SSE stream containing only server requests or notifications, never
JSON-RPC responses because replay is out of scope. A GET source cannot
broadcast to multiple owners.

The following status contract applies after admission unless a row explicitly
states denial. Closed production admission takes precedence over every other
`/mcp` outcome. In an injected open composition, validate Origin, authenticate,
validate method/headers, acquire capacity, then read the bounded body. Unknown
paths return 404; no second MCP route is introduced.

| Input or outcome                                                                                                    | HTTP result                                                                               |
| ------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| Admitted POST request whose backend completes in JSON mode                                                          | `200`, `application/json`, one response object                                            |
| Admitted POST request whose backend completes in SSE mode                                                           | `200`, `text/event-stream`, related messages then one terminal response                   |
| Accepted POST notification or JSON-RPC response                                                                     | `202`, empty body                                                                         |
| GET with an acceptable SSE header                                                                                   | `200`, owner-scoped `text/event-stream`                                                   |
| Unsupported method                                                                                                  | `405`, `Allow: GET, POST`                                                                 |
| `Accept` does not advertise both required POST media types, or GET omits SSE                                        | `406`                                                                                     |
| POST content type is missing, duplicated, or not `application/json`                                                 | `415`                                                                                     |
| Malformed JSON, invalid JSON-RPC envelope, batch, duplicate version header, or invalid/unsupported explicit version | `400`                                                                                     |
| POST body exceeds 1 MiB                                                                                             | `413`                                                                                     |
| Production admission is closed                                                                                      | `503` for every `/mcp` method, before body parsing, backend dispatch, or GET subscription |
| Injected authentication policy denies a request                                                                     | `401`, before backend dispatch                                                            |
| Injected Origin policy denies a request                                                                             | `403`, before backend dispatch                                                            |
| Global exchange permit is exhausted                                                                                 | `503` with `Retry-After: 1`                                                               |
| Backend deadline expires before headers commit                                                                      | `504`; close an already-started SSE response                                              |
| Backend protocol failure before headers commit                                                                      | `502`; close an already-started SSE response                                              |

Caption: Transport outcomes after the explicitly ordered admission checks.

POST clients advertise both `application/json` and `text/event-stream`, as
required by the pinned specification. The server's injected response policy
selects JSON or SSE; the request does not invent a preference header. GET
requires `text/event-stream`. Parse media types case-insensitively and honour
quality weights: `q=0` is not acceptance. Require explicit positive entries for
both POST types; wildcard-only offers receive 406. Accept an optional UTF-8
charset on JSON; reject other charset values. With no negotiated session
context, an absent `MCP-Protocol-Version` uses the 2025-03-26 fallback; accept
explicit 2025-03-26 and 2025-11-25 for the transport behaviours implemented
here, and reject other explicit values. This is not a claim of full feature
parity for either revision. Combine repeated `Accept` field lines as one HTTP
list. Reject duplicate `MCP-Protocol-Version` fields rather than choosing an
arbitrary value.

Validate JSON-RPC 2.0 envelopes before dispatch: a request has a string method
and a non-null string or integer ID; a notification has a string method and no
ID; a response has a valid request ID and exactly one of `result` or `error`.
Reject batches, null/float/array request IDs, malformed envelopes, and a
response containing both or neither of `result`/`error`. Allow `params` to be
absent, an object, or an array; reject present null or scalar values. Preserve
valid parameters as opaque JSON; keep `result` and error data opaque. Require
the exact `jsonrpc: "2.0"` value and validate an error object with integer
`code` and string `message`. Reject duplicate envelope keys and conflicting
method/result/error shapes. A wrong response ID, unsolicited response, early
backend close, or second terminal response is a backend protocol error and must
not be forwarded.

Use these bounded internal defaults unless a reviewed design change replaces
them: 1 MiB per POST body; 64 KiB per encoded event or final JSON response; a
16-event and 256 KiB per-exchange queue; 64 global POST exchanges; 16 global
GET streams; a 120-second POST deadline starting before body read; a 15-minute
GET stream lifetime; and a 10-second graceful shutdown period before explicit
task cancellation. Acquire the fail-closed admission gate and relevant permits
before parsing a body, calling a backend, or subscribing to a GET source. If a
POST client disconnects, drop only the HTTP delivery receiver. Keep its permit
while backend work completes or reaches its deadline, discarding undeliverable
messages. If a bounded event queue overflows, close delivery and drain/discard
until the deadline rather than cancelling the backend because the client
disconnected. A GET disconnect ends only that owner's source. Explicit deadline
and shutdown may cancel work. An oversized backend message is a protocol
failure (502 before headers, stream closure afterwards); never buffer the
entire oversized value just to reject it. Bound backend serialization with a
size-limited writer and require the port implementation to bound messages
before enqueueing. The server supervisor retains the task and permit
independently of the HTTP body. Use an injected clock for deadlines and await
task completion on shutdown. Verify delayed POST completion after disconnect,
permit retention and release, and no output leakage.

| Obligation                         | Statement and method                                                                                                                                                                                                                                                                                          | Artefact and evidence                                                                                                                                                                                                                                                                                                | Non-vacuity and limits                                                                                                                                                                                                                                                                           |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| V-1 Dispatch                       | Every method/header/version/content-type partition selects exactly one permitted handler or rejection. Use exhaustive `rstest` cases with `googletest` and `pretty_assertions`.                                                                                                                               | Unit tests in `crates/repovec-mcpd/src/transport/`; focused `cargo test -p repovec-mcpd transport`. Unsupported method, type, version, and malformed message each assert their status and no success body. POST clients advertise both response media types; an injected server policy chooses the JSON or SSE case. | Include one witness per success and error partition, including missing version fallback, empty/invalid content types, and explicit unsupported version. A representative mutation that swaps JSON/SSE response policy must fail. Finite headers are enumerated; parser internals are not proven. |
| V-2 Correlation and isolation      | Each owner token maps to one unique bounded receiver. Concurrent POST requests with equal IDs, concurrent GET streams, and interleaved GET/POST work cannot cross-route messages. Use async unit tests and proptest over bounded interleavings.                                                               | `crates/repovec-mcpd/src/transport/` routing tests; run focused tests under Tokio multi-thread runtime. Reverse two POST completions with equal IDs, open two GET owners, and interleave their events.                                                                                                               | Generator classifies both completion orders, duplicate/non-duplicate IDs, and multiple GET owners with nonzero samples. A global ID map or broadcast mutation must fail. This checks repository routing, not Tokio internals.                                                                    |
| V-3 Wire representation            | JSON mode is one complete JSON object with the original correlation id; SSE mode is valid UTF-8 event framing, with each data payload a valid JSON-RPC message and final response last. Use `rstest`, `insta` snapshots for normalized JSON/SSE variants, and proptest for event payload/framing round trips. | Unit encoder tests and snapshots colocated with transport code; `cargo test -p repovec-mcpd transport`. Add BDD assertions over a real loopback HTTP client.                                                                                                                                                         | Snapshots exclude credentials and normalize only nondeterministic headers; generators include empty event lists, multiline JSON serialization, Unicode, and multiple events. Mutation that drops a separator or response id must fail. Snapshot equality alone is not treated as proof.          |
| V-4 Bounds and lifecycle           | Oversized bodies, excess concurrent exchanges, slow/non-reading clients, timed-out work, and shutdown cannot cause unbounded buffering or false success. A disconnected POST retains its permit until backend completion/deadline; delivery closes and undeliverable events are dropped.                      | Adapter tests and `crates/repovec-mcpd/tests/streamable_http.rs`; `cargo test -p repovec-mcpd`. Observe limit response, stream closure, permit release, and clean task completion.                                                                                                                                   | Exercise below/at/above each stated limit, a stalled stream, a non-reading client, and delayed completion after disconnect. A seeded limit-removal mutation must fail. These bounds do not prove resource use under arbitrary load.                                                              |
| V-5 Production gate                | Admission runs before body parsing, backend calls, or GET subscription. Shipped composition accepts no request as successful until backend, authentication, and Origin policy are supplied.                                                                                                                   | `crates/repovec-mcpd/tests/streamable_http.rs`; request `/mcp`, observe 503, and assert fake backend/event-source call counts remain zero. Injected test composition returns protocol responses.                                                                                                                     | Demonstrate inhabited closed and injected open compositions. A parse/dispatch-before-denial mutation must fail. Socket tests exercise repository composition; TLS termination remains external.                                                                                                  |
| V-6 Contract examples              | The observable endpoint supports JSON POST, SSE POST, GET SSE, unsupported methods, and protocol errors with pinned headers and status. Use `rstest-bdd` scenarios backed by real loopback HTTP and fake backend/admission ports.                                                                             | `crates/repovec-mcpd/tests/features/streamable_http.feature` and matching steps; run `cargo test -p repovec-mcpd`.                                                                                                                                                                                                   | Keep scenarios for valid JSON response, valid SSE response, GET event delivery, unsupported method/version, body limit, and fail-closed composition. Each must fail for the intended absent behaviour before implementation.                                                                     |
| V-7 Message classes and disconnect | POST SSE may carry related server requests/notifications before one terminal correlated response; JSON mode has only the response. GET carries only owner-scoped server requests/notifications, never responses. POST disconnect does not cancel backend work.                                                | Async and loopback tests: delayed backend completion after client drop; two GET streams; interleaved GET and POST messages; invalid GET response, duplicate/wrong terminal ID, unsolicited response, and early backend close.                                                                                        | Each class has a reachable witness. Mutations that broadcast, send a response on GET, cancel POST on receiver drop, or emit a wrong/second response must fail.                                                                                                                                   |

Candidate semantic lemmas, stated as proof-sketch hypotheses rather than
assumed truths:

- **L-1 Partition completeness:** every admissible method and media-type
  combination is either accepted by exactly one branch or rejected; no input
  can fall through to a success default.
- **L-2 Owner preservation:** the owner token maps to one unique bounded
  receiver, and encoding/delivery preserve that map independent of interleaving
  or equal typed JSON-RPC IDs.
- **L-3 Correlation preservation:** serialization retains the admitted
  JSON-RPC request ID for its final response.
- **L-4 Boundedness:** each exchange consumes no more than the body, per-event,
  per-exchange queue, global concurrency, and lifetime bounds. A disconnected
  POST drops delivery but retains its permit until completion or deadline.

Caption: Verification obligations and their executable evidence.

No Verus proof is currently selected: these obligations concern finite
dispatch, bounded async integration, or framework/network behaviour, and the
required claims can be tested at their executable boundaries. If a pure,
unbounded routing kernel emerges, stop before adding it; specify its abstract
state, representation relation, lemmas, trusted assumptions, and mutation
controls here, then propose an explicit EP-P proof milestone for approval. That
conditional milestone must pin the verifier in `tools/verus/VERSION` and
`tools/verus/SHA256SUMS`, use `prover-tools verus install --repo-root .`, and
pass `prover-tools verus run --repo-root . --proof-file verus/mcp_transport.rs`
in addition to `make check-fmt`, `make lint`, and `make test`. Prove the
selected semantic property for all admissible inputs with substantive lemmas
and a refinement relationship to the executable kernel; a disconnected model is
insufficient. Do not add proof files, a stub gate, or speculative solver
plumbing in the baseline design. Tests remain necessary for HTTP, SSE framing,
runtime wiring, and the fail-closed composition even if a later proof
discharges a semantic lemma. No unproved theorem may be made to compile with
assumptions or stubs.

External axioms are limited to the pinned MCP specification's normative
contract; the documented Axum/Tokio APIs; serde's JSON serialization contract;
and the operating system's loopback socket behaviour. Do not attempt to prove
third-party internals. Test the repository-owned adapter against those actual
interfaces. Required evidence includes non-empty cases for every dispatch
class, reachable duplicate-ID interleavings, and a negative mutation or seeded
fault for each routing/framing/bounds obligation. If a tool cannot support the
planned negative control, document an independent counterexample before
accepting the obligation.

## Plan of work

The work is test-first. Each implementation stage starts with the smallest
failing contract test, makes the smallest change that turns it green, then
refactors while retaining the same focused evidence. `repovec-mcpd` remains a
small daemon crate with a `src/lib.rs` target for integration testing; keep
protocol rules in a cohesive transport module and keep HTTP request/response
types at the adapter edge.

Use stable scenario/test names and injected clocks; never mutate the parent
process environment. Preserve existing startup-failure binary tests and test
the new serve handoff through the same production composition function with
injected startup checks and an ephemeral listener. A test-only executable may
exercise this function in a child process; no bypass switch belongs in the
shipping binary. Also assert the real binary invokes this composition after
successful validation through the existing startup seam.

The first approved milestone compares the official Rust SDK with a thin Axum
adapter using the same pinned black-box contract cases. It may inspect
dependencies and run a disposable prototype; it may not select new protocol
semantics or introduce production exposure. Retain only the option that passes
the contract cases and respects the ownership/security boundaries, then record
the exact crate version and features in ADR 002 before dependency changes.

Implement the transport in separable modules such as `transport::contract`,
`transport::http`, and `transport::sse` where the existing crate structure
supports them. Define an application-owned backend port for one accepted MCP
message and a stream source for GET events; do not expose Axum types in the
port. A backend request port returns a bounded sequence that may contain
related server requests/notifications and exactly one terminal correlated
response. A separate one-way port accepts notifications/responses and cannot
emit a terminal reply. A GET source returns only server requests/notifications;
the type should make JSON-RPC responses unrepresentable there. Give each
POST/GET exchange an owner token and unique bounded receiver. Set the limits
below as explicit internal configuration with test injection. Do not add a user
CLI or public configuration surface in 4.1.1.

Keep these numeric defaults unless the approved design review records a reason
to change them: loopback bind `127.0.0.1:8080`; 1 MiB POST body; 64 KiB encoded
event; 16 events and 256 KiB per exchange; 64 concurrent POSTs; 16 concurrent
GET streams; 120-second POST deadline; 15-minute GET lifetime; 10-second
graceful shutdown. The test composition binds an ephemeral loopback port. These
limits are internal constants/configuration and do not add operator-facing
options in this item.

Add one BDD feature specification before production code:

```gherkin
Feature: MCP Streamable HTTP transport
  Scenario: Return one JSON response for a POST request
    Given the MCP test server has an accepting fake backend
    And the client accepts application/json and text/event-stream
    When the client posts a valid MCP request to /mcp
    Then the response is one application/json object with the same request id

  Scenario: Stream a POST response as server-sent events
    Given the MCP test server has an accepting fake backend configured to use SSE
    And the client accepts application/json and text/event-stream
    When the client posts a valid MCP request
    Then the response contains ordered SSE data events ending in its response

  Scenario: Open the GET event stream
    Given the MCP test server has an owned event source
    And the client accepts text/event-stream
    When the client gets /mcp
    Then the response is text/event-stream with only that source's event

  Scenario: Keep concurrent GET event sources separate
    Given two MCP clients have different owned event sources
    When both clients get /mcp concurrently
    Then each client receives only its own server message

  Scenario: Finish backend work after a POST client disconnects
    Given the MCP test server has a delayed accepting backend
    When the client disconnects before its POST response arrives
    Then backend work completes or reaches its deadline without forwarding output

  Scenario: Keep the shipped composition closed
    Given the MCP daemon has no backend or accepted security policies
    When a client posts to /mcp on loopback
    Then the response is service unavailable with no successful MCP result
```

The BDD file is the observable specification. Add finite error and edge cases in
`rstest` tables, snapshots only for stable multivariant wire output, and
properties for framing/interleaving relations that hold across generated
inputs. Add real loopback end-to-end tests because this change establishes an
HTTP network boundary. Do not use external grepai, TLS, or a public service in
these tests.

Update `docs/repovec-appliance-technical-design.md` and ADR 002 with the final
adapter/port, version, startup, listener, limits, and the accepted deviation
from 2025-11-25 reconnect guidance. Add `docs/repovec-mcpd-architecture.md` to
document the internal port, ownership, and reuse rules. Update
`docs/developers-guide.md` with the adapter and test-injection convention.
Update `docs/users-guide.md` because the daemon changes from startup-and-exit
to a persistent loopback service returning 503 while disabled; state the bind
address, status behaviour, startup/shutdown behaviour, protocol pin, and that
public authenticated access is not yet available. Keep public Rust API doctests
only if the implementation exposes a public API, and follow the doctest guide.
Mark roadmap item 4.1.1 done only in the completed implementation milestone,
after documentation and all gates pass.

## Milestones and plateaus

### EP-M1 — protocol and adapter feasibility

**Requirements and gaps:** ROADMAP-4.1.1 and ADR-002 protocol/tooling decision.
**End state:** Inspect the official SDK first; discard it without a prototype
if its documented interfaces already contradict the required boundary.
Otherwise compare the minimum SDK and Axum prototypes using the same contract
cases. Retain one path and record exact versions/features in ADR 002. Keep
prototypes in repository scratch space, never a `/tmp` build target, and remove
them before the milestone commit. Run `make check-fmt`, `make lint`, and
`make test` sequentially; the retained repository must be green. **Acceptance
evidence:** Protocol contract cases establish POST JSON, POST SSE, GET SSE, 202
handling, invalid version, and unsupported media/method behaviour; the chosen
path passes every in-scope case. **Conformance check:** Requirements match the
pinned revision; design boundaries remain intact; 2025-11-25 remains the
target; no new public interface, dependency, trust boundary, or persisted
format is introduced; trace links are current. **Recovery:** Delete the
disposable prototype and repeat the comparison from the contract cases.
**Remaining gaps:** No shipped listener or application backend. **Compatibility
decision:** none.

### EP-M2 — tested transport contracts

**Requirements and gaps:** Dispatch, envelope validation, response mode,
correlation, and bounded message ownership for ROADMAP-4.1.1. **End state:**
Add failing unit tests, capture the intended red evidence, then implement the
pure contract types and validators that make them pass. Introduce `src/lib.rs`
and the application ports together with their tested consumers. Do not add
endpoint BDD bindings until EP-M3 can make them pass. No ignored,
expected-failure, staged-away, or restored failing tests are permitted at this
milestone boundary. Run `make check-fmt`, `make lint`, and `make test`
sequentially and commit only the green plateau. **Acceptance evidence:** The
focused contract suite first fails on missing validation behaviour, then passes
for both success and rejection partitions. **Conformance check:** Recheck
ROADMAP-4.1.1 clauses, domain purity, selected adapter, upstream assumptions,
dependency changes, trust boundary, and trace links. Ports are owned by the
daemon application, not its HTTP adapter. **Recovery:** Revert the complete
milestone commit if necessary; preserve red and green transcripts in this plan.
**Remaining gaps:** Executable HTTP adapter and BDD/socket evidence.
**Compatibility decision:** none.

### EP-M3 — HTTP/SSE adapter and bounded routing

**Requirements and gaps:** One `/mcp` GET/POST endpoint, JSON/SSE modes,
correlation, framing, bounds, and runtime wiring. **End state:** Injected test
composition serves JSON POST, SSE POST, and GET SSE over loopback; production
composition fails closed. Each exchange owns bounded response delivery. The SSE
stream has no replay or session generation; the ADR records the accepted
temporary gap against 2025-11-25 reconnection recommendations. Run
`make check-fmt`, `make lint`, and `make test` sequentially after the
milestone. The daemon's startup checks still run synchronously before the serve
runtime starts. **Acceptance evidence:** BDD and socket tests observe expected
headers, status, content type, complete JSON/SSE frames, correlation under
duplicate IDs, resource-limit failures, closed production composition, and
clean shutdown. **Conformance check:** Confirm versioned wire contract; no
session, Origin, or stdio implementation leaked into this milestone; no
unapproved public API, dependency, trust boundary, or persisted format; keep
trace links current. **Recovery:** Revert adapter wiring while retaining tests
and port contracts; remove disposable dependencies only when not selected.
**Remaining gaps:** Authentication, strict Origin allowlist, session
lifecycle/replay, grepai bridge, HTTPS deployment, and remote corpus queries.
**Compatibility decision:** none.

### EP-M4 — documentation and acceptance

**Requirements and gaps:** Design traceability and complete observable
acceptance for ROADMAP-4.1.1. **End state:** Design/ADR/developer documentation
reflects implementation; user guide documents the persistent listener and
unavailable response; roadmap item 4.1.1 is marked done only after all
implementation evidence passes. Run `make fmt`, `make markdownlint`, and
`make nixie` sequentially after doc changes. Then run `make check-fmt`,
`make lint`, and `make test` sequentially. **Acceptance evidence:** All gate
transcripts are recorded in this plan, the roadmap checkbox and design are
accurate, and a loopback client can exercise the injected transport while the
shipped composition remains closed. **Conformance check:** Reconcile
discoveries against roadmap, technical design, ADR 002, and the pinned
specification; verify all trace links and boundaries. **Recovery:** Keep the
implementation complete but leave roadmap unchecked if any required gate or
upstream update fails; correct the evidence and rerun the affected sequential
gates. **Remaining gaps:** Explicitly list the later roadmap items above.
**Compatibility decision:** none.

## Concrete steps

Run repository commands from the root of the candidate worktree. Do not start
implementation commands before this plan is explicitly approved. Delegate gates
to the `scrutineer` agent, the exclusive full-gate runner for this candidate.
Run formatting before format checks, lint, and tests; never overlap gates. At
each major implementation milestone, run the three project code gates
sequentially in this order; do not run them concurrently:

```sh
set -o pipefail
make check-fmt 2>&1 | tee /tmp/repovec-check-fmt-4-1-1.out
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

Focused commands should follow the selected adapter's test layout. The
anticipated focused unit command is `cargo test -p repovec-mcpd transport`; the
full `make test` target is authoritative. Do not invent a passing test count:
record actual counts and concise output after execution. Expected acceptance
transcripts will include examples such as:

```plaintext
POST /mcp Accept: application/json, text/event-stream -> JSON policy: 200 application/json
POST /mcp Accept: application/json, text/event-stream -> SSE policy: 200 text/event-stream
GET /mcp Accept: text/event-stream -> 200 Content-Type: text/event-stream
production composition without policies/backend -> 503, no MCP success body
```

Before implementation, verify the current workspace dependency versions and
test support, use CodeGraph for relevant call paths, and compare SDK/Axum
behaviour against the contract cases. Use the requested community-of-experts
review before user approval; incorporate review findings into this plan and
record material decisions before asking for approval.

## Validation and acceptance

The plan itself is accepted when the user explicitly confirms the DRAFT and the
proposed protocol/security boundaries. That confirmation is not implied by
silence. After approval, each milestone must pass its focused acceptance and
the applicable sequential gates before work proceeds.

The completed feature is accepted when an injected real HTTP server accepts
POST JSON and POST SSE, GET returns a live SSE stream from its injected owner,
unsupported inputs produce protocol-appropriate failures, duplicate IDs do not
cross-route concurrent responses, resource limits and shutdown are observable,
and the shipped binary refuses successful MCP work until its real backend and
security policies are wired. The selected protocol revision and limitations
must be stated accurately in the design and ADR. No Verus command is required
unless a later approved plan revision selects a pure unbounded proof.

## Idempotence and recovery

Contract tests and loopback servers use ephemeral ports and injected
dependencies, so repeated test runs create no persistent session or event
state. Keep the selected dependency and lockfile change only after its spike
passes the pinned contract. If a test process stops midway, rerun the focused
command; do not reuse a stale listener or event log. If a milestone fails, keep
the last coherent commit and plan evidence, correct the smallest failed
contract, and rerun that milestone's gates sequentially.

## Artefacts and notes

Commit each coherent green milestone after its gates; never commit a red
intermediate state. Record conformance evidence and rollback commit IDs here.

Expected implementation artefacts include transport unit tests, the BDD feature
and step bindings, stable normalized snapshots for the JSON/SSE variants, and a
loopback end-to-end test. Exact module paths may be refined after the SDK/Axum
spike, but that choice must not change port ownership or protocol contracts.
Planning validation (2026-10-20): `make fmt`, `make check-fmt`,
`make markdownlint` (including spelling), `make nixie`, and `git diff --check`
passed. The spelling generator left `typos.toml` unchanged. Runtime lint and
tests are reserved for implementation milestones because this change is
documentation-only. Record later command output and deviations here.

## Interfaces and dependencies

Create `crates/repovec-mcpd/src/application.rs` for port ownership and
`src/transport/{contract,http,sse}.rs` for the adapter. The application has no
Axum, Hyper, or socket types. Use these conceptual signatures, refining the
async spelling to the selected stack without changing their contracts:

```rust,no_run
trait McpBackend {
    async fn request(&self, owner: ExchangeOwner, request: RpcRequest,
        output: BoundedPostSink) -> Result<(), BackendError>;
    async fn accept(&self, owner: ExchangeOwner, message: OneWayMessage)
        -> Result<(), BackendError>;
}
trait EventSource {
    async fn subscribe(&self, owner: ExchangeOwner, output: BoundedGetSink)
        -> Result<(), BackendError>;
}
```

`ExchangeOwner` is an opaque connection/exchange token, distinct from the
JSON-RPC ID and from a future session ID. `OneWayMessage` is a notification or
client response. `BoundedPostSink` accepts related server requests,
notifications, and one final response; `BoundedGetSink` accepts only server
requests and notifications. Each sink consumes a delivery capability unique to
one receiver. The server supervisor owns the producer task and capacity permit;
HTTP body drops cannot drop that task. The runtime is an implementation detail
of these capabilities. `BackendError` is a semantic error enum, not an opaque
report. Admission returns either an owner capability or a typed denial.

JSON mode is selected only for exchanges whose backend contract promises one
terminal reply without interactive server requests; if an intermediate message
arrives, fail with 502 rather than silently losing it. SSE mode can carry the
full related sequence. The policy is injected at server construction and can
choose SSE conservatively; do not infer a method whitelist or interpret grepai
tool semantics. Test both policy settings with the same request fixture.

The selected HTTP adapter must support one `/mcp` route, request-body limits,
response streaming, timeouts, cancellation on server shutdown, and loopback
test binding. Axum's documented SSE response and `KeepAlive` facilities are a
candidate. Resolve the actual caret version and features from current workspace
and upstream documentation in EP-M1. `rstest`, `rstest-bdd`, `googletest`,
`pretty_assertions`, `insta`, and `proptest` are authorized test tools for the
specified cases; verify whether each already exists in the workspace and add
only what the tests require.

## Revision note

Initial draft (2026-10-20): added the approved-scope candidate, versioned
transport contract, verification sketch, test-first milestones, and explicit
approval gate. Expert review revisions corrected the green milestone rule, made
resource and lifecycle contracts concrete, and added mandatory operator
documentation. No implementation has started; documentation gates, publication,
and user approval remain.
