# Architectural decision record (ADR) 002: Rationalize systemd and Quadlet analysis

## Status

Proposed. Merging the pull request that introduces this record accepts the
spike and migration direction. Parser adoption remains conditional on the
recorded compatibility evidence.

## Date

2026-08-29.

## Context and problem statement

The appliance currently contains two purpose-built parsers for related
configuration languages:

- `systemd_units::ParsedUnit` parses checked-in native systemd units; and
- `qdrant_quadlet::ParsedQuadlet` parses the Qdrant Podman Quadlet.

Both parsers flatten source into a section-to-directive-to-values map. This is
sufficient for the exact checked-in examples, but it does not model the full
systemd syntax or effective-value rules. Important omitted concepts include
line continuation, source ordering, empty assignments that reset previous
values, repeated sections, source spans, raw versus decoded values, and drop-in
provenance.

The validators then treat their approximations as authoritative. Continuous
integration asks repovec's parser whether repovec's units satisfy repovec's
rules, but does not yet ask the systemd or Podman consumers whether those files
are valid.

Existing Rust libraries cover the source-model responsibilities:

- [`systemd-unit-edit`](https://crates.io/crates/systemd-unit-edit) provides a
  lossless native unit parser and editor; and
- [`quadlet-lens`](https://crates.io/crates/quadlet-lens) provides source-aware,
  typed, version-aware Quadlet parsing and rendering.

Both libraries are young enough that adoption requires evidence rather than an
unqualified dependency switch.

## Decision drivers

- The project must not maintain a general systemd or Quadlet parser.
- Source order, resets, continuations, spans, and repeated directives matter to
  policy correctness.
- Actual systemd and Podman consumers must provide an independent validity
  oracle.
- Project policy must remain testable without installing or invoking systemd.
- Live manager state and static source analysis are different questions.
- Future indexer lifecycle work must avoid parsing `systemctl` display output.

## Requirements

### Functional requirements

- Native unit and Quadlet source can be queried through one parser-neutral view.
- Diagnostics identify the source artefact and location where the parser can
  supply a span.
- Continuous integration verifies native units with `systemd-analyze verify`.
- Continuous integration invokes the Podman Quadlet generator and verifies its
  generated unit output.
- Repovec policy checks run independently of the official-tool probes.

### Technical requirements

- Parser adapters preserve ordered directive occurrences and reset markers.
- Adapter code contains all dependencies on parser-specific public APIs.
- Tool probes retain raw standard output, standard error, arguments, and exit
  status as evidence while redacting those buffers from default `Debug` output.
- Supported systemd and Podman versions are explicit test dimensions.
- Static validation performs no ambient filesystem discovery unless the caller
  supplies the relevant source set.

## Options considered

### Promote either current parser into a shared crate

This would publish a deliberately incomplete syntax model and freeze accidental
implementation choices. It is rejected.

### Adopt one generic INI parser

Systemd resembles INI but has directive-specific repetition and reset
semantics, continuations, quoting, specifiers, and drop-ins. Generic INI models
usually erase exactly the information required here. This is rejected.

### Isolate existing parsers behind adapters and verify them through spikes

This permits a measured migration to specialist libraries while preserving
repovec's policy tests. Official consumer tools provide an independent oracle.
This is selected.

## Decision outcome / proposed direction

The project will separate four layers that the current implementation partly
combines:

1. **Source model.** Adapter implementations backed by `systemd-unit-edit` for
   native units and `quadlet-lens` for Quadlets.
2. **Contract policy.** Parser-neutral rules and diagnostics supplied by the
   nursery `repovec-unit-contract` crate described by ADR 003 and ADR 004.
3. **Consumer verification.** Podman generator and `systemd-analyze verify`
   probes represented by the nursery `repovec-systemd-probe` interface.
4. **Live manager control.** A future D-Bus adapter for starting, stopping,
   enabling, disabling, and querying generated unit instances.

The parser migrations are spikes first. Each spike must evaluate:

- every checked-in source file;
- the existing handwritten mutation suite;
- the existing property-generated corpus;
- comments, continuations, repeated sections, resets, quoting, and malformed
  input;
- source locations and secret-redaction compatibility;
- minimum supported Rust, systemd, and Podman versions;
- upstream release, maintenance, and licence posture; and
- the amount of adapter code required without accessing library internals.

A successful spike records a decision table and either adopts the library,
proposes narrowly scoped upstream changes, or documents why the current parser
must remain temporarily. New syntax features must not be added to the local
parsers while a viable upstream path exists.

The official-tool probes are authoritative for consumer acceptance, not for
repovec policy. The policy engine remains authoritative for appliance-specific
requirements such as Qdrant image selection, loopback binding, secret wiring,
service identity, and dependency topology.

Future runtime systemd work will use the manager's D-Bus API through an existing
Rust client where practical. It will not shell out to `systemctl` and parse
human-oriented output.

## Migration plan

1. Add differential fixtures and corpus runners without changing production
   validation.
2. Complete and document the native-unit parser spike.
3. Complete and document the Quadlet parser spike.
4. Add Podman generator and `systemd-analyze verify` probes in supported
   container images.
5. Introduce parser adapters that implement the shared `UnitView` interface.
6. Migrate appliance policy to the shared contract engine.
7. Delete each local parser only after its replacement passes the old corpus
   plus official-tool verification.
8. Decide whether startup checks are build-time asset checks or live effective
   configuration checks. Do not retain a runtime check of embedded source text
   as a substitute for either.
9. Complete a D-Bus client spike before implementing roadmap item 3.2.

## Known risks and limitations

- Neither candidate parser has the maturity of systemd itself. Differential and
  official-tool testing remain necessary after adoption.
- Podman generator output varies by version. The probe must report versioned
  evidence rather than normalize differences into an invented universal form.
- `systemd-analyze verify` can depend on host paths and unit search paths. The
  harness must make those dependencies explicit and reproducible.
- A parser adapter can accidentally become a second semantic model. Adapters
  must translate data, not reinterpret directives that belong to the policy
  layer.

## Architectural rationale

This decision makes the real consumers part of the test architecture while
retaining fast, pure policy checks. It replaces two small private languages with
adapters and evidence, rather than combining them into one larger private
language wearing a library badge.
