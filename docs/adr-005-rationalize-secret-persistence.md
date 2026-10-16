# Architectural decision record (ADR) 005: Rationalize secret persistence

## Status

Proposed. Merging the pull request that introduces this record accepts the
credential boundary and migration direction. Qdrant Podman-secret provisioning
remains a distinct appliance concern.

## Date

2026-08-29.

## Context and problem statement

The GitHub OAuth token store currently invokes `systemd-creds` for encryption
and decryption, implements a command-runner abstraction, and writes ciphertext
through a custom same-directory atomic replacement routine. The Qdrant API-key
helper separately creates directories, repairs ownership and permissions,
generates key material, and synchronizes a rootful Podman secret. It also falls
back to invoking `useradd`, despite the repository shipping a `sysusers.d`
configuration.

Some of this is legitimate application behaviour. Some duplicates facilities
that systemd already provides:

- service credentials can decrypt and expose secrets only for the lifetime of a
  service;
- `sysusers.d` declaratively creates service accounts;
- `tmpfiles.d`, `StateDirectory=`, and `ConfigurationDirectory=` declaratively
  create and own directories; and
- capability-oriented temporary-file libraries can supply safer temporary file
  construction.

The durable write code nevertheless has stronger semantics than a simple
`write` followed by `rename`: it synchronizes the file and containing directory
and applies restrictive permissions. Those guarantees must not disappear
inside a superficially tidier abstraction.

## Decision drivers

- Plaintext credentials should exist for the shortest practical lifetime.
- Services should consume systemd-managed credentials when available.
- Secret values must never appear in command arguments, `Debug`, or routine
  telemetry.
- Durable replacement must preserve same-directory atomicity, restrictive
  permissions, file synchronization, and parent-directory synchronization.
- Account and directory provisioning should be declarative and idempotent.
- Rootful Podman secrets must remain in the rootful Podman store.

## Decision outcome / proposed direction

The project will separate secret protection, durable persistence, service
credential delivery, and Podman secret synchronization.

The nursery `repovec-secret-store` crate defines the generic composition:

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
}
```

The traits do not mention `systemd-creds`, a concrete directory type, or a
process runner. Those belong to adapters.

For daemon credential reads, unit files will prefer
`LoadCredentialEncrypted=` or another appropriate systemd credential source.
Services will read plaintext from the credential directory supplied by systemd.
The project will first evaluate existing Rust support, including the credentials
surface in [`libsystemd`](https://crates.io/crates/libsystemd), before adding any
reader code.

For write and rotation paths, a `systemd-creds` codec adapter may remain if no
maintained crate exposes the required operation. Its process runner, argument
construction, exit evidence, and redacted error handling should live beside the
adapter rather than inside the generic store.

The durable repository implementation will evaluate
[`cap-tempfile`](https://crates.io/crates/cap-tempfile) for secure temporary file
construction and replacement. Adoption is conditional on preserving all of the
existing durability and mode guarantees. Missing guarantees should be proposed
upstream or supplied by a thin adapter, not silently omitted.

The Qdrant bootstrap remains appliance-specific because it must create or
refresh a rootful Podman secret consumed by a rootful Quadlet. It will use the
shared secret primitives where useful but will not be generalized into the
secret-store crate.

The packaging layer will remove imperative account creation. `sysusers.d`
creates the `repovec` account; `tmpfiles.d`, `StateDirectory=`, and
`ConfigurationDirectory=` create directories according to lifetime and
ownership. The helper should fail with a clear packaging error when those
prerequisites are absent rather than invoking `useradd`.

## Goals and non-goals

Goals:

- reduce duplicated secret lifecycle code;
- use systemd's activation-time credential delivery;
- preserve crash-durable, capability-oriented persistence;
- eliminate imperative account provisioning from service helpers; and
- expose a backend-neutral interface suitable for more than one secret.

Non-goals:

- replacing the rootful Podman secret database;
- designing a general secrets manager;
- storing OAuth scope metadata inside bearer-token ciphertext; or
- promising secure deletion on storage media that cannot provide it.

## Migration plan

1. Characterize current durability, permissions, redaction, and failure
   semantics with tests.
2. Add the nursery interfaces and adapters in parallel with the current token
   store.
3. Spike `libsystemd` credential reading and `cap-tempfile` durable replacement.
4. Add encrypted systemd credential loading to `repovecd.service` and migrate
   daemon reads to the service credential directory.
5. Move token writes and rotations behind `repovec-secret-store` composition.
6. Remove duplicate decryption subprocesses from daemon startup where systemd
   already supplies plaintext credentials.
7. Move user and directory provisioning to declarative packaging and delete the
   `useradd` fallback.
8. Reuse only the safe generic pieces in Qdrant provisioning while retaining
   explicit rootful Podman-secret synchronization.

## Known risks and limitations

- Systemd credential features vary across supported distributions. The
  appliance support matrix must declare the minimum version or provide an
  explicit fallback.
- Directory directives may not fit resources shared by several services.
  `tmpfiles.d` remains appropriate for package-wide state.
- Atomic rename alone does not guarantee crash durability. Tests and review must
  keep file and directory synchronization visible.
- External command error text can contain sensitive input. Raw evidence must be
  access-controlled and redacted from default formatting.

## Architectural rationale

This decision uses systemd for lifecycle-specific work, keeps Podman-specific
work explicit, and extracts only the backend-neutral protection and persistence
composition. The result is less code without sacrificing the hard-won fsync
teeth of the current implementation.
