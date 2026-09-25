# Netsuke v0.1.0 release-admission canary

This branch is one of Netsuke's three v0.1.0 release-admission canaries.
Netsuke's release workflow checks this branch out at a pinned commit, builds
the exact Netsuke release candidate, and runs `netsuke`'s `all` target from
this repository's `Netsukefile`. A failure blocks publication of the candidate
only when it exposes a defect in behaviour that v0.1.0 claims to support.

## What the canary exercises

- `all` is a serial aggregate action. It runs `check-fmt`, `lint`, `test`,
  and `package` in declaration order through `dependency_order: serial`, so a
  later gate never starts before an earlier one succeeds.
- Every gate selects the complete workspace: `--workspace`, `--all-targets`,
  and `--all-features` wherever the command accepts them. Nothing is excluded
  by negation, because the manifest has no exclusion flag.
- Warnings are denied in every compiling gate: `RUSTDOCFLAGS='-D warnings'`
  for rustdoc, `-D warnings` for Clippy, and `RUSTFLAGS='-D warnings'` for
  Whitaker and the tests. The denial lives in the recipes, as it does in the
  Makefile, rather than in a manifest setting.
- Whitaker runs unconditionally. The Makefile skips it when the binary is
  missing, but a canary that silently skipped a gate would pass without
  testing it; the release workflow installs Whitaker for this canary.

## Retained boundaries

- `all` keeps `command: ":"`. v0.1.0 requires a recipe even for a
  dependency-only aggregate, so this synthetic no-op is a documented
  compatibility workaround. `leynos/netsuke#572` removes the requirement, and
  the v0.1.1 gate in `leynos/netsuke#597` removes this no-op.
- Packaging selects the publishable `repovec-core` crate only; the internal
  path-dependent workspace crates remain outside this slice.
- The explicit empty `targets: []` is retained because v0.1.0 requires the
  top-level key even when the manifest is action-only.
- The Makefile remains for developer convenience, the integration lifecycle
  gates, and every target outside this slice. No action calls `make`: the
  `Netsukefile` owns the selected workspace commands directly.
