"""Shared pytest fixtures for repovec-appliance integration tests.

This module centralises the testcontainers lifecycle so test modules can stay
focused on the helper's behavioural contract rather than image build and
container plumbing. Two fixtures matter:

``integration_container``
    Session-scoped Fedora container built from
    ``integration-tests/Containerfile``. The container runs privileged so
    rootful Podman can manage real secrets inside it.

``container_session``
    Function-scoped :class:`ContainerSession` whose ``cleanup_state`` runs
    before and after every test. This isolates lifecycle tests without paying
    for a fresh container build per case.

If the host runtime cannot reach a Docker-compatible daemon, or cannot launch
privileged nested Podman, the fixtures emit ``pytest.skip`` with actionable
messages rather than failing with cryptic socket errors. The
``--no-skip-on-missing-runtime`` flag forces a hard failure so CI can guard
against accidentally skipping the entire suite.
"""

from __future__ import annotations

from collections.abc import Iterator
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from lib.container import ContainerSession

if TYPE_CHECKING:
    from testcontainers.core.container import DockerContainer


REPO_ROOT = Path(__file__).resolve().parent.parent
CONTAINERFILE = Path(__file__).resolve().parent / "Containerfile"
IMAGE_TAG = "repovec-integration-tests:latest"


def pytest_addoption(parser: pytest.Parser) -> None:
    """Register the ``--no-skip-on-missing-runtime`` CLI flag."""

    parser.addoption(
        "--no-skip-on-missing-runtime",
        action="store_true",
        default=False,
        help=(
            "Fail (instead of skipping) when no container runtime is "
            "available. Use in CI environments that must run the full suite."
        ),
    )


def _skip_or_fail(request: pytest.FixtureRequest, reason: str) -> None:
    """Skip the current test, or fail hard if the no-skip flag is set."""

    if request.config.getoption("--no-skip-on-missing-runtime"):
        pytest.fail(reason, pytrace=False)
    else:
        pytest.skip(reason)


def _build_image(request: pytest.FixtureRequest) -> str:
    """Build the integration-test image; skip on builder/import failure."""

    try:
        from testcontainers.core.image import DockerImage
    except ImportError as exc:  # pragma: no cover - import-time guard
        _skip_or_fail(request, f"testcontainers-python is not installed: {exc}")
        raise

    # Distinguish a missing/unreachable runtime (skip) from a broken
    # Dockerfile or build context (fail). The first failure mode is a host
    # environmental issue; the second is a real defect that must surface.
    # A separate ``docker.from_env().ping()`` keeps the discrimination
    # simple — any exception from ``image.build()`` then propagates.
    try:
        import docker
        from docker.errors import DockerException
    except ImportError as exc:  # pragma: no cover - import-time guard
        _skip_or_fail(request, f"docker SDK is not installed: {exc}")
        raise

    # ``docker.from_env()`` opens a persistent HTTP/Unix socket session
    # that we must close explicitly; otherwise Python's GC reports an
    # unclosed-socket ``ResourceWarning`` at session teardown, which the
    # project's ``filterwarnings = ["error"]`` promotes to a hard error.
    # ``DockerClient`` does not implement the context manager protocol,
    # so wrap construction + ping in try/finally and ``close()`` by hand.
    # ``docker.from_env()`` itself raises ``DockerException`` when the
    # daemon is reachable but advertises an incompatible API version, so
    # build the client *inside* the try block to keep both failure modes
    # on the skip path.
    client = None
    try:
        client = docker.from_env()
        client.ping()
    except DockerException as exc:
        _skip_or_fail(
            request,
            (
                "no Docker-compatible runtime reachable for the integration "
                f"image build: {exc}"
            ),
        )
        raise
    finally:
        if client is not None:
            client.close()

    image = DockerImage(
        path=str(REPO_ROOT),
        dockerfile_path=str(CONTAINERFILE.relative_to(REPO_ROOT)),
        tag=IMAGE_TAG,
    )
    image.build()
    return str(image)


def _start_container(request: pytest.FixtureRequest, image: str) -> DockerContainer:
    """Start ``image`` privileged with a long-running default command."""

    from testcontainers.core.container import DockerContainer

    container = DockerContainer(image)
    container.with_kwargs(privileged=True)
    container.with_command(["sleep", "infinity"])

    try:
        container.start()
    except Exception as exc:  # noqa: BLE001 - propagate as actionable skip
        _skip_or_fail(
            request,
            (
                "unable to start the integration-test container; ensure a "
                "Docker-compatible runtime is available and that privileged "
                f"containers are permitted: {exc}"
            ),
        )
        raise

    return container


def _preflight(container: DockerContainer, request: pytest.FixtureRequest) -> None:
    """Confirm nested Podman is actually usable inside the container.

    Failing here means the host kernel or runtime cannot host rootful
    Podman-in-Podman; that is an environmental constraint, not a code defect,
    so skip with an explanation unless ``--no-skip-on-missing-runtime`` is set.
    """

    session = ContainerSession(container)
    result = session.run("podman", "info")
    if not result.ok:
        _skip_or_fail(
            request,
            (
                "podman info failed inside the integration container; the "
                "host runtime likely cannot host privileged nested Podman. "
                f"\n{result.render()}"
            ),
        )


@pytest.fixture(scope="session")
def integration_container(
    request: pytest.FixtureRequest,
) -> Iterator[DockerContainer]:
    """Session-scoped privileged Fedora container for lifecycle tests.

    Yields
    ------
    DockerContainer
        The running, preflighted container. Per-test wrappers (with
        their domain-specific cleanup) live in suite-local conftests
        — see ``provisioning/conftest.py`` for the canonical pattern.
    """

    image = _build_image(request)
    container = _start_container(request, image)
    try:
        _preflight(container, request)
        yield container
    finally:
        try:
            container.stop()
        except Exception:  # noqa: BLE001 - best-effort teardown
            pass
