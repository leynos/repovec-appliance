# repovec-appliance roadmap

This roadmap describes the implementation plan for repovec-appliance, a
self-hosted VM appliance that turns private GitHub repositories into a
continuously indexed, multi-branch semantic and graph-queryable corpus exposed
as a remote MCP server over HTTPS. Each phase represents a significant
capability shift; each step is a coherent workstream with a defined delivery
objective; each task is a concrete, measurable unit of work.

See repovec-appliance-technical-design.md for architecture, rationale, and
constraints referenced throughout.

## 1. Foundation and service skeleton

Establish the project structure, the Qdrant vector store, and the systemd
service layout. On completion, the appliance boots into a managed target with
Qdrant running and reachable by local processes.

### 1.1. Project scaffolding and crate structure

Objective: a compilable Rust workspace with binary targets for each daemon and
CLI, a strict lint baseline, and a CI pipeline that gates merges.

- [ ] 1.1.1. Define Cargo workspace with binary crates
  - Create workspace members: `repovecd`, `repovec-mcpd`, `repovec-tui`,
    `repovectl`.
  - Create shared library crate `repovec-core` for common types and
    configuration.
  - Confirm `cargo build` succeeds for all members.
- [ ] 1.1.2. Establish lint and formatting baseline
  - Carry forward the existing `Cargo.toml` lint profile (clippy pedantic,
    panic-prone denials, missing docs).
  - Add `rustfmt.toml` with project conventions.
  - Confirm `cargo clippy` and `cargo fmt --check` pass with zero warnings.
- [ ] 1.1.3. Add CI gating pipeline
  - Configure CI to run `cargo build`, `cargo clippy`, `cargo fmt --check`,
    and `cargo test` on every push.
  - Gate merge on all checks passing.

### 1.2. Qdrant container management

Objective: Qdrant runs as a Podman Quadlet under systemd, bound to localhost
with API-key authentication, and survives host reboots.

- [ ] 1.2.1. Write Podman Quadlet definition for Qdrant
  - Define Quadlet `.container` file targeting the official Qdrant image.
    See repovec-appliance-technical-design.md, "Qdrant under Podman + systemd".
  - Bind Qdrant REST (6333) and gRPC (6334) to `127.0.0.1` only.
  - Mount a persistent volume at `/var/lib/repovec/qdrant-storage`.
  - Enable Podman auto-update label.
- [ ] 1.2.2. Configure Qdrant API-key authentication
  - Generate a random API key at first boot and store it in
    `/etc/repovec/qdrant-api-key`.
  - Pass the key to Qdrant via environment variable.
  - Restrict file permissions to the `repovec` system user.
  - Success criteria: unauthenticated requests to Qdrant are rejected.
- [ ] 1.2.3. Validate Qdrant liveness from Rust
  - Implement a health-check function in `repovec-core` that connects to
    Qdrant gRPC at `localhost:6334` with the stored API key and confirms the
    service is ready.
  - Add an integration test that starts Qdrant and runs the health check.

### 1.3. Systemd service layout

Objective: a `repovec.target` that orchestrates all appliance units with
correct ordering and dependency declarations.

- [ ] 1.3.1. Define `repovec.target` and static unit files
  - Create `repovec.target` that wants `qdrant.service`, `repovecd.service`,
    `repovec-mcpd.service`, and `cloudflared.service`.
    See repovec-appliance-technical-design.md, "Service layout".
  - Write stub unit files for `repovecd.service` and `repovec-mcpd.service`
    with correct `After=` and `Requires=` relationships.
- [ ] 1.3.2. Define template unit for per-repo indexers
  - Create `repovec-grepai@.service` template unit.
  - Configure the unit to run as the `repovec` user with `HOME` set to
    `/var/lib/repovec`.
  - Ensure journald captures all output (no bespoke log files).
- [ ] 1.3.3. Create `repovec` system user and directory layout
  - Create system user `repovec` with home `/var/lib/repovec`.
  - Create directories: `git-mirrors/`, `worktrees/`, and `.grepai/`.
  - Create `/etc/repovec/` with restricted permissions for secrets.
  - Success criteria: `systemctl start repovec.target` succeeds and Qdrant
    becomes reachable.

## 2. Repository lifecycle

Enable GitHub authentication, repository discovery, and local Git
mirror/worktree management. On completion, the appliance can authenticate
with GitHub, discover accessible repositories and branches, and maintain
local worktrees that track remote state.

### 2.1. GitHub device-flow authentication

Objective: the appliance can authenticate with GitHub using the OAuth device
flow without requiring a browser on the VM.

- [ ] 2.1.1. Implement device-flow OAuth client
  - Implement the three-step device flow: request device/user codes, poll for
    token, and handle `slow_down`/`expired_token`/`access_denied` responses.
    See repovec-appliance-technical-design.md, "Authentication: device flow".
  - Store the access token encrypted at rest in `/etc/repovec/`.
  - Success criteria: a test binary completes the device flow and retrieves
    a valid token.
- [ ] 2.1.2. Implement token refresh and expiry handling
  - Detect token expiry and re-initiate the device flow when refresh is not
    available.
  - Log token lifecycle events to journald.
- [ ] 2.1.3. Implement permissions checking
  - After authentication, verify the token's scopes are sufficient for
    repository listing and (optionally) webhook creation.
  - Surface missing permissions as a structured warning for the TUI.

### 2.2. Repository and branch discovery

Objective: repovecd can enumerate accessible repositories and their branches,
determining which branches are active per the configured policy.

- [ ] 2.2.1. Implement repository listing via GitHub API
  - Query the GitHub API for repositories accessible to the authenticated
    user or installation.
  - Support both personal and organisation repositories.
  - Paginate results correctly.
- [ ] 2.2.2. Implement branch listing and active-branch evaluation
  - For each repository, list branches and evaluate the active branch policy:
    default branch always active, branches with recent pushes, branches
    referenced by open pull requests, and LRU eviction beyond the per-repo
    cap. See repovec-appliance-technical-design.md, "Active branch policy".
  - Store the desired state (set of active branches per repo) for the
    reconciler.
- [ ] 2.2.3. Implement periodic reconciliation scheduler
  - Run reconciliation on a configurable interval.
    See repovec-appliance-technical-design.md, "Discovery and continuous
    monitoring".
  - Compare desired state against actual state (existing mirrors, worktrees,
    and indexer units) and emit a diff of actions to take.
  - Success criteria: the reconciler correctly detects added repos, removed
    repos, added branches, and removed branches in a test scenario.

### 2.3. Git mirror and worktree management

Objective: the appliance maintains bare mirrors and per-branch worktrees that
faithfully track the remote repository state.

- [ ] 2.3.1. Implement bare mirror creation and fetch
  - Clone repositories as bare mirrors to
    `/var/lib/repovec/git-mirrors/{owner}/{repo}.git`.
    See repovec-appliance-technical-design.md, "Worktrees and checkout layout".
  - Implement incremental fetch for existing mirrors.
- [ ] 2.3.2. Implement per-branch worktree lifecycle
  - Create worktrees at `/var/lib/repovec/worktrees/{owner}/{repo}/{branch}/`
    from the bare mirror.
  - On update, hard-reset the worktree to the target ref to avoid drift.
  - On branch deactivation, remove the worktree and prune.
- [ ] 2.3.3. Wire reconciler to mirror and worktree operations
  - Connect the reconciliation diff (from 2.2.3) to mirror clone/fetch and
    worktree add/remove operations.
  - Ensure operations are idempotent: re-running reconciliation with no
    remote changes produces no side effects.
  - Success criteria: end-to-end test provisioning a repo with two branches
    and verifying the expected directory layout exists.

### 2.4. Webhook acceleration

Objective: when the user grants webhook permissions, push events trigger
immediate reconciliation rather than waiting for the next polling interval.

- [ ] 2.4.1. Implement webhook registration via GitHub API
  - Register `push` and `create` webhooks on repositories where the token
    has sufficient scope.
    See repovec-appliance-technical-design.md, "Webhook events and how they
    map to workspaces".
  - Store webhook secrets encrypted in `/etc/repovec/`.
- [ ] 2.4.2. Implement webhook ingestion endpoint in repovecd
  - Accept inbound webhook payloads on a local-only HTTP endpoint.
  - Validate HMAC signatures against the stored secret.
  - Parse `push` events to detect `created` and `deleted` flags for branch
    lifecycle.
- [ ] 2.4.3. Wire webhook events to the reconciler
  - On a valid webhook event, trigger an immediate targeted reconciliation
    for the affected repository.
  - Ensure concurrent webhook deliveries and scheduled reconciliation do
    not race. Requires 2.2.3.

## 3. Continuous indexing

Connect grepai indexers to the worktree lifecycle so that every active branch
is continuously indexed. On completion, semantic search, call-graph tracing,
and RPG graph queries return results for all active branches.

### 3.1. grepai workspace and project configuration

Objective: the appliance programmatically manages grepai workspace
configuration so that each repository maps to a workspace and each branch
maps to a project within it.

- [ ] 3.1.1. Implement workspace YAML generation
  - Generate entries in `/var/lib/repovec/.grepai/workspace.yaml` using the
    canonical mapping: workspace = repository, project = branch.
    See repovec-appliance-technical-design.md, "Canonical mapping".
  - Configure store and embedder settings per the operator's chosen provider
    (OpenRouter or Ollama).
- [ ] 3.1.2. Implement project add and remove operations
  - Add a project entry when a branch worktree is created.
  - Remove a project entry when a branch worktree is retired.
  - Ensure path prefixing uses `workspaceName/projectName/relativePath` for
    index isolation.
- [ ] 3.1.3. Validate workspace configuration round-trip
  - Write an integration test that generates a workspace configuration,
    invokes `grepai` to parse it, and confirms the expected projects are
    visible.
  - Success criteria: grepai accepts the generated configuration without
    errors.

### 3.2. Per-branch indexer lifecycle

Objective: each active branch has a dedicated grepai watcher managed as a
systemd unit, started and stopped in response to reconciliation.

- [ ] 3.2.1. Implement indexer unit instantiation
  - When a branch becomes active, instantiate
    `repovec-grepai@{owner}-{repo}-{branch}.service` via systemd.
  - Configure the unit to run `grepai watch` against the branch worktree
    directory. Requires 1.3.2.
- [ ] 3.2.2. Implement indexer unit teardown
  - When a branch is deactivated, stop and disable the corresponding
    systemd unit.
  - Optionally retain index data for a configurable grace period before
    purging.
- [ ] 3.2.3. Implement indexer health monitoring
  - Poll indexer unit status via systemd and surface failures.
  - Automatically restart failed indexers with exponential back-off.
  - Expose indexer status per repo and branch for the TUI (via the
    repovecd local Unix socket API).

### 3.3. Reconciliation loop integration

Objective: the full reconciliation loop drives mirror updates, worktree
management, workspace configuration, and indexer lifecycle as a single
coherent operation.

- [ ] 3.3.1. Compose the full reconciliation pipeline
  - Chain: discover repos and branches (2.2), update mirrors and worktrees
    (2.3), update workspace configuration (3.1), and start/stop indexers
    (3.2) into a single reconciliation pass.
  - Ensure each stage is individually retriable on transient failure.
    Requires 2.2.3, 2.3.3, 3.1.2, 3.2.1, and 3.2.2.
- [ ] 3.3.2. Add reconciliation observability
  - Log each reconciliation pass with a summary of actions taken (repos
    added/removed, branches activated/deactivated, indexers started/stopped).
  - Expose a reconciliation timestamp and action count via the local status
    API.

### 3.4. Embeddings provider configuration

Objective: the appliance supports both OpenRouter and Ollama as embedding
providers, selectable by the operator, with clear trade-off communication.

- [ ] 3.4.1. Implement OpenRouter provider configuration
  - Accept an OpenRouter API key and model selection.
    See repovec-appliance-technical-design.md, "OpenRouter".
  - Write the provider configuration into grepai workspace settings.
- [ ] 3.4.2. Implement Ollama provider configuration
  - Configure grepai to use a local Ollama instance.
    See repovec-appliance-technical-design.md, "Ollama".
  - Optionally manage an Ollama Podman container alongside Qdrant.
- [ ] 3.4.3. Implement provider switching with re-index warning
  - When the operator changes provider or model, warn that a full re-embed
    is required and estimate the cost in time.
  - Trigger a full re-index on confirmation.

## 4. Remote MCP access

Expose the indexed corpus to remote agents via an authenticated MCP endpoint
over HTTPS. On completion, agents can connect to a public HTTPS URL and issue
semantic search, call-graph trace, and RPG graph queries scoped by repository
and branch.

### 4.1. MCP Streamable HTTP transport

Objective: `repovec-mcpd` implements the MCP Streamable HTTP transport
specification, handling sessions, origin validation, and authentication.

- [ ] 4.1.1. Implement Streamable HTTP endpoint
  - Implement a single HTTP endpoint supporting GET and POST as defined by
    the MCP transport specification.
    See repovec-appliance-technical-design.md, "MCP transport and security
    invariants".
  - Support `application/json` single-response and `text/event-stream`
    SSE modes.
- [ ] 4.1.2. Implement session management
  - Generate and track `Mcp-Session-Id` headers.
  - Map each session to a dedicated `grepai mcp-serve` subprocess.
  - Clean up subprocesses on session termination or timeout.
- [ ] 4.1.3. Implement origin validation
  - Validate the `Origin` header against a configured allowlist.
  - Reject requests with absent or non-matching origins on browser-capable
    clients.
  - Success criteria: requests from unlisted origins receive a 403 response.

### 4.2. grepai stdio bridge

Objective: `repovec-mcpd` bridges Streamable HTTP JSON-RPC to grepai's
stdio-based MCP server transparently.

- [ ] 4.2.1. Implement subprocess lifecycle management
  - On `InitializeRequest`, spawn `grepai mcp-serve` with the correct
    `HOME` environment variable (`/var/lib/repovec`).
    See repovec-appliance-technical-design.md, "Bridging to grepai MCP
    tools".
  - Forward JSON-RPC messages to stdin (newline-delimited).
  - Read responses from stdout and route them to the correct HTTP response.
- [ ] 4.2.2. Implement transparent tool proxying
  - Pass all tool calls through without interpretation so that grepai tool
    additions are automatically available.
  - Confirm the full grepai tool surface is accessible: `grepai_search`,
    trace tools, `grepai_index_status`, and RPG graph tools.
- [ ] 4.2.3. Handle subprocess failure and recovery
  - Detect subprocess crashes and return an MCP error response to the
    client.
  - Allow session re-establishment after a subprocess failure.

### 4.3. Token minting and revocation

Objective: operators can create, list, and revoke API tokens that control
access to the MCP endpoint.

- [ ] 4.3.1. Implement token storage backend
  - Store tokens hashed with Argon2id alongside metadata: name,
    `created_at`, `last_used_at`, optional expiry, and scopes.
    See repovec-appliance-technical-design.md, "Token minting and
    revocation".
  - Use an SQLite database in `/var/lib/repovec/` for token storage.
- [ ] 4.3.2. Implement token minting
  - Generate 256-bit random tokens, display the plaintext once, and store
    the hash.
  - Support scope assignment: read, search, trace, admin.
- [ ] 4.3.3. Implement token validation and revocation
  - Validate `Authorization: Bearer <token>` on all non-initialization
    requests.
  - Implement immediate revocation by setting `revoked_at`.
  - Update `last_used_at` on each successful authentication.
  - Success criteria: a revoked token is rejected on the next request.

### 4.4. Cloudflare edge integration

Objective: the MCP endpoint is reachable via a public HTTPS URL managed by
Cloudflare, with no inbound ports opened on the VM.

- [ ] 4.4.1. Implement Cloudflare Tunnel mode
  - Configure `cloudflared` to route a hostname to the local
    `repovec-mcpd` port.
    See repovec-appliance-technical-design.md, "Cloudflare edge integration".
  - Automate tunnel creation and DNS CNAME record via the Cloudflare API.
- [ ] 4.4.2. Implement Cloudflare Origin CA mode
  - As an alternative to tunnels, provision an Origin CA certificate via the
    Cloudflare API and bind `repovec-mcpd` with that certificate.
  - Configure Cloudflare for "Full (strict)" TLS mode.
- [ ] 4.4.3. Implement DNS configuration for both modes
  - Support subdomain mode (records under an existing zone) and new-zone
    mode.
  - Validate DNS propagation after record creation.
  - Success criteria: an HTTPS request to the configured hostname reaches
    `repovec-mcpd` and returns a valid MCP response.

## 5. Operator experience

Provide a terminal-based operator interface for configuration, monitoring,
and token management. On completion, operators can SSH into the appliance and
manage all aspects of the system through an interactive TUI.

### 5.1. TUI framework and navigation

Objective: `repovec-tui` provides a navigable ratatui interface connected to
repovecd's local Unix socket API.

- [ ] 5.1.1. Scaffold ratatui application
  - Set up the ratatui event loop, main layout, and screen navigation.
  - Connect to repovecd's local Unix socket for status and control.
- [ ] 5.1.2. Implement status dashboard
  - Display a summary: authenticated user, number of indexed repos,
    active branches, indexer health, and Qdrant status.
  - Auto-refresh on a configurable interval.

### 5.2. GitHub device-flow interface

Objective: the TUI guides the operator through the GitHub device-flow login
interactively.

- [ ] 5.2.1. Implement device-flow login screen
  - Display the user code and verification URL.
  - Show a polling indicator while waiting for authorisation.
  - Display success or failure and surface any missing-permissions warnings
    from 2.1.3. Requires 2.1.1.

### 5.3. Configuration and monitoring views

Objective: the operator can configure repositories, branches, and embedding
providers and monitor indexing status.

- [ ] 5.3.1. Implement repository and branch configuration view
  - List discovered repositories with toggle controls for indexing.
  - Display per-repo branch lists with active/inactive indicators.
  - Allow manual override of the active-branch policy per repository.
- [ ] 5.3.2. Implement embedding provider configuration view
  - Allow selection between OpenRouter and Ollama.
  - Accept API keys and model selection.
  - Display the re-index warning when changing providers. Requires 3.4.3.
- [ ] 5.3.3. Implement indexer status view
  - Display per-branch indexer status: running, failed, index age, and
    document count.
  - Provide a manual reconciliation trigger. Requires 3.2.3.

### 5.4. Token management interface

Objective: the operator can mint, list, and revoke MCP access tokens from the
TUI.

- [ ] 5.4.1. Implement token management view
  - List existing tokens with name, scopes, creation date, last used date,
    and revocation status.
  - Provide controls to mint new tokens and revoke existing ones.
    Requires 4.3.2 and 4.3.3.
  - Display the plaintext token exactly once after minting, with a warning
    that it will not be shown again.

## 6. Automated provisioning

Enable one-command deployment of the appliance to cloud providers via
`repovectl`. On completion, an operator can provision a fully configured VM
with a single CLI invocation.

### 6.1. repovectl CLI scaffolding

Objective: `repovectl` provides a clap-based CLI that wraps OpenTofu for
multi-provider deployment.

- [ ] 6.1.1. Implement CLI argument parsing
  - Define subcommands: `deploy aws`, `deploy digitalocean`,
    `deploy hetzner`, `deploy scaleway`, `destroy`, and `status`.
    See repovec-appliance-technical-design.md, "CLI shape".
  - Validate required arguments per provider (region, instance size,
    Cloudflare API token, domain).
- [ ] 6.1.2. Implement OpenTofu workspace management
  - Render an OpenTofu working directory from bundled templates.
  - Write provider configuration, variables, and outputs.
  - Invoke `tofu init` and `tofu apply` with structured output capture.

### 6.2. OpenTofu provider templates

Objective: deployment templates exist for each supported cloud provider and
produce a consistent VM with the appliance pre-installed.

- [ ] 6.2.1. Write AWS deployment template
  - Provision an EC2 instance with security group, SSH key, and cloud-init.
- [ ] 6.2.2. Write DigitalOcean deployment template
  - Provision a Droplet with firewall, SSH key, and cloud-init.
- [ ] 6.2.3. Write Hetzner deployment template
  - Provision a Hetzner Cloud server with firewall, SSH key, and cloud-init.
- [ ] 6.2.4. Write Scaleway deployment template
  - Provision a Scaleway instance with security group, SSH key, and
    cloud-init.

### 6.3. Cloud-init bootstrap

Objective: cloud-init brings a fresh VM from bare OS to a running
`repovec.target` without manual intervention.

- [ ] 6.3.1. Write cloud-init configuration
  - Create the `repovec` system user and directory layout (as in 1.3.3).
    See repovec-appliance-technical-design.md, "Bootstrap of the VM
    appliance".
  - Install Podman, cloudflared, grepai, and repovec binaries.
  - Install systemd units and Quadlet definitions.
  - Start `repovec.target`.
  - Success criteria: a freshly provisioned VM is reachable via SSH with
    `repovec-tui` available and Qdrant healthy.
- [ ] 6.3.2. Implement Cloudflare provisioning in deploy flow
  - Automate tunnel or Origin CA setup as part of `repovectl deploy`.
  - Write the selected Cloudflare mode configuration to the VM.

### 6.4. Update lifecycle

Objective: all appliance components can be updated automatically with safe
sequencing that avoids index corruption.

- [ ] 6.4.1. Implement `repovec-upgrade.timer` and upgrade script
  - Define a systemd timer that triggers the upgrade sequence on a
    configurable schedule.
    See repovec-appliance-technical-design.md, "Automatic updates and safe
    rollouts".
  - Sequence: pause indexers, upgrade Qdrant via `podman auto-update`,
    upgrade grepai via `grepai update`, upgrade repovec binaries, resume
    indexers, and run reconciliation.
- [ ] 6.4.2. Implement rollback on upgrade failure
  - Detect upgrade failures at each stage and halt the sequence.
  - Restore the previous binary version and resume indexers.
  - Log the failure for operator review.
