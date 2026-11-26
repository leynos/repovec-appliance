# Architectural decision record (ADR) 0004: Terminate MCP and manage grepai backends

## Status

Proposed. The pooling element depends on a feasibility spike against the pinned
grepai binary, described under "Migration plan". No bridge implementation is
authorized by this document.

## Date

2026-11-26.

## Context and problem statement

grepai exposes its tools through `grepai mcp-serve`, which
[communicates over stdio only][grepai-serve]. The technical design therefore
made `repovec-mcpd` a transparent bridge: one `grepai mcp-serve` subprocess per
Model Context Protocol (MCP) session, with every JSON Remote Procedure Call
(JSON-RPC) message forwarded unread. Roadmap item 4.2.2 required tool calls to
pass "without interpretation".

Three findings, gathered on 2026-11-26, undermine that design.

- **The session was the only reason for one process per client.**
  [ADR 0003](adr-0003-mcp-transport-baseline.md) proposes a stateless
  transport. Without a protocol session, nothing identifies the lifetime of a
  per-client subprocess.
- **grepai selects its project per request.** The
  [grepai changelog][grepai-changelog] for 0.37.0 (2026-09-10) records that
  project-scoped tools "accept an optional `root` parameter with an absolute
  project path", so one server can answer for several projects "without
  restarting `mcp-serve`". Tools also accept `workspace` and `project` arguments
  (`projects` on `grepai_search`), and `--workspace` starts a server from the
  workspace configuration.
- **Transparent forwarding is unsafe on a remote, multi-corpus appliance.**
  With `root`, "configuration, vector store and symbol index are loaded from
  that path instead of the server startup project", and an explicit `root`
  "takes precedence over a server started with `--workspace`". A remote caller
  who can set `root` chooses which index on the host is read. The grepai 1.2.0
  release also added a warning about indexing untrusted repositories.

The same changelog states that "read-only search, MCP, and trace processes
remain concurrent", so several reader processes may share one index. How a
single `mcp-serve` process behaves under concurrent requests, cancellation, and
sustained reuse is not documented and has not been tested here.

The question is how `repovec-mcpd` relates the external protocol to grepai
processes, and what it does with tool arguments.

## Decision drivers

- Authorization must bound the repositories a caller can reach, whatever the
  transport does.
- Agent count must not determine process count.
- Backend processes should survive client reconnects and be owned by an
  appliance policy.
- The external and internal MCP revisions should be able to differ.
- grepai's search, trace, and graph semantics must not be reimplemented.

## Options considered

### Option A: One subprocess per protocol session, forwarded unread

The existing design. It needs sessions, which ADR 0003 removes, and it forwards
`root` unchecked. Sixty agents mean sixty processes.

### Option B: One subprocess per request

Spawn `grepai mcp-serve` for each call. This needs no shared state and isolates
failures completely. Every call pays process start-up, index loading, and an
MCP handshake, which is likely to dominate short queries.

### Option C: Terminate MCP; manage a bounded pool of grepai clients

`repovec-mcpd` is the MCP server for external callers and an MCP client of
grepai. It owns a bounded pool of long-lived `grepai mcp-serve` subprocesses,
partitioned by security context, and applies an authorization policy to each
tool call before forwarding it.

| Consideration                  | Option A        | Option B          | Option C           |
| ------------------------------ | --------------- | ----------------- | ------------------ |
| Needs protocol sessions        | Yes             | No                | No                 |
| Processes for sixty agents     | Sixty           | One per call      | Bounded by policy  |
| Survives client reconnect      | No              | Not applicable    | Yes                |
| Cost per call                  | Low after start | Start-up and load | Low after warm-up  |
| Argument authorization         | None            | Possible          | Required           |
| Independent protocol revisions | No              | Yes               | Yes                |
| Unverified grepai behaviour    | None            | None              | Concurrency, reuse |

_Table 1: Trade-offs between the backend bridge options._

## Proposed direction

Adopt Option C, with Option B as the fallback if the spike refutes pooling.

- **Protocol termination.** `repovec-mcpd` answers the external protocol
  itself. It connects to each grepai process as an MCP client over stdio and
  negotiates that connection independently.
- **Managed backends.** A supervisor owns a bounded pool of
  `grepai mcp-serve` processes. It sets the maximum pool size, idle lifetime,
  start-up deadline, and restart back-off. Each process starts with
  `HOME=/var/lib/repovec` and an explicitly built environment.
- **Partitioning.** Processes are partitioned by security context, initially
  one partition per grepai workspace started with `--workspace`. A request is
  served only by a process in a partition its caller may use.
- **Tool catalogue.** The catalogue offered to callers is derived from
  grepai's `tools/list` and filtered through an allowlist. A tool that is not
  on the allowlist is neither listed nor callable, so a grepai upgrade cannot
  expose a new tool without review.
- **Argument authorization.** Each call's arguments are checked before
  forwarding. The `root` argument is refused, and it is removed from the input
  schemas shown to callers. `workspace`, `project`, and `projects` values are
  checked against the caller's authorized scope, and the effective scope is set
  by the appliance when the caller omits them. Unknown arguments on a scoped
  tool are refused.
- **Semantics.** Results are forwarded without reinterpretation, so search,
  trace, and graph behaviour remains grepai's.
- **Failure isolation.** A crashed or unresponsive process fails only the
  requests it holds. Those callers receive an error; the supervisor replaces
  the process, and later requests succeed without any client-side
  re-establishment.

## Goals and non-goals

The goal is a bridge whose security boundary is the authorization of each call,
and whose process count is an appliance policy.

This decision does not reimplement grepai queries, define token scopes (item
4.3), or change how indexers run. It does not make the allowlist a
compatibility layer: a tool is offered as grepai defines it or not at all.

## Migration plan

1. **Feasibility spike.** Before item 4.2.1 commits to pooling, exercise the
   pinned grepai binary through one long-lived `mcp-serve` process and record:
   - whether concurrent requests on one connection complete correctly, and
     with what latency;
   - whether cancelling one request leaves the others, and the process,
     intact;
   - whether repeated queries over a long-lived process stay correct and
     bounded in memory; and
   - which MCP revision grepai negotiates, and whether it issues any
     server-to-client request.
2. **Decision gate.** If concurrent use of one process is unsafe, serialize
   requests per process and size the pool accordingly. If long-lived reuse is
   unsafe, fall back to Option B. Record the outcome here.
3. **Deliver items 4.2.1 to 4.2.3** as revised in the roadmap.

## Known risks and limitations

- **Pooling rests on unverified behaviour.** The spike above gates it.
- **The allowlist must track grepai.** A new grepai tool stays hidden until
  it is reviewed, which trades automatic availability for a known surface.
- **Schema rewriting can drift.** Removing `root` from an input schema
  depends on grepai's schema shape; a contract test against the pinned binary
  must fail when that shape changes.
- **Partition granularity is provisional.** One partition per workspace is
  the starting point; token scopes in item 4.3 may require a finer one.
- **grepai may send server-to-client requests.** grepai 0.37.0 advertises
  tool capabilities so that clients can discover roots. If it issues
  `roots/list`, `repovec-mcpd` answers as the client and never relays the
  request to an external caller.

## Outstanding decisions

- The partition key, once token scopes are designed in item 4.3.
- Pool sizing defaults, after the spike measures start-up and query cost.
- Whether `grepai_list_workspaces` and `grepai_list_projects` are offered,
  filtered to the caller's scope, or withheld.

## Architectural rationale

Separating the protocol lifecycle from the backend process lifecycle lets each
follow its own constraint: the protocol is stateless per request, and processes
are a pooled resource under admission control. Placing authorization at the
tool call makes the security boundary independent of transport details, which
is the property a remote appliance needs.

[grepai-serve]: https://yoanbernabeu.github.io/grepai/commands/grepai_mcp-serve/
[grepai-changelog]: https://github.com/yoanbernabeu/grepai/blob/main/CHANGELOG.md
