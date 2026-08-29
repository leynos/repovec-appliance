# Nursery crates

This document defines the current boundaries and governance of the experimental
Rust crates under `crates/nursery/`. The crates make prospective extraction
seams executable without presenting them as stable ecosystem packages.

ADR 004 governs nursery lifecycle and graduation. ADRs 002, 003, and 005 govern
the initial crate responsibilities. The roadmap owns migration status; this
document owns the current interface map.

## Workspace status

The nursery is a nested Cargo workspace excluded from the production workspace.
Every package sets `publish = false`. Interfaces may change without a migration
period until a crate graduates.

Run the complete nursery gate from the repository root:

```console
cargo fmt --manifest-path crates/nursery/Cargo.toml --all -- --check
cargo check --manifest-path crates/nursery/Cargo.toml --workspace --all-targets
cargo clippy --manifest-path crates/nursery/Cargo.toml --workspace \
  --all-targets -- -D warnings
cargo test --manifest-path crates/nursery/Cargo.toml --workspace --all-targets
cargo doc --manifest-path crates/nursery/Cargo.toml --workspace --no-deps
```

The dedicated `nursery` workflow runs those commands when nursery source or its
workflow changes. A second `nursery-msrv` job checks the declared Rust 1.85
minimum. Production workspace commands intentionally do not compile these
packages.

## Interface map

| Crate | Owns | Does not own | First intended consumers |
| --- | --- | --- | --- |
| `repovec-unit-contract` | Unit views, rule execution, structured diagnostics | Parsing, effective systemd semantics, logging, repovec policy | Native-unit and Qdrant policy adapters |
| `repovec-systemd-probe` | Official-tool evidence and generator/verifier ports | Unit parsing, policy rules, live D-Bus control | CI systemd gate and packaging integration tests |
| `repovec-secret-store` | Secret protection and durable repository composition | `systemd-creds` process details, filesystem choice, Podman synchronization | GitHub token store and later webhook credentials |

_Table 1: Nursery crate responsibilities and intended consumers._

## `repovec-unit-contract`

### Purpose

`repovec-unit-contract` evaluates application rules over an ordered,
parser-neutral view of unit directives. A parser adapter supplies occurrences;
a product crate supplies rules; the engine returns diagnostics.

### Principal interfaces

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

pub fn validate(
    artifact: &ArtifactId,
    unit: &dyn UnitView,
    rules: &[&dyn Rule],
) -> ValidationReport;
```

`DirectiveOccurrence` exposes raw and decoded values, a source origin, and an
optional byte span. `Diagnostic` exposes a stable code, severity, message,
artefact identity, location, and sensitivity. `ValidationReport` accumulates
findings and is invalid when any finding has error severity.

### Adoption constraints

- A native systemd adapter must preserve repeated occurrences, source order,
  empty reset assignments, and drop-in origin.
- A Quadlet adapter must preserve source locations and unknown directives.
- Rules must not downcast to a parser-specific type.
- Rules must not emit tracing or process output directly.
- Secret-bearing diagnostics must use `Sensitivity::Secret` even when the
  rendered message is already redacted.

## `repovec-systemd-probe`

### Purpose

`repovec-systemd-probe` defines the boundary between deterministic policy tests
and external consumer verification. It models source artefacts, generated
units, raw tool evidence, diagnostics, and the ports implemented by Podman and
systemd command adapters.

### Principal interfaces

```rust,no_run
pub trait QuadletGenerator {
    type Error: std::error::Error + Send + Sync + 'static;

    fn generate(
        &self,
        sources: &[SourceArtifact<'_>],
    ) -> Result<GenerationReport, Self::Error>;
}

pub trait SystemdVerifier {
    type Error: std::error::Error + Send + Sync + 'static;

    fn verify(
        &self,
        units: &[SourceArtifact<'_>],
    ) -> Result<VerificationReport, Self::Error>;
}
```

`ToolEvidence` retains the program, arguments, exit status, standard output, and
standard error. Its default `Debug` representation reveals buffer lengths only.
Callers must opt in through explicit accessors before processing raw evidence.

### Adoption constraints

- Production adapters must invoke tools without a shell.
- Temporary roots and search paths must be caller-controlled.
- Reports must record tool versions alongside evidence before the first
  production adoption.
- Adapter diagnostics may summarize raw output but must not discard it.
- This crate must not grow a parser for tool display output. Structured or
  stable machine-readable output is preferred; otherwise adapters return
  conservative diagnostics plus the evidence buffer.

Live unit control is outside this crate. Roadmap work will evaluate an existing
D-Bus client and define an application port only after that spike.

## `repovec-secret-store`

### Purpose

`repovec-secret-store` composes a protection codec with a durable repository.
The interface permits a `systemd-creds` codec, a test codec, a capability-based
filesystem repository, and future credential backends without binding the core
types to any one command or storage library.

### Principal interfaces

```rust,no_run
pub trait SecretCodec {
    type Error: std::error::Error + Send + Sync + 'static;

    fn protect(
        &self,
        name: &SecretName,
        plaintext: &SecretBytes,
        policy: ProtectionPolicy,
    ) -> Result<ProtectedSecret, Self::Error>;

    fn unprotect(
        &self,
        name: &SecretName,
        protected: &ProtectedSecret,
    ) -> Result<SecretBytes, Self::Error>;
}

pub trait SecretRepository {
    type Error: std::error::Error + Send + Sync + 'static;

    fn load(
        &self,
        name: &SecretName,
    ) -> Result<Option<ProtectedSecret>, Self::Error>;

    fn replace(
        &self,
        name: &SecretName,
        protected: &ProtectedSecret,
    ) -> Result<(), Self::Error>;

    fn remove(&self, name: &SecretName) -> Result<(), Self::Error>;
}
```

`SecretBytes` and `ProtectedSecret` redact their default `Debug` output.
Access to bytes requires an explicit `expose()` call. `CodecBackedSecretStore`
provides the initial composition implementation.

### Repository contract

A filesystem implementation of `SecretRepository::replace` must document and
test all of the following:

- temporary creation in the destination directory;
- restrictive creation mode before secret bytes are written;
- complete write and file synchronization;
- atomic replacement of the destination;
- containing-directory synchronization; and
- cleanup after failures where the operating system permits it.

A backend that cannot provide those guarantees must report the weaker contract
in its type or documentation. It must not silently implement `replace` with a
plain truncating write.

## Documentation ownership and de-duplication

Each architectural fact has one principal home:

- ADRs record accepted decisions and rejected alternatives.
- The technical design describes production composition and target state.
- This document defines current nursery interfaces and graduation state.
- The roadmap tracks implementation and migration status.
- Crate-level Rust documentation defines exact callable behaviour.

Other documents should link to the principal source instead of copying complete
interface lists or migration plans. This keeps the documentation graph from
becoming a hall of slightly different mirrors.

## Graduation ledger

| Crate | Production consumers | Interchangeable adapters | Graduation state |
| --- | ---: | ---: | --- |
| `repovec-unit-contract` | 0 | 0 | Interface seed |
| `repovec-systemd-probe` | 0 | 0 | Interface seed |
| `repovec-secret-store` | 0 | 0 | Interface seed |

_Table 2: Initial evidence ledger for nursery graduation._

The ledger changes only when merged production code supplies the evidence.
Design intent and test doubles alone do not count as independent consumers or
adapters.
