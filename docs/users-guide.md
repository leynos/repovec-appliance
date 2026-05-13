# Users guide

This guide is for contributors and operators who need to understand the
user-visible behaviour of repovec-appliance and its repository automation. It
focuses on what a user can expect to happen, not on the internal crate layout.

## Documentation gate decisions

When a change reaches continuous integration (CI), the workflow decides whether
documentation validation is required and whether Mermaid diagram validation
should also run. The decision is based on the changed-file list, whether any
documentation-tooling configuration changed, and, for Markdown files, whether
the current file contents contain Mermaid diagrams.

Figure 1. Accessible flow diagram showing how the CI policy decides whether the
documentation gate and Mermaid validation are required from the changed-file
list, including the conservative fallback path used when the list is empty or
malformed.

```mermaid
flowchart TD
    Start[Start
Receive_changed_file_list] --> Validate[Validate_input_list]

    Validate --> IsEmpty{List_empty_or_malformed?}
    IsEmpty -->|Yes| Fallback[Apply_safe_default_policy]
    IsEmpty -->|No| Classify[Classify_changes_by_path]

    Classify --> DocsOnly{Only_documentation_inputs_changed?}
    DocsOnly -->|Yes| RequireDocs[Set docs_gate_required = true]
    DocsOnly -->|No| MixedOrCode[Mixed_or_code_only_changes]

    MixedOrCode --> HasDocs{Any_documentation_inputs_changed?}
    HasDocs -->|Yes| RequireDocs
    HasDocs -->|No| SkipDocs[Set docs_gate_required = false]

    RequireDocs --> CheckNixie{Mermaid_or_conservative_path?}
    CheckNixie -->|Yes| RequireNixie[Set nixie_required = true]
    CheckNixie -->|No| SkipNixie[Set nixie_required = false]

    Fallback --> Output[Emit_policy_output
with_safe_defaults]
    SkipDocs --> SkipNixie

    RequireNixie --> Output
    SkipNixie --> Output

    Output --> End[End
Workflow_consumes_flags]
```

In practice, the current policy behaves as follows:

- If the changed-file list is unavailable, CI runs both the documentation gate
  and Mermaid validation as a safe default.
- If no documentation inputs changed, the documentation gate is skipped.
- If Markdown files changed, the documentation gate runs.
- If documentation-tooling configuration changed, the documentation gate and
  Mermaid validation both run as a conservative default.
- Mermaid validation runs only when one of the changed Markdown files contains
  a Mermaid diagram, or when the workflow takes that conservative fallback.

The CI workflow publishes these decisions as stable flags so the `docs-gate`
job can stay required even when it skips documentation-specific work. When the
workflow takes the conservative Mermaid path because a file could not be read,
it also publishes which files triggered that fallback.

## Qdrant service

repovec-appliance ships Qdrant as an appliance-internal Podman Quadlet.
Operators should treat it as a local dependency of the appliance rather than a
general-purpose network service.

The checked-in Quadlet is installed to
`/etc/containers/systemd/qdrant.container`. It tracks the official Qdrant
`docker.io/qdrant/qdrant:v1` image stream and enables `AutoUpdate=registry` so
the systemd-managed container can participate in Podman's registry-based
auto-update flow within the current major version.

Qdrant's REST and gRPC ports are published only on loopback:

- REST: `127.0.0.1:6333`
- gRPC: `127.0.0.1:6334`

Persistent vector storage lives at `/var/lib/repovec/qdrant-storage` on the
host and is mounted into the container at `/qdrant/storage`. The mount uses an
explicit `:Z` SELinux relabel so the rootful Podman service can write to the
directory on enforcing hosts.

Qdrant requires an API key. On first boot, `repovec-qdrant-api-key.service`
generates a random raw key at `/etc/repovec/qdrant-api-key`, restricts the file
to `repovec:repovec` with mode `0400`, and refreshes the rootful Podman secret
`repovec-qdrant-api-key`. The Qdrant Quadlet injects that Podman secret as
`QDRANT__SERVICE__API_KEY` inside the container.

Operators can inspect service state without printing the key:

```sh
systemctl status repovec-qdrant-api-key.service qdrant.service
journalctl -u repovec-qdrant-api-key.service
stat -c '%U:%G %a %n' /etc/repovec/qdrant-api-key
podman secret inspect repovec-qdrant-api-key
```

Local clients authenticate by reading the key as the `repovec` user and sending
it in Qdrant's `api-key` header:

```sh
sudo -u repovec sh -c \
  'api_key="$(cat /etc/repovec/qdrant-api-key)"
  curl --config - http://127.0.0.1:6333/collections <<EOF
header = "api-key: ${api_key}"
EOF'
```

Requests to Qdrant without the `api-key` header are rejected.

### Troubleshooting

Qdrant Quadlet validation emits structured events with the target
`repovec_core::qdrant_quadlet`. Enable these events with
`RUST_LOG=repovec_core::qdrant_quadlet=info` or an equivalent tracing
subscriber configuration.

The message `validating checked-in qdrant quadlet` confirms that the embedded
`packaging/systemd/qdrant.container` asset is being checked. The message
`qdrant quadlet contract validation succeeded` confirms that the current
Quadlet satisfies the appliance contract.

If validation fails with `qdrant quadlet validation failed: missing image`,
`qdrant quadlet validation failed: image is not fully qualified and pinned`, or
`qdrant quadlet validation failed: unexpected image`, inspect the `image` and
`expected_image` fields. Ensure `Image=` is set exactly to
`docker.io/qdrant/qdrant:v1`.

If validation fails with
`qdrant quadlet validation failed: missing publish port` or
`qdrant quadlet validation failed: publish port is not bound to loopback`,
inspect the `port`, `publish_port`, and `expected_publish_port` fields. Ensure
the Quadlet publishes `127.0.0.1:6333:6333` for REST and `127.0.0.1:6334:6334`
for gRPC.

If validation fails with
`qdrant quadlet validation failed: missing storage mount`,
`qdrant quadlet validation failed: incorrect storage source`, or
`qdrant quadlet validation failed: incorrect storage target`, inspect the
`volume`, `source`, `target`, `expected_source`, and `expected_target` fields.
Ensure the Quadlet mounts `/var/lib/repovec/qdrant-storage:/qdrant/storage:Z`.

If validation fails with
`qdrant quadlet validation failed: missing selinux relabel`, inspect the
`volume` field. Ensure the storage mount ends with the explicit `:Z` relabel
option so Podman can write to the directory on enforcing SELinux hosts.

If validation fails with
`qdrant quadlet validation failed: missing auto-update policy` or
`qdrant quadlet validation failed: incorrect auto-update policy`, inspect the
`auto_update` and `expected_auto_update` fields. Ensure `AutoUpdate=registry`
is present exactly once in the `[Container]` section.

If validation fails with
`qdrant quadlet validation failed: missing api key provisioning dependency` or
`qdrant quadlet validation failed: incorrect api key provisioning dependency`,
inspect the `directive`, `dependency`, and `expected_dependency` fields. Ensure
the `[Unit]` section includes both `Requires=repovec-qdrant-api-key.service`
and `After=repovec-qdrant-api-key.service`.

If validation fails with
`qdrant quadlet validation failed: missing api key secret` or
`qdrant quadlet validation failed: incorrect api key secret`, inspect the
`secret`, `expected_secret`, and `expected_target` fields. Ensure the
`[Container]` section includes
`Secret=repovec-qdrant-api-key,type=env,target=QDRANT__SERVICE__API_KEY`.

If validation fails with
`qdrant quadlet validation failed: inline api key environment is disallowed`,
inspect the redacted `environment` field. Remove any inline
`Environment=QDRANT__SERVICE__API_KEY=...` assignment and inject the API key
through the Podman secret instead.

If validation fails with `qdrant quadlet validation rejected invalid line` or
`qdrant quadlet validation rejected property before section`, inspect the
`line_number` and `line` fields. Ensure every non-comment line is either a
section header or a `Key=Value` property, and that properties appear after a
section header.
