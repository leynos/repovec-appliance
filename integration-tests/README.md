# Integration tests

End-to-end tests for the appliance's packaging helpers. These do not run as
part of `make test`; they require a Docker-compatible runtime and, for the
full lifecycle suite, the ability to launch privileged nested Podman.

## Suite layout

The harness ships two test suites with different runtime requirements:

| Suite                                          | Marker            | Runtime required                                                          |
| ---------------------------------------------- | ----------------- | ------------------------------------------------------------------------- |
| `provisioning/test_qdrant_api_key_cmd_mox.py`  | `cmd_mox`         | None beyond Python and the pinned dependencies.                           |
| `provisioning/test_qdrant_api_key.py`          | `integration`     | A Docker-compatible runtime able to host privileged Podman-in-Podman.     |

The `cmd_mox` suite exists to make the helper's command orchestration cheap
to test in isolation. The `integration` suite owns the lifecycle truths that
only show up when real `useradd` and real rootful Podman are involved; if a
contract assertion fits both, push it to the `integration` suite.

## Prerequisites

- Python 3.13 (managed automatically by [`uv`](https://github.com/astral-sh/uv)).
- For the `cmd_mox` suite: nothing else. The cmd-mox shims are pure Python.
- For the `integration` suite: a reachable Docker-compatible API. On Linux,
  the canonical configuration is rootless Podman exposing its socket to
  Docker-API clients.

## Installation

```bash
cd integration-tests
uv sync
```

`uv sync` creates the project virtualenv (under `.venv/`), resolves
dependencies from `pyproject.toml`, and writes the lockfile. The Makefile's
`integration-test` and `integration-command-test` targets advertise the
same command in their skip-message hint, so the developer instructions and
the Make contract stay in lockstep.

The harness is intentionally not a wheel; the `pyproject.toml` is a vehicle
for `uv sync` and for pytest's rootdir/markers configuration.

## Running tests

### Through the Makefile

```bash
# Fast command-contract tests (no container runtime required)
make integration-command-test

# Full lifecycle tests (requires privileged Podman)
make integration-test
```

Both targets skip gracefully if their prerequisites are missing. Set
`PYTHON=/path/to/python` to point at a specific interpreter (for example, the
virtualenv created by `uv venv`).

### Directly via pytest

```bash
cd integration-tests
.venv/bin/python -m pytest -m cmd_mox provisioning
.venv/bin/python -m pytest -m integration provisioning
```

Pass `--no-skip-on-missing-runtime` to convert "no runtime" skips into hard
failures, which is appropriate for CI jobs that must guarantee the full
suite ran.

## Podman-backed Testcontainers configuration

The full lifecycle suite uses
[`testcontainers-python`](https://testcontainers-python.readthedocs.io/), which
talks to a Docker-compatible API. On Linux with rootless Podman:

```bash
# Start the Podman API service (run once per session).
podman system service --time=0 &

# Point Docker clients at the rootless socket.
export DOCKER_HOST="unix://${XDG_RUNTIME_DIR}/podman/podman.sock"

# Ryuk (Testcontainers' reaper) does not play nicely with rootless Podman.
export TESTCONTAINERS_RYUK_DISABLED=true
```

The harness builds the Fedora image from this directory's `Containerfile`
using the repository root as the build context, then starts it `--privileged`
so the inner rootful Podman can manage secrets. The host's container runtime
must permit privileged containers for the lifecycle suite to run; if it
cannot, the fixtures emit `pytest.skip` with a pointer to this README.

## Manual smoke run

Useful when debugging the image itself:

```bash
podman build -f integration-tests/Containerfile -t repovec-integration-tests .
podman run --rm --privileged repovec-integration-tests \
    /bin/bash -lc 'podman info && /usr/libexec/repovec/repovec-qdrant-api-key'
```

## Test inventory

### `cmd_mox` suite (`test_qdrant_api_key_cmd_mox.py`)

| Test                                                             | Asserts                                                            |
| ---------------------------------------------------------------- | ------------------------------------------------------------------ |
| `test_invokes_useradd_when_repovec_user_is_absent`               | `useradd` is invoked with the documented system-user arguments.    |
| `test_skips_useradd_when_repovec_user_exists`                    | `useradd` is not invoked when `getent` reports the user.           |
| `test_generates_key_when_key_file_is_absent`                     | `chown`/`chmod`/`mv` materialise a fresh key with `0400`/owner.    |
| `test_creates_or_refreshes_podman_secret_from_key_file`          | `podman secret rm` precedes `podman secret create`.                |
| `test_does_not_regenerate_valid_existing_key`                    | A valid 64-hex key survives a rerun with mtime unchanged.          |

### `integration` suite (`test_qdrant_api_key.py`)

| Test                                                                 | Asserts                                                                      |
| -------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `test_creates_repovec_system_user_when_absent`                       | The helper creates the system user with the documented home/shell.           |
| `test_creates_key_file_with_mode_0400_and_ownership_repovec`         | `/etc/repovec/qdrant-api-key` is `0400 repovec:repovec` and one 64-hex line. |
| `test_creates_podman_secret_repovec_qdrant_api_key`                  | The rootful Podman secret is created and inspectable.                        |
| `test_preserves_existing_key_file_on_rerun`                          | A valid existing key file is left untouched on rerun.                        |
| `test_regenerates_key_file_when_absent_and_refreshes_secret`         | A missing key is regenerated and the secret refreshed.                       |
| `test_creates_etc_repovec_with_mode_0750_and_ownership_root_repovec` | `/etc/repovec` ends up as `0750 root:repovec`.                               |

## Troubleshooting

- **`podman info failed inside the integration container`** — the host kernel
  or runtime cannot host privileged nested Podman. Either run the suite on a
  VM with that capability, or set `--no-skip-on-missing-runtime` and accept
  that the relevant tests will fail loudly instead of skipping silently.
- **`No Docker-compatible runtime reachable; skipping integration-test.`** —
  the `DOCKER_HOST` socket is not reachable. Start `podman system service`
  and re-export `DOCKER_HOST` per the snippet above.
- **"pytest, cmd-mox, or cuprum not installed; skipping ..."** — install the
  harness's dependencies with `uv pip install -e .` from this directory, or
  pass `PYTHON=$(pwd)/integration-tests/.venv/bin/python` to the `make`
  invocation so the targets resolve the right interpreter.
- **`UnknownProgramError` from cuprum** — a host command was invoked that is
  not on the curated allowlist in `lib/commands.py`. Add it to the catalogue
  rather than bypassing the helper; the allowlist is the harness's safety
  rail against accidental shell-outs in test code.
