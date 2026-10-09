# repovec-appliance technical design

## Document status and governing decisions

This is the living design for the appliance's production architecture. Accepted
Architecture Decision Records (ADRs) take precedence where this document and a
decision record diverge. The roadmap owns implementation status, while the
[nursery crate guide](nursery-crates.md) owns current experimental interfaces
and graduation evidence.

The following proposed decisions define the rationalization target described in
this revision:

- [ADR 001](adr-001-retire-custom-oauth-device-flow.md) delegates OAuth device
  grant mechanics to `oauth2`.
- [ADR 002](adr-002-rationalize-systemd-and-quadlet-analysis.md) separates
  source models, appliance policy, official consumer verification, and live
  systemd control.
- [ADR 003](adr-003-centralize-unit-contract-policy.md) centralizes
  parser-neutral rules and structured diagnostics.
- [ADR 004](adr-004-manage-extraction-candidates-in-a-nursery.md) governs
  experimental crates under `crates/nursery/`.
- [ADR 005](adr-005-rationalize-secret-persistence.md) separates secret
  protection, durable persistence, systemd delivery, and Podman synchronization.

Sections that describe migration targets distinguish them from the current
implementation. The current implementation remains authoritative until its
roadmap migration task is complete.

## Problem statement and design goals

repovec-appliance is a self-hosted VM appliance that turns a user's private
repositories on GitHub into a continuously indexed, multi-branch semantic and
graph-queryable corpus, exposed as a remote Model Context Protocol (MCP) server
over HTTPS. The core interaction model is:

- user authorizes the appliance (device flow) to access repos/branches and
  (optionally) create webhooks
- appliance clones repos, creates per-branch worktrees, and keeps them
  current
- grepai runs indexers (semantic embeddings + symbol/call-graph/Relational
  Property Graph (RPG) graph) into a store backed by Qdrant
- users (or agents) talk to a single MCP HTTPS endpoint, filter queries by
  repo + branch, and get:
  - semantic search results
  - call graph tracing (callers/callees/graph)
  - RPG graph interrogation (search/fetch/explore)

grepai already provides (a) daemonized indexing (`grepai watch`) and (b) MCP
tool exposure (`grepai mcp-serve`) with search, trace, index status, and RPG
graph tools. The appliance's job is to operationalize this at "many repos +
many branches", add lifecycle management, and add hardened remote access with
token minting/revocation, Cloudflare-managed DNS/TLS, and a text user interface
(TUI) configuration surface.

### Non-goals

- Not a full managed repo host (it indexes repositories; it does not replace
  GitHub).
- Not a general-purpose "coding agent" runtime (it serves retrieval/graph
  context; the agent runs elsewhere).
- Not a multi-tenant SaaS; appliance is single-owner (with optional multiple
  issued API tokens).

## High-level architecture

The appliance is composed of five long-running concerns, each mapped to systemd
units and explicit data directories:

### Control plane

A Rust daemon, `repovecd`, provides:

- GitHub device-flow login, token storage/refresh and permissions checks
- repository/branch discovery (polling), plus optional webhook registration
  and webhook ingestion
- creation/removal of:
  - bare mirrors and per-branch worktrees
  - grepai workspaces and workspace projects
  - systemd units for grepai indexers
- health and status API used by the TUI (over a local-only Unix socket)

### Data plane

- grepai indexers:
  - `grepai watch` builds and maintains embeddings and graph indices
    continuously
  - workspaces allow modelling {repo} as a workspace and {branch} as
    projects inside it, with `--workspace` / `--project` query scoping and
    cross-project search
- Qdrant:
  - runs locally on the VM (Podman container) as the vector store for grepai
  - bound to `127.0.0.1` only and protected with API keys

### Remote MCP endpoint

A Rust service, `repovec-mcpd`, exposes an MCP endpoint over Streamable HTTP
transport (single endpoint supporting GET and POST), implements origin
validation, sessions, and authentication as required by the MCP transport
specification.

Because grepai's built-in MCP server is stdio transport (`grepai mcp-serve`),
and is designed for local agent integrations, `repovec-mcpd` acts as a
transport and security adapter:

- externally: Streamable HTTP MCP over HTTPS
- internally: one `grepai mcp-serve` subprocess per MCP session (or per
  client), with JSON-RPC bridged between HTTP and stdio

grepai's MCP tool surface includes:

- `grepai_search` (semantic search, includes RPG context when enabled)
- trace tools (`grepai_trace_callers`, `grepai_trace_callees`,
  `grepai_trace_graph`)
- `grepai_index_status`
- RPG graph tools (`grepai_rpg_search`, `grepai_rpg_fetch`,
  `grepai_rpg_explore`)

This keeps "graphing semantics" identical to grepai, rather than reimplementing
them.

### Edge networking, DNS, and TLS

The recommended exposure mechanism is Cloudflare Tunnel:

- `cloudflared` maintains outbound tunnel connections to Cloudflare, and
  Cloudflare routes a hostname to that tunnel via DNS records.
- tunnel creation and DNS routing can be fully automated via the Cloudflare
  API; Cloudflare documents required token permissions for the "Create a tunnel
  (API)" flow.

This approach avoids exposing the VM directly to the Internet (no inbound 443
needed), while still providing a public HTTPS endpoint with Cloudflare-managed
TLS at the edge.

As an alternative (when tunnels are undesired), the appliance can run a
public-facing reverse proxy (or `repovec-mcpd` directly on 443) behind
Cloudflare's reverse proxy using Cloudflare Origin CA certificates. Cloudflare
documents Origin CA certificate creation and also exposes Origin CA certificate
APIs.

### Operator interface

- `repovec-tui`: a ratatui TUI over SSH, used to:
  - run the GitHub device flow
  - configure embedder/store choices
  - view repo/branch indexing status
  - mint/revoke MCP access tokens
  - trigger reconciliation and upgrades

## GitHub integration and repository lifecycle

### Authentication: device flow

repovec-appliance uses GitHub's OAuth device flow so the VM can be configured
via SSH without a browser on-box:

- request device/user codes via
  `POST https://github.com/login/device/code`
- user enters the shown code at `https://github.com/login/device`
- appliance polls `POST https://github.com/login/oauth/access_token` until
  approval or expiry, respecting the server-provided minimum interval to avoid
  `slow_down` errors

GitHub explicitly indicates the device flow does not require the OAuth app
`client_secret` (device flow uses `client_id` + device code + grant type).

The target implementation uses the generic `oauth2` crate for the complete
device authorization grant rather than only for HTTP types. `oauth2` owns the
device authorization request, token polling, RFC 8628 response interpretation,
and protocol errors. A thin `repovecd` adapter owns GitHub endpoint selection,
one redirect-disabled HTTP client, timeouts, operator prompt presentation,
telemetry, and token persistence. Behavioural tests continue to use
`oauth2-test-server` and deterministic time seams.

The current implementation still contains repovec-owned wire structures,
protocol outcome types, and a polling state machine. Roadmap item `1.4.2`
removes those duplicates after scenario parity is demonstrated. New OAuth
features must extend the upstream-backed adapter rather than the private
protocol model.

Access tokens are persisted below `/etc/repovec/` as
`github-oauth-token.cred`. The write and rotation path protects the bearer token
without placing plaintext on a command line, then commits the protected bytes
through a durable repository. The repository contract requires restrictive
creation mode, same-directory temporary creation, complete write and file
synchronization, atomic replacement, and containing-directory synchronization.

The target service unit uses systemd service credentials to decrypt and expose
the token only for the `repovecd` activation lifetime. The daemon reads from
the credential directory supplied by systemd instead of spawning a second
`systemd-creds decrypt` process. Existing Rust credential readers are evaluated
before local reader code is added. Write-side `systemd-creds` invocation remains
an adapter only if no maintained crate provides the required operation.

Reloaded tokens contain the bearer secret only; scope metadata is treated as
server response metadata and is not persisted with the credential. Permission
checks based on granted scopes are therefore login-time checks only in this
adapter. The GitHub OAuth token reload flow infers no scope-derived
authorization from the bearer secret alone. After a restart, the control plane
must revalidate the reloaded token against GitHub before enforcing permissions
that depend on granted scopes; if that lookup fails, permissions remain unknown
and the operator must complete login again.

### Discovery and continuous monitoring

repovecd maintains correctness via a reconcile-first model:

- **Periodic reconciliation** (authoritative):
  - list accessible repositories
  - for each repository, list branches and determine "active branches"
  - ensure local clones/worktrees/workspace projects match desired set
- **Webhook acceleration** (optional):
  - on push/create/delete activity, update immediately and avoid waiting
    for the next reconcile

This split matters because not every desired event is reliably available via a
single GitHub webhook, and webhook delivery can be disrupted; periodic
reconciliation preserves eventual correctness.

### Webhook events and how they map to workspaces

If the user grants scopes/permissions sufficient to register webhooks, the
appliance configures:

- `push` events:
  - GitHub documents that push events include branch pushes and also
    include booleans `created` and `deleted` indicating whether the push
    created or deleted the ref.
  - repovecd uses this to:
    - detect new branches (`created=true`) and provision branch
      worktrees/projects
    - detect branch deletions (`deleted=true`) and retire branch
      worktrees/projects (subject to retention policy)
- `create` events:
  - GitHub documents `create` fires when a branch or tag is created, with
    `ref` and `ref_type` (`branch`/`tag`).
  - this can be used as an earlier signal than the first push, but
    `push.created` already covers most "new branch" workflows.

For organization-wide automation, GitHub provides organization webhooks and
notes that OAuth app tokens (and classic PATs) need `admin:org_hook` scope to
create them. This is useful when an operator wants to automatically index new
repos created in the org without manually configuring each repository, while
still keeping polling as the safety net.

## Workspace model and branch indexing strategy

### Canonical mapping

The appliance models:

- **Workspace = repository**
- **Project = branch**

This aligns with grepai's multi-project workspace capabilities (workspace
configuration includes store/embedder and a list of project entries) and
grepai's ability to search across projects with `--workspace` and optionally
scope with `--project`.

grepai's workspace configuration is stored globally in
`~/.grepai/workspace.yaml`. The appliance runs grepai as a dedicated system
user (e.g. `repovec`), so workspace config lives in that user's home (e.g.
`/var/lib/repovec/.grepai/workspace.yaml`).

grepai documents path prefixing for workspace isolation as
`workspaceName/projectName/relativePath`. repovec uses this to safely index
multiple branches into a shared store without collisions.

### Worktrees and checkout layout

Per repo:

- maintain a bare mirror:
  - `/var/lib/repovec/git-mirrors/{owner}/{repo}.git`
- create per-branch worktrees:
  - `/var/lib/repovec/worktrees/{owner}/{repo}/{branch}/`
- update worktrees on pushes:
  - fetch mirror
  - `git worktree` add/update
  - hard reset the worktree to the target ref (to avoid drift)

This makes branch indexing deterministic and avoids "branch switches in place"
that can confuse file watchers.

grepai has explicit, evolving support for git worktrees and multi-worktree
watch/daemon behaviour (including worktree detection utilities and
multi-worktree improvements noted in releases). The appliance leverages that
where possible, but it does not require it (it can run per-branch watchers if
needed).

### Active branch policy

Indexing every branch forever becomes expensive (storage, embedding churn, and
watch CPU). repovec therefore defines an "active branch set" policy:

- always index default branch
- index any branch with pushes in the last *N* days
- optionally index branches referenced by open pull requests
- cap total indexed branches per repo (LRU eviction beyond cap)

This policy is fully configurable in the TUI; the reconcile loop applies it to
add/remove projects and start/stop corresponding indexers.

## MCP HTTPS endpoint and authentication

### MCP transport and security invariants

repovec-mcpd implements MCP Streamable HTTP transport because MCP defines
Streamable HTTP as the standard remote transport and describes requirements
including:

- a single HTTP endpoint supporting GET and POST
- Origin validation to mitigate DNS rebinding
- binding to localhost when running locally
- authentication for all connections

repovec-mcpd follows the MCP session mechanism (`Mcp-Session-Id` header) so it
can map a session to a dedicated `grepai mcp-serve` subprocess and cleanly
terminate sessions.

### Bridging to grepai MCP tools

grepai's built-in MCP server communicates via stdio and exposes the full grepai
tool surface, including RPG graph tools. repovec-mcpd bridges Streamable HTTP
JSON-RPC to stdio JSON-RPC:

- on `InitializeRequest`, spawn:
  - `grepai mcp-serve` (with environment set to the grepai system user's
    HOME so `~/.grepai/workspace.yaml` resolves)
- for each incoming JSON-RPC message:
  - forward to stdin (newline-delimited, without embedded newlines as per
    stdio transport expectations)
- stream responses back to the client using either:
  - `application/json` (single response) or
  - `text/event-stream` (SSE stream), as allowed by Streamable HTTP
    transport

This design intentionally avoids "re-implement grepai semantics" and therefore
preserves:

- hybrid search behaviour
- trace output shape and depth behaviour
- RPG graph traversal semantics
- any future grepai MCP tool additions (the proxy can be designed to be
  largely transparent)

### Token minting and revocation

repovec provides authentication at the MCP endpoint independent of GitHub
credentials:

- **API tokens** are random, high-entropy secrets (e.g. 256-bit), shown
  once at creation.
- tokens are stored hashed (Argon2id) with metadata: name, created\_at,
  last\_used\_at, optional expiry, scopes (read/search/trace/admin).
- revocation is immediate: set revoked\_at and reject thereafter.

To align with MCP's emphasis on proper authentication for remote servers and to
reduce exposure to CSRF/DNS rebinding vectors, repovec-mcpd requires:

- `Authorization: Bearer <token>` on all non-initialization requests
- strict `Origin` allowlist (configured hostnames only), rejecting
  absent/incorrect origins on browser-capable clients, as MCP recommends for
  Streamable HTTP servers

### Cloudflare edge integration

With Cloudflare Tunnel:

- DNS is a CNAME to the tunnel UUID (`<UUID>.cfargotunnel.com`) and is only
  valid within the same Cloudflare account.
- cloudflared connects outbound; this avoids exposing Qdrant or internal
  APIs and reduces attack surface.
- automation uses a Cloudflare API token with Tunnel edit + DNS edit
  privileges.

If the "direct origin" mode is chosen, the appliance provisions Cloudflare
Origin CA certificates (dashboard or API) and binds the MCP server with that
certificate/key, using Cloudflare's "Full (strict)" TLS model.

## Architecture rationalization and library reuse

### Ownership rule

Repovec owns appliance policy and product composition. It does not own generic
implementations of external protocols or configuration languages when a viable
specialist library exists. Adoption still requires compatibility evidence;
"use a dependency" is not a substitute for verifying its semantics.

The architecture therefore distinguishes four responsibilities:

1. **External format and protocol implementation.** Maintained libraries own
   OAuth device-flow mechanics, native systemd syntax, and Quadlet syntax.
2. **Repovec policy.** Production crates define the image, networking,
   dependency, identity, hardening, and secret-wiring contracts of the
   appliance.
3. **Independent consumer verification.** Podman and systemd tools verify that
   the real consumers accept source and generated units.
4. **Runtime effects.** Adapters invoke the GitHub API, credential tools, or the
   live systemd manager through explicit ports.

This separation prevents a parser result from becoming proof that systemd will
accept a unit, and prevents a tool invocation from becoming the only way to
unit-test product policy.

### Systemd and Quadlet source adapters

The current `systemd_units::ParsedUnit` and
`qdrant_quadlet::ParsedQuadlet` implementations are migration scaffolding, not
candidate public parsers. Both flatten source into maps and omit parts of the
systemd syntax and effective-value model.

Roadmap item `1.4.3` evaluates:

- [`systemd-unit-edit`](https://crates.io/crates/systemd-unit-edit) for native
  systemd units; and
- [`quadlet-lens`](https://crates.io/crates/quadlet-lens) for Podman Quadlets.

Adapters over the selected libraries implement the parser-neutral `UnitView`
interface. They preserve ordered directive occurrences, raw and decoded values,
source spans, reset assignments, and source origin. Parser-specific types do not
cross the adapter boundary.

The existing mutation and property-test corpus becomes differential adoption
evidence. It is not a reason to continue extending the private parsers. Missing
features should first become narrowly scoped upstream proposals.

### Unit contract policy

The nursery `repovec-unit-contract` crate defines source-aware unit views,
validation rules, a diagnostic sink, and accumulated validation reports. It does
not parse source, interpret every systemd directive, emit tracing, or know
repovec policy.

Production rule sets remain with their owning subsystem. A Qdrant rule set can
require the approved image, loopback bindings, storage mount, SELinux option,
auto-update policy, provisioning dependency, and secret target. A service rule
set can require target membership, executable path, identity, directory, and
hardening settings.

Rules return structured diagnostics containing a stable code, severity,
artefact identity, optional source span, and sensitivity. Application adapters
render diagnostics to tracing or terminal output. Future CI integrations may
render JSON or Static Analysis Results Interchange Format (SARIF) without
changing rule code.

The callback-per-finding `QdrantQuadletObserver` remains only until policy
migration is complete. It must not become the shared validation abstraction.

### Official consumer verification

The nursery `repovec-systemd-probe` crate defines evidence types and ports for
two external checks:

- the Podman system generator consumes caller-supplied Quadlet documents and
  produces native unit files; and
- `systemd-analyze verify` consumes native checked-in and generated unit files.

Probe adapters invoke programs directly without a shell and within
caller-controlled temporary roots. Reports retain the exact program, argument
vector, tool version, exit status, standard output, and standard error. Default
`Debug` output exposes only buffer lengths because tool diagnostics may repeat
secret-derived source.

Consumer verification and policy validation both gate packaging, but answer
different questions. Tool success does not prove appliance policy, and policy
success does not prove tool acceptance.

### Build-time, packaging-time, and runtime checks

Repository source contracts belong in build and packaging gates. Runtime
liveness checks belong in daemon startup when they verify resources the daemon
will actually use.

The current daemon startup checks embedded unit source. That proves neither the
installed file nor the effective unit after administrator drop-ins. Roadmap item
`1.4.6` replaces this ambiguous check with one of two honest boundaries:

- a build and packaging invariant over repository assets; or
- a runtime diagnostic over installed or effective manager configuration.

Authenticated Qdrant liveness remains a valid runtime check because it proves a
live dependency and credential path.

### Live systemd manager control

Per-branch indexer reconciliation requires live unit start, stop, enable,
disable, and state queries. Roadmap item `1.4.7` evaluates maintained Rust D-Bus
clients before defining the application port. Production code must not invoke
`systemctl` and parse human-oriented output.

The application port remains narrower than the systemd D-Bus API and models
only reconciliation needs. Deterministic fakes drive unit lifecycle tests;
the selected D-Bus adapter owns wire types and manager calls.

### Nursery crates

Potentially reusable interfaces live in the isolated `crates/nursery/`
workspace described by
[ADR 004](adr-004-manage-extraction-candidates-in-a-nursery.md)
and the [nursery crate guide](nursery-crates.md). The initial interfaces are:

- `repovec-unit-contract` for parser-neutral validation;
- `repovec-systemd-probe` for consumer-tool evidence; and
- `repovec-secret-store` for protection and durable persistence composition.

The production workspace excludes the nursery. A separate workflow compiles,
lints, tests, and documents it. No crate is publishable, and no compatibility
promise applies until the graduation evidence is complete.

## Systemd, Podman/Qdrant, and update lifecycle

### Service layout

Systemd manages the appliance lifecycle via a dedicated target:

- `repovec.target`
  - wants: `qdrant.service` (Podman), `repovecd.service`,
    `repovec-mcpd.service`, `cloudflared.service`
  - wants: concrete per-repository indexer instances when later
    reconciliation work enables them

Roadmap items `1.3.1` and `1.3.2` ship the static source files for the base
target, daemon services, and grepai indexer template under `packaging/systemd/`:

- `packaging/systemd/repovec.target`
- `packaging/systemd/repovecd.service`
- `packaging/systemd/repovec-mcpd.service`
- `packaging/systemd/repovec-grepai@.service`

On an appliance host, install these units to `/etc/systemd/system/`. The
existing Qdrant Quadlet remains installed to
`/etc/containers/systemd/qdrant.container`; systemd exposes that generated
container unit as `qdrant.service`, which is the name every dependent service
uses. `repovecd.service` declares both `Requires=qdrant.service` and
`After=qdrant.service`. `repovec-mcpd.service` declares
`Requires=qdrant.service repovecd.service` and
`After=qdrant.service repovecd.service`.

`repovec-grepai@.service` is a systemd template for future concrete indexer
instances. Each instance runs `/usr/bin/grepai watch` as the `repovec` user and
group, sets `HOME=/var/lib/repovec`, and uses
`WorkingDirectory=/var/lib/repovec/worktrees/%I`. The `%I` value is the systemd
instance identifier; roadmap item `3.2.1` owns the final mapping from
repository and branch identity to a concrete, systemd-safe instance name. The
template declares `Requires=qdrant.service repovecd.service`,
`After=qdrant.service repovecd.service`, `PartOf=repovec.target`, and
`WantedBy=repovec.target`. `WantedBy=repovec.target` wires manually enabled
instances into the appliance target at enable time, while
`PartOf=repovec.target` propagates target stop and restart operations to those
instances. The template keeps stdout and stderr in journald with
`StandardOutput=journal` and `StandardError=journal`; it must not create
bespoke log files.

The template includes conservative systemd sandboxing directives such as
`NoNewPrivileges=yes`, private temporary storage, read-only host filesystem
protections, kernel and namespace restrictions, limited address families, and
restricted process visibility. These hardening settings are packaging defaults
for the appliance image.

During the rationalization migration, `repovec-core` remains the owner of the
service-layout policy while parsing and diagnostic plumbing move behind the
interfaces described above. Packaging gates also ask the real Podman generator
and `systemd-analyze verify` consumers to process the files. None of those
static checks proves that a live host has `/usr/bin/grepai`, the `repovec` user,
concrete worktrees, Qdrant reachability, or the expected effective drop-ins.
Runtime checks must inspect those resources directly when the daemon depends on
them.

`cloudflared.service` is package-owned and is not supplied by the appliance
unit set. The target queues it with `Wants=cloudflared.service`; tighter tunnel
ordering belongs with the later Cloudflare provisioning work if that package
unit needs additional drop-ins.

Key service properties:

- indexers run as the unprivileged `repovec` account with fixed HOME;
- `sysusers.d` is the sole account-creation mechanism;
- shared package paths use `tmpfiles.d`, while service-owned state and
  configuration use `StateDirectory=` and `ConfigurationDirectory=` when their
  lifetime and ownership semantics fit;
- `/var/lib/repovec/` contains repositories, worktrees, and grepai indices;
- `/etc/repovec/` contains restricted configuration and protected credentials;
  and
- journald captures all unit output, with no bespoke log files.

### Qdrant under Podman + systemd

The appliance manages Qdrant via Podman + systemd. Podman's documentation now
prefers Quadlet files for systemd-managed containers; `podman generate systemd`
is explicitly described as deprecated in favour of Quadlets.

Qdrant networking assumptions:

- Qdrant exposes REST on 6333 and gRPC on 6334; Qdrant's docs show gRPC
  configured at `service.grpc_port: 6334` and typical docker invocation
  publishing both ports.
- grepai's configuration defaults show Qdrant endpoint `localhost` and
  default port `6334`, consistent with preferring Qdrant's gRPC port.

Security controls:

- Qdrant supports a static API key; Qdrant recommends API key auth and also
  recommends binding to localhost/private interfaces to prevent unauthenticated
  external access.
- In appliance mode, Qdrant binds to `127.0.0.1` only and is never exposed
  publicly; callers are local processes only.
- The sysusers asset `packaging/sysusers.d/repovec.conf` is the sole
  account-creation mechanism for the minimal `repovec` system user. The
  provisioning helper must fail clearly when that packaging prerequisite is
  absent; it must not invoke `useradd`.
- Shared directory creation and ownership move to a packaged `tmpfiles.d`
  definition, with service directory directives used where appropriate.
- A one-shot systemd unit, `repovec-qdrant-api-key.service`, provisions the
  local API-key material before `qdrant.service` starts. It generates
  `/etc/repovec/qdrant-api-key` only when that file is absent and relies on the
  declarative packaging layer for account and directory prerequisites.
- `/etc/repovec/qdrant-api-key` stores the raw key, not an environment-file
  assignment. The file is owned by `repovec:repovec` with mode `0400`, so the
  appliance service user can read it while other unprivileged users cannot.
- The provisioning helper refreshes the rootful Podman secret
  `repovec-qdrant-api-key` from the raw key file without printing the key. The
  Qdrant Quadlet exposes that secret to the container as
  `QDRANT__SERVICE__API_KEY`, which maps to Qdrant's `service.api_key` setting.
- The provisioning helper logs secret lifecycle decisions to journald through
  `repovec-qdrant-api-key.service` without printing the key value. Operators
  detect provisioning failures through the oneshot unit state, journal entries,
  and dependent daemon startup failures; no metrics exporter exists for this
  first-boot packaging step yet.
- Local clients authenticate to Qdrant by sending the stored key in the
  `api-key` request header. Requests without the header are expected to fail
  with Qdrant's authentication error response.
- The repository source of truth is
  `packaging/systemd/qdrant.container`, installed to
  `/etc/containers/systemd/qdrant.container` on rootful appliance hosts.
- The Quadlet tracks Qdrant's `docker.io/qdrant/qdrant:v1` image stream so
  registry auto-update can advance within the current major version while
  retaining a stable, fully qualified image reference.
- The Quadlet publishes `127.0.0.1:6333:6333` and
  `127.0.0.1:6334:6334`, keeping both Qdrant interfaces loopback-only even if
  container defaults change.
- Persistent storage is mounted from `/var/lib/repovec/qdrant-storage` to
  `/qdrant/storage` with an explicit `:Z` SELinux relabel, because the
  appliance uses rootful Podman-managed system services.
- The Quadlet owns only the container contract; boot-target wiring remains the
  responsibility of roadmap item `1.3.1`.

Runtime liveness validation is separate from the static Quadlet validator.
`repovec_core::appliance::qdrant_liveness` reads the raw API key from
`/etc/repovec/qdrant-api-key`, connects to the default gRPC endpoint
`http://127.0.0.1:6334`, and returns a redacted semantic error when the key
file, endpoint, authentication, connection, or readiness contract fails.

The liveness policy performs two gRPC operations:

- `health_check()` proves that the Qdrant process answers the gRPC health
  service.
- `list_collections()` proves that the stored API key authenticates a
  lightweight read-only Qdrant request.

The second operation is required because live integration testing showed that
Qdrant's gRPC health endpoint can answer even when the supplied API key is
wrong. `repovec_core::appliance::daemon_startup` currently validates embedded
checked-in systemd-unit source before establishing authenticated Qdrant
liveness. `repovecd` and `repovec-mcpd` are thin delegates to that shared
boundary.

Roadmap item `1.4.6` removes the ambiguous embedded-source startup check after
repository assets are covered by build and packaging gates. A later runtime
unit diagnostic, if required, inspects installed or effective live manager
configuration instead. Authenticated Qdrant liveness remains fatal because it
proves a dependency the daemon will immediately use. Failure causes the daemon
to exit with status `1`, leaving systemd to report a failed service and apply
its normal restart policy. Unit tests inject the liveness closure to exercise
ordering and failure mapping, while the ignored Podman integration test verifies
the network and authentication boundary.

### Validation diagnostics and telemetry boundary

The current Qdrant validator makes telemetry explicit through
`QdrantQuadletObserver`, with `TracingQdrantQuadletObserver` as the production
adapter and `()` as the no-op implementation. Secret-bearing parser content is
redacted before it reaches public errors or telemetry.

The target architecture replaces the broad callback surface with structured
`repovec-unit-contract::Diagnostic` values. Diagnostics carry code, severity,
artefact, source span, and sensitivity. A tracing renderer maps those values to
operator events under the appropriate target; silent validation simply omits a
renderer. This preserves caller-visible effects while allowing the same report
to support terminal output, JSON, or SARIF.

The existing observer remains a compatibility bridge only until roadmap item
`1.4.5` migrates all unit policies. New validators must return structured data
rather than add more domain-specific observer methods.

### Automatic updates and safe rollouts

There are three independently versioned artefacts:

- Qdrant container image
- grepai binary
- repovec binaries

**Qdrant updates** use Podman auto-update:

- Podman can auto-update containers when configured for auto-updates and
  run under systemd.
- Podman ships a `podman-auto-update.service` and a
  `podman-auto-update.timer` that triggers daily by default.
- The container/unit must opt in using the auto-update policy (documented in
  Podman systemd integration).

**grepai updates** can be driven by grepai itself:

- grepai provides `grepai update` which fetches the latest release from
  GitHub, verifies checksum integrity, and replaces the current binary.

repovec implements update policy as configurable systemd timers:

- `repovec-upgrade.timer` can:
  - pause indexers
  - upgrade Qdrant via `podman auto-update` + restart
  - upgrade grepai via `grepai update`
  - upgrade repovec via package or signed tarball update
  - resume indexers and reconcile state

This sequencing avoids embedding during store migrations and keeps the
appliance in a coherent state.

## Repository governance and CI gating

The repository uses GitHub Actions as the merge gate for code and documentation
changes. The workflow policy is intentionally derived from the same Make
targets contributors run locally:

- `make build`
- `make check-fmt`
- `make lint`
- `make test`

Markdown validation is treated as a conditional gate rather than an always-on
core build step:

- `make markdownlint`
- `make nixie`

Those documentation checks run only when the change set contains documentation
inputs. `make markdownlint` runs for any changed Markdown file and for
documentation-tooling configuration such as `.markdownlint-cli2.jsonc`, while
`make nixie` runs when one of the changed Markdown files contains a Mermaid
diagram. If the changed-file list is missing or malformed, the workflow takes a
safe fallback and runs both documentation checks rather than risking a skipped
validation. The workflow also keeps the conservative `make nixie` path when a
relevant Markdown file cannot be read during Mermaid detection. The
change-classification policy lives in a dedicated Rust helper so the decision
remains unit-testable and behaviourally testable rather than being buried
entirely in workflow YAML.

The repository-level required checks are intentionally stable and map directly
to workflow job names:

- `build`
- `check-fmt`
- `lint`
- `test`
- `docs-gate`

Merge enforcement is implemented through a GitHub repository ruleset targeting
`refs/heads/main`. The ruleset payload is versioned alongside the workflow so
the required-check policy is reviewable and can evolve with the repository.

## Embeddings configurability: OpenRouter vs Ollama

repovec exposes a single "embedding provider" configuration that is then
written into grepai workspace/store configuration.

### OpenRouter

OpenRouter exposes an embeddings API and documents an embeddings API reference
and model listing.

grepai has explicit support for OpenRouter embedding providers in recent
releases.

Operational characteristics:

- higher throughput and lower local CPU use
- code content is sent off-box to the provider (risk profile must be
  explicit)

### Ollama

Ollama documents embeddings as a first-class capability with model-dependent
vector length.

grepai positions Ollama as the privacy-first local option and documents running
`ollama serve` and pulling a recommended embedding model during installation.

Operational characteristics:

- "code remains on the local machine" privacy profile (still produces
  embeddings on-box)
- requires CPU/GPU resources sized for embedding throughput

repovec's TUI supports switching the provider, but also warns that switching
embedding models/dimensions implies full re-embedding and therefore reindex
time and cost.

## Provisioning and deployment with repovectl and OpenTofu

### CLI shape

A Rust/clap CLI, `repovectl`, wraps OpenTofu to provide a single command
surface:

- `repovectl deploy aws …`
- `repovectl deploy digitalocean …`
- `repovectl deploy hetzner …`
- `repovectl deploy scaleway …`
- `repovectl destroy …`
- `repovectl status …`

Each `deploy` subcommand:

- renders an OpenTofu working directory (bundled templates)
- writes provider configuration, variables, and outputs to a workspace
  directory
- invokes `tofu init`, `tofu apply`
- configures Cloudflare:
  - either (a) tunnel + DNS route, or (b) DNS + Origin CA certificate
    provisioning

OpenTofu's documentation describes that the OpenTofu CLI installs providers
when initializing a working directory, based on declared provider requirements.
It also documents CLI configuration for credentials and provider installation
behaviour.

### Cloudflare domain modes

repovectl supports:

- **subdomain mode**: create records under an existing Cloudflare-managed
  zone
- **new zone mode**: create a new zone in Cloudflare (note: domain purchase
  remains external to Cloudflare DNS automation)

Cloudflare tunnel automation requirements are explicit: Cloudflare documents
creating a tunnel via API and the permissions required (Tunnel edit + DNS edit).

### Bootstrap of the VM appliance

OpenTofu provisions a VM plus initial cloud-init:

- installs the packaged `sysusers.d` and `tmpfiles.d` definitions and lets
  systemd directory directives create service-owned paths, rather than carrying
  an independent imperative account and directory implementation;
- installs:
  - podman
  - cloudflared
  - qdrant container definition (Quadlet)
  - grepai binary
  - repovec binaries and systemd units
- starts `repovec.target`

After boot, the operator SSHs in and completes:

- GitHub device flow login in the TUI
- selection of embedding provider and models (OpenRouter vs Ollama)
- selecting repositories/organizations to index (and webhook enablement
  policy)

This keeps cloud-init deterministic and keeps credentials entry out of IaC
state.
