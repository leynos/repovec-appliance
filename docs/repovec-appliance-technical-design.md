# repovec-appliance technical design

## Problem statement and design goals

repovec-appliance is a self-hosted VM appliance that turns a user’s private repositories on entity["company","GitHub","code hosting platform"] into a continuously indexed, multi-branch semantic and graph-queryable corpus, exposed as a remote Model Context Protocol (MCP) server over HTTPS. The core interaction model is:

- user authorizes the appliance (device flow) to access repos/branches and (optionally) create webhooks
- appliance clones repos, creates per-branch worktrees, and keeps them current
- grepai runs indexers (semantic embeddings + symbol/callgraph/RPG graph) into a store backed by entity["company","Qdrant","vector database"]
- users (or agents) talk to a single MCP HTTPS endpoint, filter queries by repo + branch, and get:
  - semantic search results
  - call graph tracing (callers/callees/graph)
  - RPG graph interrogation (search/fetch/explore)

grepai already provides (a) daemonised indexing (`grepai watch`) and (b) MCP tool exposure (`grepai mcp-serve`) with search, trace, index status, and RPG graph tools. citeturn2search13turn4search1turn1search10 The appliance’s job is to operationalize this at “many repos + many branches”, add lifecycle management, and add hardened remote access with token minting/revocation, Cloudflare-managed DNS/TLS, and a TUI configuration surface.

A local code snapshot of grepai was supplied as an auxiliary reference. fileciteturn0file0

### Non-goals

- Not a full managed repo host (it indexes repositories; it does not replace GitHub).
- Not a general-purpose “coding agent” runtime (it serves retrieval/graph context; the agent runs elsewhere).
- Not a multi-tenant SaaS; appliance is single-owner (with optional multiple issued API tokens).

## High-level architecture

The appliance is composed of five long-running concerns, each mapped to systemd units and explicit data directories:

### Control plane

A Rust daemon, `repovecd`, provides:

- GitHub device-flow login, token storage/refresh and permissions checks citeturn16view0
- repository/branch discovery (polling), plus optional webhook registration and webhook ingestion citeturn11search1turn11search18turn14view1turn14view0
- creation/removal of:
  - bare mirrors and per-branch worktrees
  - grepai workspaces and workspace projects
  - systemd units for grepai indexers
- health and status API used by the TUI (over a local-only Unix socket)

### Data plane

- grepai indexers:
  - `grepai watch` builds and maintains embeddings and graph indices continuously citeturn2search13turn5search6
  - workspaces allow modelling {repo} as a workspace and {branch} as projects inside it, with `--workspace` / `--project` query scoping and cross-project search. citeturn2search3turn5search10turn5search1
- Qdrant:
  - runs locally on the VM (Podman container) as the vector store for grepai
  - bound to localhost/private interface and protected with API keys (and TLS when not strictly local). citeturn5search13turn2search7turn10search6

### Remote MCP endpoint

A Rust service, `repovec-mcpd`, exposes an MCP endpoint over Streamable HTTP transport (single endpoint supporting GET and POST), implements origin validation, sessions, and authentication as required by the MCP transport specification. citeturn17view0turn0search30

Because grepai’s built-in MCP server is stdio transport (`grepai mcp-serve`), and is designed for local agent integrations, `repovec-mcpd` acts as a transport and security adapter:

- externally: Streamable HTTP MCP over HTTPS
- internally: one `grepai mcp-serve` subprocess per MCP session (or per client), with JSON-RPC bridged between HTTP and stdio

grepai’s MCP tool surface includes:
- `grepai_search` (semantic search, includes RPG context when enabled)
- trace tools (`grepai_trace_callers`, `grepai_trace_callees`, `grepai_trace_graph`)
- `grepai_index_status`
- RPG graph tools (`grepai_rpg_search`, `grepai_rpg_fetch`, `grepai_rpg_explore`) citeturn4search1turn1search10

This keeps “graphing semantics” identical to grepai, rather than reimplementing them.

### Edge networking, DNS, and TLS

The recommended exposure mechanism is entity["company","Cloudflare","cdn and dns provider"] Tunnel:

- `cloudflared` maintains outbound tunnel connections to Cloudflare, and Cloudflare routes a hostname to that tunnel via DNS records. citeturn0search7turn0search11turn0search23
- tunnel creation and DNS routing can be fully automated via the Cloudflare API; Cloudflare documents required token permissions for the “Create a tunnel (API)” flow. citeturn0search3turn0search11

This approach avoids exposing the VM directly to the Internet (no inbound 443 needed), while still providing a public HTTPS endpoint with Cloudflare-managed TLS at the edge. citeturn0search7turn0search23

As an alternative (when tunnels are undesired), the appliance can run a public-facing reverse proxy (or `repovec-mcpd` directly on 443) behind Cloudflare’s reverse proxy using Cloudflare Origin CA certificates. Cloudflare documents Origin CA certificate creation and also exposes Origin CA certificate APIs. citeturn9search3turn9search7

### Operator interface

- `repovec-tui`: a ratatui TUI over SSH, used to:
  - run the GitHub device flow
  - configure embedder/store choices
  - view repo/branch indexing status
  - mint/revoke MCP access tokens
  - trigger reconciliation and upgrades

## GitHub integration and repository lifecycle

### Authentication: device flow

repovec-appliance uses GitHub’s OAuth device flow so the VM can be configured via SSH without a browser on-box:

- request device/user codes via `POST https://github.com/login/device/code`
- user enters the shown code at `https://github.com/login/device`
- appliance polls `POST https://github.com/login/oauth/access_token` until approval or expiry, respecting the server-provided minimum interval to avoid `slow_down` errors. citeturn16view0

GitHub explicitly indicates the device flow does not require the OAuth app `client_secret` (device flow uses `client_id` + device code + grant type). citeturn16view0

### Discovery and continuous monitoring

repovecd maintains correctness via a reconcile-first model:

- **Periodic reconciliation** (authoritative):
  - list accessible repositories
  - for each repository, list branches and determine “active branches”
  - ensure local clones/worktrees/workspace projects match desired set
- **Webhook acceleration** (optional):
  - on push/create/delete activity, update immediately and avoid waiting for the next reconcile

This split matters because not every desired event is reliably available via a single GitHub webhook, and webhook delivery can be disrupted; periodic reconciliation preserves eventual correctness.

### Webhook events and how they map to workspaces

If the user grants scopes/permissions sufficient to register webhooks, the appliance configures:

- `push` events:
  - GitHub documents that push events include branch pushes and also include booleans `created` and `deleted` indicating whether the push created or deleted the ref. citeturn14view1
  - repovecd uses this to:
    - detect new branches (`created=true`) and provision branch worktrees/projects
    - detect branch deletions (`deleted=true`) and retire branch worktrees/projects (subject to retention policy)
- `create` events:
  - GitHub documents `create` fires when a branch or tag is created, with `ref` and `ref_type` (`branch`/`tag`). citeturn14view0
  - this can be used as an earlier signal than the first push, but `push.created` already covers most “new branch” workflows

For organisation-wide automation, GitHub provides organisation webhooks and notes that OAuth app tokens (and classic PATs) need `admin:org_hook` scope to create them. citeturn11search18turn11search1 This is useful when you want “automatically index new repos created in the org” without manually configuring each repository, but you still keep polling as the safety net.

image_group{"layout":"carousel","aspect_ratio":"16:9","query":["GitHub OAuth device flow diagram","Cloudflare Tunnel request flow diagram"],"num_per_query":1}

## Workspace model and branch indexing strategy

### Canonical mapping

The appliance models:

- **Workspace = repository**
- **Project = branch**

This aligns with grepai’s multi-project workspace capabilities (workspace configuration includes store/embedder and a list of project entries) and grepai’s ability to search across projects with `--workspace` and optionally scope with `--project`. citeturn5search10turn2search3

grepai’s workspace configuration is stored globally in `~/.grepai/workspace.yaml`. citeturn5search10turn2search3 The appliance runs grepai as a dedicated system user (e.g. `repovec`), so workspace config lives in that user’s home (e.g. `/var/lib/repovec/.grepai/workspace.yaml`).

grepai documents path prefixing for workspace isolation as `workspaceName/projectName/relativePath`. citeturn2search3 repovec uses this to safely index multiple branches into a shared store without collisions.

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

This makes branch indexing deterministic and avoids “branch switches in place” that can confuse file watchers.

grepai has explicit, evolving support for git worktrees and multi-worktree watch/daemon behaviour (including worktree detection utilities and multi-worktree improvements noted in releases). citeturn2search16turn5search5turn1search10 The appliance leverages that where possible, but it does not require it (it can run per-branch watchers if needed).

### Active branch policy

Indexing every branch forever becomes expensive (storage, embedding churn, and watch CPU). repovec therefore defines an “active branch set” policy:

- always index default branch
- index any branch with pushes in the last *N* days
- optionally index branches referenced by open pull requests
- cap total indexed branches per repo (LRU eviction beyond cap)

This policy is fully configurable in the TUI; the reconcile loop applies it to add/remove projects and start/stop corresponding indexers.

## MCP HTTPS endpoint and authentication

### MCP transport and security invariants

repovec-mcpd implements MCP Streamable HTTP transport because MCP defines Streamable HTTP as the standard remote transport and describes requirements including:

- a single HTTP endpoint supporting GET and POST
- Origin validation to mitigate DNS rebinding
- binding to localhost when running locally
- authentication for all connections citeturn17view0turn0search30

repovec-mcpd follows the MCP session mechanism (`Mcp-Session-Id` header) so it can map a session to a dedicated `grepai mcp-serve` subprocess and cleanly terminate sessions. citeturn17view0

### Bridging to grepai MCP tools

grepai’s built-in MCP server communicates via stdio and exposes the full grepai tool surface, including RPG graph tools. citeturn4search1turn1search10 repovec-mcpd bridges Streamable HTTP JSON-RPC <-> stdio JSON-RPC:

- on `InitializeRequest`, spawn:
  - `grepai mcp-serve` (with environment set to the grepai system user’s HOME so `~/.grepai/workspace.yaml` resolves)
- for each incoming JSON-RPC message:
  - forward to stdin (newline-delimited, without embedded newlines as per stdio transport expectations) citeturn17view0turn4search1
- stream responses back to the client using either:
  - `application/json` (single response) or
  - `text/event-stream` (SSE stream), as allowed by Streamable HTTP transport citeturn17view0

This design intentionally avoids “re-implement grepai semantics” and therefore preserves:
- hybrid search behaviour
- trace output shape and depth behaviour
- RPG graph traversal semantics
- any future grepai MCP tool additions (the proxy can be designed to be largely transparent)

### Token minting and revocation

repovec provides authentication at the MCP endpoint independent of GitHub credentials:

- **API tokens** are random, high-entropy secrets (e.g. 256-bit), shown once at creation.
- tokens are stored hashed (Argon2id) with metadata: name, created_at, last_used_at, optional expiry, scopes (read/search/trace/admin).
- revocation is immediate: set revoked_at and reject thereafter.

To align with MCP’s emphasis on proper authentication for remote servers and to reduce exposure to CSRF/DNS rebinding vectors, repovec-mcpd requires:

- `Authorization: Bearer <token>` on all non-initialization requests
- strict `Origin` allowlist (configured hostnames only), rejecting absent/incorrect origins on browser-capable clients, as MCP recommends for Streamable HTTP servers. citeturn17view0

### Cloudflare edge integration

With Cloudflare Tunnel:

- DNS is a CNAME to the tunnel UUID (`<UUID>.cfargotunnel.com`) and is only valid within the same Cloudflare account. citeturn0search11
- cloudflared connects outbound; this avoids exposing Qdrant or internal APIs and reduces attack surface. citeturn0search7turn0search23
- automation uses a Cloudflare API token with Tunnel edit + DNS edit privileges. citeturn0search3

If the “direct origin” mode is chosen, the appliance provisions Cloudflare Origin CA certificates (dashboard or API) and binds the MCP server with that certificate/key, using Cloudflare’s “Full (strict)” TLS model. citeturn9search3turn9search7

## Systemd, Podman/Qdrant, and update lifecycle

### Service layout

Systemd manages the appliance lifecycle via a dedicated target:

- `repovec.target`
  - wants: `qdrant.service` (Podman), `repovecd.service`, `repovec-mcpd.service`, `cloudflared.service`
  - wants: per-repo indexers `repovec-grepai@<repo>.service`

Key service properties:

- indexers run as unprivileged user (e.g. `repovec`) with fixed HOME
- tight filesystem permissions on:
  - `/var/lib/repovec/` (repos, worktrees, grepai indices)
  - `/etc/repovec/` (config and secrets)
- journald logging for all units, no bespoke log files

### Qdrant under Podman + systemd

The appliance manages Qdrant via Podman + systemd. Podman’s documentation now prefers Quadlet files for systemd-managed containers; `podman generate systemd` is explicitly described as deprecated in favour of Quadlets. citeturn9search0turn9search19

Qdrant networking assumptions:

- Qdrant exposes REST on 6333 and gRPC on 6334; Qdrant’s docs show gRPC configured at `service.grpc_port: 6334` and typical docker invocation publishing both ports. citeturn10search0turn10search2
- grepai’s configuration defaults show Qdrant endpoint `localhost` and default port `6334`, consistent with preferring Qdrant’s gRPC port. citeturn2search0

Security controls:

- Qdrant supports a static API key; Qdrant recommends API key auth and also recommends binding to localhost/private interfaces to prevent unauthenticated external access. citeturn2search7turn5search13
- In appliance mode, Qdrant binds to 127.0.0.1 (or a private interface) and is never exposed publicly; callers are local processes only.

### Automatic updates and safe rollouts

There are three independently versioned artefacts:

- Qdrant container image
- grepai binary
- repovec binaries

**Qdrant updates** use Podman auto-update:

- Podman can auto-update containers when configured for auto updates and run under systemd.
- Podman ships a `podman-auto-update.service` and a `podman-auto-update.timer` that triggers daily by default. citeturn9search5turn9search1
- The container/unit must opt in using the auto-update policy (documented in Podman systemd integration). citeturn9search4turn9search1

**grepai updates** can be driven by grepai itself:

- grepai provides `grepai update` which fetches the latest release from GitHub, verifies checksum integrity, and replaces the current binary. citeturn4search5turn5search11

repovec implements update policy as configurable systemd timers:

- `repovec-upgrade.timer` can:
  - pause indexers
  - upgrade Qdrant via `podman auto-update` + restart
  - upgrade grepai via `grepai update`
  - upgrade repovec via package or signed tarball update
  - resume indexers and reconcile state

This sequencing avoids embedding during store migrations and keeps the appliance in a coherent state.

## Embeddings configurability: OpenRouter vs Ollama

repovec exposes a single “embedding provider” configuration that is then written into grepai workspace/store configuration.

### OpenRouter

entity["company","OpenRouter","model routing api platform"] exposes an embeddings API and documents an embeddings API reference and model listing. citeturn1search13

grepai has explicit support for OpenRouter embedding providers in recent releases. citeturn2search16turn1search10turn1search4

Operational characteristics:

- higher throughput and lower local CPU use
- code content is sent off-box to the provider (risk profile must be explicit)

### Ollama

entity["company","Ollama","local model runtime"] documents embeddings as a first-class capability with model-dependent vector length. citeturn2search19

grepai positions Ollama as the privacy-first local option and documents running `ollama serve` and pulling a recommended embedding model during installation. citeturn4search5turn1search1

Operational characteristics:

- “code stays on your machine” privacy profile (still produces embeddings on-box)
- requires CPU/GPU resources sized for embedding throughput

repovec’s TUI supports switching the provider, but also warns that switching embedding models/dimensions implies full re-embedding and therefore reindex time and cost.

## Provisioning and deployment with repovectl and OpenTofu

### CLI shape

A Rust/clap CLI, `repovectl`, wraps OpenTofu to provide a single command surface:

- `repovectl deploy aws …`
- `repovectl deploy digitalocean …`
- `repovectl deploy hetzner …`
- `repovectl deploy scaleway …`
- `repovectl destroy …`
- `repovectl status …`

Each `deploy` subcommand:

- renders an OpenTofu working directory (bundled templates)
- writes provider configuration, variables, and outputs to a workspace directory
- invokes `tofu init`, `tofu apply`
- configures Cloudflare:
  - either (a) tunnel + DNS route, or (b) DNS + Origin CA certificate provisioning

OpenTofu’s documentation describes that the OpenTofu CLI installs providers when initializing a working directory, based on declared provider requirements. citeturn9search2turn9search10 It also documents CLI configuration for credentials and provider installation behaviour. citeturn9search6

### Cloudflare domain modes

repovectl supports:

- **subdomain mode**: create records under an existing Cloudflare-managed zone
- **new zone mode**: create a new zone in Cloudflare (note: domain purchase remains external to Cloudflare DNS automation)

Cloudflare tunnel automation requirements are explicit: Cloudflare documents creating a tunnel via API and the permissions required (Tunnel edit + DNS edit). citeturn0search3turn0search11

### Bootstrap of the VM appliance

OpenTofu provisions a VM plus initial cloud-init:

- creates `repovec` system user and directories
- installs:
  - podman
  - cloudflared
  - qdrant container definition (Quadlet)
  - grepai binary
  - repovec binaries and systemd units
- starts `repovec.target`

After boot, the operator SSHs in and completes:

- GitHub device flow login in the TUI citeturn16view0
- selection of embedding provider and models (OpenRouter vs Ollama) citeturn1search13turn2search19turn4search5
- selecting repositories/organisations to index (and webhook enablement policy)

This keeps cloud-init deterministic and keeps credentials entry out of IaC state.

