# Execution plan for roadmap item 1.2.1: Podman Quadlet definition for Qdrant

## Preamble

- **Roadmap item:** `1.2.1`
- **Status:** Implemented
- **Created:** 2026-04-11
- **Primary references:** [roadmap](../roadmap.md),
  [technical design](../repovec-appliance-technical-design.md), [Podman Quadlet
  documentation][podman-quadlet-docs], [Qdrant configuration
  documentation][qdrant-config-docs], and [Qdrant security
  documentation][qdrant-security-docs]

## 1. Summary

Implement roadmap item `1.2.1` by adding a checked-in Quadlet definition for
Qdrant, plus Rust-side validation and test coverage that prove the file keeps
the required contract:

- official Qdrant image
- REST on `127.0.0.1:6333`
- gRPC on `127.0.0.1:6334`
- persistent storage mounted from `/var/lib/repovec/qdrant-storage`
- Podman auto-update enabled

This plan covers the Quadlet definition itself, its repository placement, and
the validation surface needed to keep the definition from regressing. API-key
generation, storage, and secret-file permissions remain the responsibility of
roadmap item `1.2.2`.

## 2. Scope and constraints

- The repository currently has no packaging or appliance-assets directory, so
  this task needs to establish one rather than append to an existing pattern.
- The repository also has no canonical `docs/users-guide.md`; the
  implementation should create it rather than attempt to extend
  `docs/ortho-config-users-guide.md`, which documents a different library.
- CI in this repository does not currently appear to provide Podman or a
  writable systemd environment, so the mandatory automated coverage should
  validate the Quadlet contract statically from Rust.
- Manual smoke coverage should still be documented for environments where
  Podman and systemd are available.
- The prompt's completion criteria about interactive sessions, resize
  propagation, and exit-code handling do not describe this roadmap item. Treat
  them as a carried-over mismatch and do not use them as the acceptance gate
  for `1.2.1` without clarification.

## 3. Proposed design decisions

- Store the repository source-of-truth Quadlet at
  `packaging/systemd/qdrant.container`.
- Treat the deployment install path as
  `/etc/containers/systemd/qdrant.container`, which is the rootful system
  Quadlet directory described in Podman's documentation.
- Use the fully qualified image reference
  `docker.io/qdrant/qdrant:v1.17.1`. This is required for predictable rollouts
  and matches Podman's `AutoUpdate=registry` expectations.
- Bind both published ports explicitly to `127.0.0.1` rather than relying on
  Qdrant's internal host binding alone. This follows Qdrant's guidance to bind
  to localhost or a private interface for local deployments.
- Validate the checked-in Quadlet with a small Rust parser and contract checker
  in `repovec-core`, rather than relying on shell greps in tests. This keeps
  the behaviour testable under `cargo test` and makes failures diagnostic.
- Record these decisions in
  `docs/repovec-appliance-technical-design.md` when the implementation lands.

## 4. Execution phases

### 4.1. Establish the appliance asset layout

- Create a new repository directory `packaging/systemd/`.
- Add `packaging/systemd/qdrant.container` as the checked-in Quadlet source.
- Keep the file narrowly scoped to the Qdrant container contract for `1.2.1`.
  Do not fold in target orchestration from roadmap item `1.3.1`.
- Use Quadlet-native keys instead of opaque `PodmanArgs=` where possible:
  `Image=`, `PublishPort=`, `Volume=`, and `AutoUpdate=`.
- Use two explicit `PublishPort=` entries:
  `127.0.0.1:6333:6333` and `127.0.0.1:6334:6334`.
- Use a persistent storage mount from
  `/var/lib/repovec/qdrant-storage` to `/qdrant/storage`.
- Add a short comment in the file only where it clarifies a non-obvious
  contract, for example why both host bindings are loopback-only.

### 4.2. Decide the exact Quadlet shape before writing tests

- Pin the Qdrant image to an explicit version tag and keep the reference fully
  qualified so `AutoUpdate=registry` remains valid.
- Include the storage mount suffix `:Z` so rootful Podman can relabel
  `/var/lib/repovec/qdrant-storage` correctly on SELinux-enforcing hosts.
- Keep the Quadlet free of an `[Install]` section. Boot-target wiring remains
  owned by roadmap item `1.3.1`, which avoids splitting unit-enablement
  responsibilities across two tasks.
- Keep API-key wiring limited to the seam needed for `1.2.2`. Avoid inventing
  secret-management behaviour in `1.2.1` that the roadmap already allocates to
  the next task.

### 4.3. Add a Rust validation surface in `repovec-core`

- Add a small module such as
  `crates/repovec-core/src/appliance/qdrant_quadlet.rs`.
- Expose a helper that loads the checked-in file and a validator that parses
  the minimal subset of Quadlet syntax needed for this contract.
- Model validation failures with a semantic error enum instead of stringly
  typed assertions. Suggested cases: `MissingImage`, `ImageNotFullyQualified`,
  `MissingRestPort`, `MissingGrpcPort`, `PortNotBoundToLoopback`,
  `MissingStorageMount`, `IncorrectStorageTarget`, and `MissingAutoUpdate`.
- Keep the parser deliberately small. A section-aware line parser is sufficient
  here and avoids introducing a new dependency for one static file format.

### 4.4. Add `rstest` unit coverage

- Put the fast contract tests alongside the new validator in
  `repovec-core`.
- Use `rstest` fixtures for:
  - the checked-in `qdrant.container` contents
  - mutated invalid fixtures derived from the checked-in file
- Cover the following cases at minimum:
  - happy path: the checked-in file passes validation
  - unhappy path: REST port exposed on `0.0.0.0` or without an explicit IP
  - unhappy path: gRPC port missing or bound to the wrong host IP
  - unhappy path: storage mount missing or targeting anything other than
    `/qdrant/storage`
  - unhappy path: auto-update missing or not configured for registry tracking
  - edge case: image reference is unqualified or uses a floating value that the
    project decides to reject

### 4.5. Add `rstest-bdd` behavioural coverage

- Add `rstest-bdd = "0.5.0"` as a development dependency in the crate that
  owns the validator tests.
- Create a feature file at
  `crates/repovec-core/tests/features/qdrant_quadlet.feature`.
- Create step definitions in
  `crates/repovec-core/tests/qdrant_quadlet_bdd.rs`.
- Express the behavioural contract in user-visible terms, for example:
  - a valid Qdrant Quadlet only exposes REST and gRPC on loopback
  - a valid Qdrant Quadlet persists storage under
    `/var/lib/repovec/qdrant-storage`
  - a valid Qdrant Quadlet opts in to Podman auto-update
  - an invalid Quadlet exposing external interfaces is rejected
  - an invalid Quadlet omitting the persistent storage mount is rejected
- Keep the BDD tests focused on behaviour of the validator and checked-in
  asset. Do not attempt to spin up real Podman containers in these tests.

### 4.6. Update the documentation set

- Update `docs/repovec-appliance-technical-design.md` to record the final
  implementation decisions for:
  - repository asset location
  - installed Quadlet location
  - fully qualified pinned image policy
  - `AutoUpdate=registry`
  - any SELinux mount option that becomes part of the contract
- Create `docs/users-guide.md` and add an operator-facing section describing:
  - Qdrant is an appliance-internal service
  - REST and gRPC bind only to loopback
  - persistent data lives under `/var/lib/repovec/qdrant-storage`
  - API-key authentication is required once roadmap item `1.2.2` is complete
- Mark roadmap entry `1.2.1` as done only after the Quadlet, tests,
  documentation, and quality gates all pass.

## 5. Validation plan

### 5.1. Required automated gates

Run the repository's documentation and Rust gates with `tee` and
`set -o pipefail`, per `AGENTS.md`:

```bash
set -o pipefail && make fmt 2>&1 | tee /tmp/1-2-1-make-fmt.log
set -o pipefail && make markdownlint 2>&1 | tee /tmp/1-2-1-markdownlint.log
set -o pipefail && make nixie 2>&1 | tee /tmp/1-2-1-nixie.log
set -o pipefail && make check-fmt 2>&1 | tee /tmp/1-2-1-check-fmt.log
set -o pipefail && make lint 2>&1 | tee /tmp/1-2-1-lint.log
set -o pipefail && make test 2>&1 | tee /tmp/1-2-1-test.log
```

### 5.2. Manual smoke validation when Podman/systemd are available

- Copy the generated Quadlet to `/etc/containers/systemd/qdrant.container` or
  `/run/containers/systemd/qdrant.container` on a disposable host.
- Run `systemctl daemon-reload`.
- Start the generated unit with `systemctl start qdrant.service`.
- Confirm `ss -ltn` shows `127.0.0.1:6333` and `127.0.0.1:6334`, and does not
  show those ports bound to `0.0.0.0`.
- Confirm the container can write to `/var/lib/repovec/qdrant-storage`.
- Confirm `systemctl enable qdrant.service` or the later `repovec.target`
  wiring survives a reboot, depending on which unit-owning decision is chosen.

## 6. Risks and open questions

- **Prompt mismatch:** the terminal-behaviour completion criteria in the prompt
  do not map to this feature and should be clarified before being used in
  review.

## 7. Implementation notes

- Added `packaging/systemd/qdrant.container` with the pinned image
  `docker.io/qdrant/qdrant:v1.17.1`, loopback-only REST and gRPC port
  publishing, persistent storage mounted at
  `/var/lib/repovec/qdrant-storage:/qdrant/storage:Z`, and
  `AutoUpdate=registry`.
- Added `repovec_core::appliance::qdrant_quadlet`, a small section-aware parser
  and validator that loads the checked-in Quadlet and enforces the contract
  statically under `cargo test`.
- Added `rstest` unit tests for the happy path and the required unhappy-path
  regressions.
- Added `rstest-bdd` behavioural coverage under
  `crates/repovec-core/tests/features/qdrant_quadlet.feature` and
  `crates/repovec-core/tests/qdrant_quadlet_bdd.rs`.
- Updated `docs/repovec-appliance-technical-design.md`,
  `docs/users-guide.md`, and `docs/roadmap.md` to reflect the shipped contract.

## 8. Validation status

- Completed on 2026-04-20:
  `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`, `make lint`,
  and `make test` all passed.
- `make lint` completed with the repository's normal Whitaker behaviour:
  the target reported `whitaker` absent on `PATH` and skipped that optional
  sub-check without failing the gate.

## 9. Definition of done

Roadmap item `1.2.1` is complete when all of the following are true:

- `packaging/systemd/qdrant.container` exists and matches the required
  localhost, storage, image, and auto-update contract
- `repovec-core` contains Rust validation logic with `rstest` unit tests
- `rstest-bdd` behavioural tests cover happy and unhappy paths
- the technical design and canonical users guide document the implemented
  behaviour
- `make fmt`, `make markdownlint`, `make nixie`, `make check-fmt`,
  `make lint`, and `make test` all succeed
- `docs/roadmap.md` marks `1.2.1` as done

[podman-quadlet-docs]:
https://docs.podman.io/en/latest/markdown/podman-systemd.unit.5.html
[qdrant-config-docs]:
https://qdrant.tech/documentation/operations/configuration/
[qdrant-security-docs]: https://qdrant.tech/documentation/operations/security/
