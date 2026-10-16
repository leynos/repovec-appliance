# Architectural decision record (ADR) 003: Centralize unit contract policy

## Status

Proposed. Merging the pull request that introduces this record accepts the
contract and diagnostic direction. The initial interface remains a nursery API.

## Date

2026-08-29.

## Context and problem statement

Native systemd-unit validation and Qdrant Quadlet validation currently use
separate parsed representations, separate error enums, and separate query
helpers. The Qdrant path additionally exposes a broad observer trait with one
callback for nearly every validation event.

The useful common capability is not parsing. It is expressing and evaluating
application contracts over a source-aware unit view:

- a section or directive must exist;
- one effective value must equal a required value;
- a list-valued directive must contain a required token;
- an occurrence must not contain a forbidden value;
- related artefacts must contain matching dependency edges; and
- diagnostics must identify location and sensitivity.

Copying these mechanics for every service or Quadlet would create parallel
validator dialects. Extracting the current parsers would instead expose their
limitations as public API.

## Decision drivers

- Rules must not depend on one parser implementation.
- Diagnostics must be data, not irreversible logging side effects.
- Validation should report all independent findings in one pass where safe.
- Sensitive source fragments must remain marked across all renderers.
- Repovec-specific policy must not leak into a general contract engine.
- Source order and origin must remain available to rules that need them.

## Decision outcome / proposed direction

The project will incubate a parser-neutral contract engine in
`crates/nursery/repovec-unit-contract`.

Its boundary is intentionally small:

```rust,no_run
pub trait UnitView {
    fn occurrences<'a>(
        &'a self,
        section: &str,
        directive: &str,
    ) -> Box<dyn Iterator<Item = DirectiveOccurrence<'a>> + 'a>;
}

pub trait Rule {
    fn check(
        &self,
        artifact: &ArtifactId,
        unit: &dyn UnitView,
        diagnostics: &mut dyn DiagnosticSink,
    );
}
```

`DirectiveOccurrence` carries raw and decoded values, an optional source span,
and source origin. The source origin distinguishes at least base units,
drop-ins, generated units, and synthetic test inputs.

`Diagnostic` carries a stable code, severity, message, artefact identity,
optional source span, and sensitivity classification. Rules emit diagnostics;
they do not call `tracing`, print output, terminate a process, or choose an exit
code.

A `ValidationReport` accumulates diagnostics and computes validity from error
severity. Callers may stop after parsing or another fatal boundary, but policy
rules should otherwise report all independent violations in one run.

The engine will provide reusable rule combinators only after two real policies
need the same operation. Initial migration should prefer explicit rule structs
over a prematurely elaborate domain-specific language.

Repovec-specific rules remain in production crates. Examples include:

- the permitted Qdrant image and auto-update policy;
- loopback-only port publication;
- Qdrant storage and SELinux labelling;
- API-key secret wiring;
- `repovec.target` membership;
- daemon executable paths and identities; and
- grepai template hardening.

Renderers and adapters may turn diagnostics into tracing events, terminal
messages, JSON, or Static Analysis Results Interchange Format (SARIF). The
contract crate owns none of those presentation choices.

## Goals and non-goals

Goals:

- remove duplicated validation plumbing;
- expose source-aware, parser-neutral rule inputs;
- make redaction and sensitivity explicit;
- support richer CI and operator diagnostics; and
- retain fast in-process policy tests.

Non-goals:

- parsing systemd or Quadlet syntax;
- reproducing systemd's effective-value engine;
- installing or controlling units;
- defining repovec's product policy; or
- designing a general configuration-validation language.

## Migration plan

1. Land the nursery interface without production dependencies.
2. Add adapters over the current parsers solely to prove interface usability.
3. Express one native-unit contract and one Qdrant Quadlet contract through the
   engine.
4. Compare accumulated diagnostics with the current first-error APIs and define
   the compatibility mapping required by callers.
5. Introduce adapters for the parser libraries selected under ADR 002.
6. Move tracing to diagnostic renderers at application boundaries.
7. Remove duplicated parser query helpers and the callback-per-finding observer
   surface after downstream callers migrate.
8. Add JSON and SARIF rendering only when a consuming workflow requires them.

## Known risks and limitations

- A generic rule abstraction can obscure simple checks. Explicit Rust rule
  implementations remain the default until repetition demonstrates a need.
- Stable diagnostic codes become compatibility surface. Nursery codes may
  change until the graduation criteria in ADR 004 are met.
- The engine cannot compensate for source adapters that discard reset or
  effective-value semantics.

## Architectural rationale

The contract engine turns validation into a pure query over an explicit source
view. Effects remain at the boundary, and specialist parsers remain replaceable.
This is the reusable centre of the existing work without importing its
application-specific barnacles.
