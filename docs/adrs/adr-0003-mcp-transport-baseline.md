# Architectural decision record (ADR) 0003: MCP transport baseline

Renumbered from 002 on 2026-11-24 (duplicate number).

## Status

Accepted on 2026-11-26. `repovec-mcpd` adopts MCP 2026-07-28 Streamable HTTP as
its stateless foundation, serves earlier Streamable HTTP revisions through
`initialize` without sessions, uses RMCP subject to the verification milestone,
treats a closed response stream as cancellation in both eras, and ships closed
until authentication, Origin policy, and a backend exist.

This decision replaces the first proposal, which pinned Model Context Protocol
(MCP) revision 2025-11-25. That proposal was never accepted; it is retained
below as Option A.

## Date

2026-11-26.

## Context and problem statement

Roadmap item 4.1.1 requires a Streamable HTTP endpoint for `repovec-mcpd`. The
first proposal pinned MCP 2025-11-25, because the roadmap and technical design
asked for a GET stream and protocol sessions, and the
[2026-07-28 revision][changelog] removes both.

That reasoning treated transport mechanisms as product requirements. A review
on 2026-11-26 tested each one against primary sources, with these results.

- **The newer revision replaces the mechanisms; it does not drop the
  capabilities.** The [2026-07-28 changelog][changelog] removes
  `Mcp-Session-Id`, the `initialize` handshake, the GET endpoint, and
  `Last-Event-ID` replay. It adds per-request metadata, a mandatory
  `server/discover` method, `subscriptions/listen` for change notifications,
  and Multi Round-Trip Requests for server-to-client input. Tool calls still
  use POST, and results still stream over Server-Sent Events (SSE).
- **RepoVec uses none of the removed mechanisms for its own purposes.** The
  appliance serves read-only search, trace, and graph queries. It sends no
  unsolicited notifications, makes no server-to-client requests, and keeps no
  per-client state. Sessions were needed only because the design mapped each
  one to a `grepai mcp-serve` subprocess, which is an implementation choice that
  [ADR 0004](adr-0004-grepai-backend-bridge.md) now revisits.
- **One endpoint can serve both protocol eras.** The
  [versioning specification][versioning] permits a server to answer both
  `initialize` clients and per-request-metadata clients, and its compatibility
  matrix shows a dual-era server working with every client era. A modern-only
  server fails legacy clients, which "have no fall-forward mechanism".
- **Legacy clients are still in use.** A comment of 2026-10-04 on
  [openai/codex#37417][codex] reports that Codex 0.155 to 0.160 negotiates
  protocol 2025-06-18. Anthropic's [announcement][claude] of 2026-07-28
  describes support as "rolling out across Claude products". No source checked
  on 2026-11-26 shows every mainstream agent on the modern revision.
- **The official Rust Software Development Kit (SDK) implements both eras.**
  [RMCP 3.11.0][rmcp-release] was released on 2026-11-17. Its
  [README][rmcp-readme] states that it "serves the `2026-07-28` draft
  statelessly automatically", and that disabling `legacy_session_mode` serves
  legacy versions without sessions too. Its [server configuration][rmcp-config]
  bounds request bodies, validates `Host` and `Origin`, and selects JSON or SSE
  responses.
- **The direction of travel is stateless.** The maintainers'
  [release-cycle announcement][cadence] targets the next revision for
  2027-03-30, and the 2026-07-28 feature lifecycle policy guarantees deprecated
  features at least twelve months. No source suggests that sessions or the GET
  stream will return.

The question is therefore which protocol contract RepoVec adopts as its
foundation, and how it treats clients that still speak an earlier revision.

## Decision drivers

- Serve the agents that operators use today, whichever era they speak.
- Avoid building mechanisms that the current specification has removed and
  that no RepoVec capability needs.
- State an auditable protocol contract and avoid mixed-era semantics.
- Keep HTTP infrastructure outside the corpus and query domain.
- Avoid exposing a partially secured service while adjacent tasks remain open.
- Prefer a maintained implementation of the protocol to a bespoke one.

## Options considered

### Option A: Pin MCP 2025-11-25 (the first proposal)

Build a thin HTTP adapter for the 2025-11-25 transport: GET and POST on one
endpoint, with sessions to follow in item 4.1.2. This preserves the original
roadmap wording. It also commits the project to an owned GET stream per
connection, session lifecycle, and a documented gap against replay, none of
which a RepoVec capability requires. Clients that speak only 2026-07-28 would
be refused.

### Option B: Implement only MCP 2026-07-28

Build against the current revision alone. This is the smallest contract. It
refuses every legacy client, and the evidence above shows that such clients
remain common.

### Option C: Modern stateless foundation with legacy negotiation

Adopt 2026-07-28 as the architectural foundation, and answer legacy clients
through `initialize` without sessions. Use RMCP's Streamable HTTP service for
both eras.

| Consideration                     | Option A          | Option B    | Option C          |
| --------------------------------- | ----------------- | ----------- | ----------------- |
| Modern (2026-07-28) clients       | Refused           | Served      | Served            |
| Legacy (`initialize`) clients     | Served            | Refused     | Served, stateless |
| Protocol sessions to build        | Yes (4.1.2)       | No          | No                |
| Standalone GET stream to build    | Yes               | No          | No                |
| Replay or documented replay gap   | Documented gap    | Not in spec | Not offered       |
| Protocol implementation           | Bespoke           | Bespoke/SDK | RMCP              |
| Alignment with 2027-03-30 release | Against direction | Aligned     | Aligned           |

_Table 1: Trade-offs between the transport baseline options._

## Decision outcome

Adopt Option C.

- **Foundation.** MCP 2026-07-28 Streamable HTTP is the contract RepoVec is
  designed around: one `/mcp` endpoint that accepts POST, per-request metadata,
  `server/discover`, and a JSON or SSE response per request.
- **Legacy compatibility.** The same endpoint answers `initialize` for the
  earlier Streamable HTTP revisions that the verified RMCP release supports;
  2025-03-26 to 2025-11-25 are expected. Legacy clients are served statelessly:
  the server issues no `Mcp-Session-Id`, opens no standalone stream, and offers
  no replay. This is compatibility, not a second architecture.
- **Removed mechanisms.** GET and DELETE on `/mcp` return
  `405 Method Not Allowed`. A received `Mcp-Session-Id` or `Last-Event-ID`
  header is ignored. The deprecated HTTP+SSE transport of 2024-11-05 is not
  offered.
- **Implementation.** Use RMCP, pinned by a caret requirement to the release
  verified in the first milestone, mounted as a Tower service on Axum. The
  execution plan's first milestone verifies the behaviours this ADR relies on
  against the real crate before any dependency is added. If RMCP fails a
  required behaviour, stop and return to this ADR.
- **Capabilities.** Advertise tools only. Do not advertise resources,
  prompts, list-change notifications, or the tasks extension until a RepoVec
  capability needs one.
- **Cancellation.** Closing a response stream cancels that request, as
  2026-07-28 requires. For legacy clients the same rule applies once a response
  can no longer be delivered. Revision 2025-11-25 says a disconnect should not
  be read as cancellation, but without replay the result is undeliverable, and
  every RepoVec tool is a read-only query. This SHOULD-level deviation was
  accepted with this ADR.
- **Delivery boundary.** Keep the shipping composition closed to MCP work
  until authentication, Origin policy, and a real backend exist. A loopback
  listener that answers `503 Service Unavailable`, and test-only injected
  policies, permit network contract tests without an unauthenticated production
  query path.
- **Vocabulary.** RMCP's protocol model types are the shared MCP vocabulary
  and may appear in application ports. HTTP, Tower, and Axum types may not
  leave the inbound adapter.

## Goals and non-goals

The goal is a testable transport foundation that serves both client eras and
preserves the intended security and bridge boundaries.

This decision does not deliver protocol sessions, a GET stream, stream replay,
the HTTP+SSE transport, `subscriptions/listen`, the tasks extension, or new
query semantics. Item 4.1.1 does not complete remote corpus access, HTTPS
deployment, API-token storage, Origin policy, or the grepai bridge.

## Migration plan

1. Follow the
   [Streamable HTTP execution plan](../execplans/4-1-1-streamable-http-endpoint.md).
   This ADR and [ADR 0004](adr-0004-grepai-backend-bridge.md) were accepted on
   2026-11-26.
2. Run the plan's RMCP verification milestone, and record the verified crate
   version, features, and observed behaviour here.
3. Deliver the endpoint behind the closed production composition.
4. A later protocol
   revision must update the roadmap, design, client documentation, and contract
   tests together.

## Known risks and limitations

- **SDK conformance is asserted, not yet observed.** The RMCP behaviour cited
  here comes from its documentation. The first milestone tests it over a real
  socket, and the contract tests use a raw HTTP client so that an RMCP client
  and server cannot agree on a shared defect.
- **Legacy clients lose stateful features.** A legacy client that expects a
  GET stream receives 405. RepoVec emits nothing on such a stream, so no
  capability is lost, but a strict client may log the refusal.
- **Client claims were only partly verified.** The Codex report above is a
  user comment, not a vendor statement. An issue said to have been opened on
  2026-11-20 about the modern Codex client and tool-list subscriptions was not
  found. Current Claude Code behaviour was not confirmed beyond the July
  announcement.
- **The specification will move again.** The 2027-03-30 revision may change
  subscriptions, tasks, and authorization. Advertising tools only keeps the
  exposed surface small.

## Outstanding decisions

None blocks implementation. The user accepted the following on 2026-11-26:

- the modern stateless foundation with stateless legacy compatibility;
- the choice of RMCP, subject to the verification milestone;
- treating disconnect as cancellation for legacy clients;
- the fail-closed incremental delivery boundary;
- advertising tools only; and
- moving request limits and cancellation propagation to roadmap item 4.1.2.

If RMCP fails the verification milestone, this ADR must be revisited before any
alternative is built.

## Architectural rationale

The appliance exposes stateless, read-only queries over repositories. A
stateless protocol matches that shape, removes session affinity from any later
scaling decision, and lets the security boundary rest on per-request
authentication instead of connection state. Separating the protocol lifecycle
from the backend process lifecycle, as ADR 0004 proposes, is what makes the
session mechanism unnecessary.

[changelog]: https://modelcontextprotocol.io/specification/2026-07-28/changelog
[versioning]: https://modelcontextprotocol.io/specification/2026-07-28/basic/versioning
[codex]: https://github.com/openai/codex/issues/37417
[claude]: https://claude.com/blog/bringing-mcp-2026-07-28-to-claude
[rmcp-release]: https://github.com/modelcontextprotocol/rust-sdk/releases/tag/rmcp-v3.11.0
[rmcp-readme]: https://github.com/modelcontextprotocol/rust-sdk/blob/rmcp-v3.11.0/README.md#stateless-streamable-http
[rmcp-config]: https://docs.rs/rmcp/latest/rmcp/transport/streamable_http_server/tower/struct.StreamableHttpServerConfig.html
[cadence]: https://blog.modelcontextprotocol.io/posts/mcp-release-cycle/
