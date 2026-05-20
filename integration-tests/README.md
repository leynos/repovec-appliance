# Provisioning Integration Tests

The integration test suite exercises the packaging helpers against real
system-level state. It is intentionally separate from `make test` because it
creates users, writes under `/etc`, and uses rootful Podman secrets.

## Prerequisites

- Root privileges.
- Rootful Podman with secret support.
- `useradd`, `userdel`, `getent`, and `stat`.
- Bats-core. The repository vendors `bats-core`, `bats-support`, and
  `bats-assert` under `integration-tests/vendor/`.

## Running Locally

Run the suite from the repository root:

```sh
make integration-test
```

The target skips gracefully when Bats or Podman is unavailable. Tests also skip
when they are not run as root.

To run the vendored Bats executable directly:

```sh
sudo integration-tests/vendor/bats-core/bin/bats --recursive integration-tests/provisioning
```

## Container Execution

An optional Fedora-based container image is provided for isolated local runs.
The container must be privileged because the tests exercise rootful Podman and
system user provisioning.

```sh
podman build -f integration-tests/Containerfile -t repovec-integration-tests .
podman run --privileged repovec-integration-tests
```

## Test Inventory

- `creates repovec system user when absent`
- `creates key file with mode 0400 and ownership repovec:repovec`
- `creates Podman secret repovec-qdrant-api-key`
- `preserves existing key file on re-run`
- `regenerates key file when absent and refreshes secret`
- `creates /etc/repovec with mode 0750 and ownership root:repovec`
