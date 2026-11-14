# Architectural decision record (ADR) 002: MCP transport baseline

## Status

Proposed. Approval of the linked execution plan is required before
implementation. No transport implementation is authorized by this document.

## Date

2026-10-20.

## Context and problem statement

Roadmap item 4.1.1 requires one Model Context Protocol (MCP) endpoint
supporting GET and POST, with JSON and Server-Sent Events (SSE) responses. The
technical design also requires initialization and protocol sessions. These
requirements match the [2025-11-25 transport specification][legacy].

The [2026-07-28 specification][current] removes GET streams and protocol
sessions. It also makes closing an SSE response stream cancel its request; the
earlier specification explicitly distinguishes disconnect from cancellation.
Following an unversioned specification or an SDK's defaults would silently
change the approved project requirements.

## Decision drivers

- Preserve the requested GET/POST and subsequent session workstreams.
- State an auditable protocol version and avoid mixed-version semantics.
- Keep HTTP infrastructure outside the corpus and query domain.
- Avoid exposing a partially secured service while adjacent tasks remain open.

## Options considered

Adopting 2026-07-28 immediately would require revising the roadmap, session
architecture, bridge lifecycle, and client contract. Supporting both revisions
would introduce two protocol lifecycles before either has been implemented.
Pinning 2025-11-25 preserves the requested scope at the cost of requiring
clients that support that revision.

## Proposed direction

Implement item 4.1.1 against MCP 2025-11-25. Treat newer-version support as a
separate architecture decision. Do not claim latest-protocol compatibility.
Reject unsupported explicit protocol versions. The execution plan specifies how
absent version headers are handled and tested. Negotiated context takes
precedence over the 2025-03-26 fallback when a header is absent.

Use a thin HTTP adapter with injected application ports. Compare the official
Rust SDK with a small Axum adapter during the first approved implementation
milestone; accept an SDK only if it passes the pinned contract tests without
importing later session or subprocess work. Resolve crate versions and feature
flags before adding dependencies, using caret requirements and the lockfile.

Keep the shipping composition closed to accepted MCP work until real security
policies and a backend are available. A loopback listener and test-only
injected policies permit network contract tests without an unauthenticated
production query path. Item 4.1.1 does not complete remote corpus access, HTTPS
deployment, API-token storage, Origin policy, session management, or the grepai
bridge. The users guide must describe the persistent loopback listener, its
unavailable response, and the remaining prerequisites.

Transport contracts must be described independently of HTTP framework types.
Single-owner response channels are preferred to a shared routing registry.
Tests exercise request isolation, response correlation, framing, resource
bounds, and failure behaviour. A separate pure routing kernel, if needed,
requires a new proof decision and a representation relationship to executable
code; an idealized model alone is not evidence about the implementation.

## Goals and non-goals

The goal is a testable transport foundation that preserves the intended
security and bridge boundaries. There is no legacy HTTP+SSE compatibility
endpoint, persisted session format, resumable event log, or new query semantics
in this item. SSE event IDs and the recommended empty priming event are omitted
because no resumable event log exists. Reject `Last-Event-ID` rather than
advertising unsupported replay; this documented SHOULD-level deviation is
included in the approval request.

## Migration plan

After explicit approval, follow the
[Streamable HTTP execution plan](execplans/4-1-1-streamable-http-endpoint.md).
Record the dependency decision and executable evidence in that plan. Update
this ADR to Accepted only when the user accepts the proposed baseline. Future
protocol migration must update the roadmap, design, client documentation, and
contract tests together.

## Known risks and limitations

Current SDK releases may default to the newer protocol. Transport tests must
verify the pinned wire behaviour rather than relying on an SDK name or
successful compilation. GET stream ownership needs an explicit injected
connection identity until session management lands; a shared anonymous event
bus is prohibited.

## Outstanding decisions

Approval must explicitly cover retaining MCP 2025-11-25 and the fail-closed
incremental delivery boundary. The SDK feasibility check may refine internal
adapter mechanics; it may not change the approved wire protocol or security
boundary without renewed approval.

[legacy]: https://modelcontextprotocol.io/specification/2025-11-25/basic/transports
[current]: https://modelcontextprotocol.io/specification/2026-07-28/basic/transports/streamable-http
