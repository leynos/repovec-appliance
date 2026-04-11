# Execution plan for roadmap item 1.2.1: Podman Quadlet definition for Qdrant

## Preamble

- **Roadmap item:** `1.2.1`
- **Status:** Proposed
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
- Use a fully qualified image reference such as
  `docker.io/qdrant/qdrant:<pinned-version>` instead of an unqualified or
  floating reference. This is required for predictable rollouts and matches
  Podman's `AutoUpdate=registry` expectations.
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
- Decide whether the storage mount should include an SELinux relabel suffix
  such as `:Z`. This depends on the supported appliance base images and should
  be captured explicitly in the design document once chosen.
- Decide whether the Quadlet itself should include an `[Install]` section.
  Prefer keeping boot-target wiring in roadmap item `1.3.1`, but document the
  decision either way so the enablement path is unambiguous.
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

- **Boot integration ownership:** decide whether `qdrant.container` should own
  standalone enablement, or whether all boot wiring belongs exclusively to
  `repovec.target` in roadmap item `1.3.1`.
- **SELinux relabelling:** decide whether the persistent storage mount requires
  `:Z` on the supported appliance images.
- **Image pinning policy:** decide whether `latest` is explicitly forbidden in
  tests, or whether any fully qualified tag is acceptable. The safer choice is
  to reject floating tags.
- **Prompt mismatch:** the terminal-behaviour completion criteria in the prompt
  do not map to this feature and should be clarified before being used in
  review.

## 7. Definition of done

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
