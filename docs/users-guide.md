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

    Classify --> DocsOnly{Only_docs_paths_changed?}
    DocsOnly -->|Yes| RequireDocs[Set docs_gate_required = true]
    DocsOnly -->|No| MixedOrCode[Mixed_or_code_only_changes]

    MixedOrCode --> HasDocs{Any_docs_or_mermaid_docs_changed?}
    HasDocs -->|Yes| RequireDocs
    HasDocs -->|No| SkipDocs[Set docs_gate_required = false]

    RequireDocs --> CheckNixie{Mermaid_docs_changed?}
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
