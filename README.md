# repovec-appliance

*A self-hosted VM appliance that turns private GitHub repositories into a
continuously indexed, semantically searchable MCP server.*

Point coding agents at a single HTTPS endpoint to get semantic search,
call-graph tracing, and RPG graph queries across every repository and branch of
interest without sending code to a third-party indexing service.

______________________________________________________________________

## Why repovec-appliance?

- **One endpoint, many repos**: Index dozens of private repositories and
  branches into a single queryable corpus. Agents connect to one MCP URL and
  filter by repo and branch.
- **Semantic + structural search**: Built on
  [grepai](https://github.com/sloganking/grepai), the appliance provides
  embedding-based semantic search, symbol-level call-graph tracing, and RPG
  graph exploration out of the box.
- **Self-hosted infrastructure**: Runs on a VM under operator control.
  Choose between local embeddings (Ollama) for full privacy or remote
  embeddings (OpenRouter) for throughput. Code never leaves the network unless
  remote embeddings are enabled.
- **Hands-off lifecycle**: Branches are discovered automatically, indexed
  continuously, and retired when stale. Qdrant, grepai, and the appliance
  binaries update themselves on a configurable schedule.
- **One-command deployment**: `repovectl deploy` provisions a VM on AWS,
  DigitalOcean, Hetzner, or Scaleway, wires up Cloudflare DNS and TLS, and
  boots into a ready-to-configure TUI.

______________________________________________________________________

## Quick start

### Deploy

```bash
# Provision a VM on the preferred cloud provider
repovectl deploy hetzner \
  --region fsn1 \
  --size cpx31 \
  --domain mcp.example.com \
  --cloudflare-token "$CF_TOKEN"
```

### Configure

```bash
# SSH into the appliance and launch the TUI
ssh repovec@mcp.example.com
```

From the TUI:

1. Complete the GitHub device-flow login (enter the code shown at
   `github.com/login/device`).
2. Select an embedding provider (Ollama for local, OpenRouter for remote).
3. Choose which repositories and organizations to index.
4. Mint an API token for agents.

### Connect an agent

Point any MCP-compatible client at the appliance:

```json
{
  "mcpServers": {
    "repovec": {
      "url": "https://mcp.example.com/mcp",
      "headers": {
        "Authorization": "Bearer <api-token>"
      }
    }
  }
}
```

The agent now has access to `grepai_search`, call-graph tracing, index status,
and RPG graph tools across all indexed repositories.

______________________________________________________________________

## Features

- GitHub OAuth device-flow authentication (no browser needed on the VM)
- Automatic repository and branch discovery with configurable active-branch
  policy
- Bare mirrors and per-branch worktrees for deterministic indexing
- Optional webhook acceleration for near-instant branch tracking
- MCP Streamable HTTP transport with session management
- Bearer-token authentication with scoped permissions and immediate
  revocation
- Cloudflare Tunnel (zero inbound ports) or Origin CA certificate modes
- Operator TUI over SSH for configuration, monitoring, and token management
- Automated rolling updates for Qdrant, grepai, and repovec binaries

______________________________________________________________________

## Learn more

- [Technical design](docs/repovec-appliance-technical-design.md) —
  architecture, constraints, and rationale
- [Roadmap](docs/roadmap.md) — planned features and implementation progress

______________________________________________________________________

## Licence

ISC — see [LICENSE](LICENSE) for details.

______________________________________________________________________

## Contributing

Contributions welcome! Please see [AGENTS.md](AGENTS.md) for coding standards
and workflow guidelines.
