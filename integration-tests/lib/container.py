"""Testcontainers-backed harness for the provisioning helper.

The harness builds a Fedora image containing Podman, the helper script, and
its sibling packaging assets, then starts a privileged container so rootful
Podman-in-Podman can create and inspect secrets. A small :class:`ContainerSession`
wrapper hides the docker-py exec API behind argv-first methods so test code
reads as command intent rather than transport plumbing.

This module is deliberately domain-agnostic: anything specific to the
``repovec-qdrant-api-key`` lifecycle (cleanup scripts, fixture wiring) lives
in :mod:`provisioning.conftest` so a future second suite can reuse this
container API without inheriting provisioning's vocabulary.
"""

from __future__ import annotations

import io
import tarfile
from collections.abc import Iterator
from pathlib import Path
from typing import TYPE_CHECKING

from .result import CommandResult

if TYPE_CHECKING:
    from testcontainers.core.container import DockerContainer


class ContainerCommandError(AssertionError):
    """Raised when a ``must_*`` invocation fails inside the container."""

    def __init__(self, result: CommandResult) -> None:
        """Store ``result`` and surface its rendered diagnostic as the message."""

        super().__init__(result.render())
        self.result = result


class ContainerSession:
    """Argv-first wrapper around a started ``DockerContainer``.

    The class intentionally stays thin: assertion behaviour lives in
    :mod:`integration_tests.lib.assertions`, and command-line construction
    belongs to the caller. Shell forms are reserved for the few helpers that
    genuinely need pipes, redirection, or compound cleanup logic.
    """

    def __init__(self, container: DockerContainer) -> None:
        """Bind this session to an already-started ``DockerContainer``."""

        self._container = container

    @property
    def container(self) -> DockerContainer:
        """Return the underlying ``DockerContainer`` for direct API access."""

        return self._container

    def run(self, *argv: str) -> CommandResult:
        """Execute ``argv`` inside the container without raising on failure.

        Parameters
        ----------
        *argv : str
            Command line as a sequence of tokens. The first token is the
            program; the rest are its arguments. At least one token is
            required.

        Returns
        -------
        CommandResult
            The unified :class:`lib.result.CommandResult` populated with
            ``argv``, the captured exit code, and UTF-8-decoded
            ``stdout``/``stderr`` (with ``replace`` errors). ``cwd`` is
            left ``None`` because the container runtime, not the harness,
            picks the working directory.

        Raises
        ------
        ValueError
            If ``argv`` is empty. The container's exec API requires at
            least one token and would otherwise produce a confusing
            error far from the call site.
        """

        if not argv:
            msg = "ContainerSession.run requires at least one argv token"
            raise ValueError(msg)
        wrapped = self._container.get_wrapped_container()
        exec_result = wrapped.exec_run(
            list(argv),
            demux=True,
            tty=False,
        )
        exit_code = int(exec_result.exit_code or 0)
        stdout_bytes, stderr_bytes = exec_result.output or (b"", b"")
        stdout = (stdout_bytes or b"").decode("utf-8", errors="replace")
        stderr = (stderr_bytes or b"").decode("utf-8", errors="replace")
        return CommandResult(
            argv=tuple(argv),
            exit_code=exit_code,
            stdout=stdout,
            stderr=stderr,
        )

    def run_shell(self, script: str) -> CommandResult:
        """Run ``script`` via ``/bin/sh -lc``.

        Reserved for genuine shell needs (pipes, compound cleanup, ``|| true``).
        Prefer :meth:`run` for everything else so command intent stays explicit.
        """

        return self.run("/bin/sh", "-lc", script)

    def must_run(self, *argv: str) -> CommandResult:
        """Like :meth:`run`, but raise :class:`ContainerCommandError` on failure."""

        result = self.run(*argv)
        if not result.ok:
            raise ContainerCommandError(result)
        return result

    def must_run_shell(self, script: str) -> CommandResult:
        """Like :meth:`run_shell`, but raise on non-zero exit."""

        result = self.run_shell(script)
        if not result.ok:
            raise ContainerCommandError(result)
        return result

    def copy_text(self, path: str, content: str, *, mode: int = 0o755) -> None:
        """Copy a UTF-8 text payload into the container.

        Parameters
        ----------
        path : str
            Absolute path inside the container where the file should
            land. Only the basename is used for the in-tar entry; the
            directory portion is the destination given to docker-py's
            ``put_archive`` and must already exist.
        content : str
            UTF-8 text content of the file. Encoded with the standard
            ``str.encode("utf-8")`` (no error replacement).
        mode : int, optional
            POSIX mode bits applied to the in-tar entry, defaulting to
            ``0o755``. The container's tar extraction honours this.
        """

        archive = io.BytesIO()
        with tarfile.open(fileobj=archive, mode="w") as tar:
            data = content.encode("utf-8")
            info = tarfile.TarInfo(name=Path(path).name)
            info.size = len(data)
            info.mode = mode
            tar.addfile(info, io.BytesIO(data))
        archive.seek(0)
        wrapped = self._container.get_wrapped_container()
        wrapped.put_archive(str(Path(path).parent), archive.getvalue())


def iter_packaging_files(repo_root: Path) -> Iterator[Path]:
    """Yield packaging asset paths copied into the test image at build time."""

    packaging = repo_root / "packaging"
    yield from sorted(packaging.rglob("*"))
