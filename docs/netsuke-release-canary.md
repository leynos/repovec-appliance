# Netsuke v0.1.0 release-admission canary

This branch replaces the selected quality-gate graph with `Netsukefile` and
runs its `all` target through the exact Netsuke candidate pinned in the
workflow. `all` serializes formatting, linting, testing, and packaging across
the complete workspace. Packaging selects publishable `repovec-core`; internal
path-dependent workspace support crates remain deliberately out of this slice.

The action retains `command: ":"` because v0.1.0 requires a recipe for an
otherwise dependency-only aggregate. This synthetic no-op is intentional and
tracked for removal by `leynos/netsuke#572`.

The explicit empty `targets: []` is also retained because the v0.1.0 schema
requires the top-level key even when this canary is action-only.

The Makefile remains for developer convenience, integration lifecycle gates,
and targets outside this release slice. The canary does not call `make`: its
Netsukefile owns the selected workspace commands directly.
