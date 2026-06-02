"""Testcontainers-backed harness for the provisioning helper.

The harness builds a Fedora image containing Podman, the helper script, and
its sibling packaging assets, then starts a privileged container so rootful
Podman-in-Podman can create and inspect secrets. A small :class:`ContainerSession`
wrapper hides the docker-py exec API behind argv-first methods so test code
reads as command intent rather than transport plumbing.
"""

from __future__ import annotations

import dataclasses
import io
import shlex
import tarfile
from collections.abc import Iterator
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from testcontainers.core.container import DockerContainer


REPOVEC_CLEANUP_SCRIPT = """\
set -eu
podman secret rm repovec-qdrant-api-key >/dev/null 2>&1 || true
if getent passwd repovec >/dev/null; then
    userdel -r repovec >/dev/null 2>&1 || userdel repovec >/dev/null 2>&1 || true
fi
rm -rf /etc/repovec /var/lib/repovec
"""


@dataclasses.dataclass(frozen=True)
class CommandResult:
    """Outcome of a single command executed inside the container."""

    argv: tuple[str, ...]
    exit_code: int
    stdout: str
    stderr: str

    @property
    def ok(self) -> bool:
        """Return ``True`` iff the command exited with status zero."""

        return self.exit_code == 0

    def render(self) -> str:
        """Return a multi-line diagnostic suitable for assertion failures."""

        rendered_argv = " ".join(shlex.quote(part) for part in self.argv)
        parts = [
            f"command: {rendered_argv}",
            f"exit_code: {self.exit_code}",
        ]
        if self.stdout:
            parts.append(f"stdout:\n{self.stdout.rstrip()}")
        if self.stderr:
            parts.append(f"stderr:\n{self.stderr.rstrip()}")
        return "\n".join(parts)


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
        """Execute ``argv`` inside the container without raising on failure."""

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

    def cleanup_state(self) -> None:
        """Reset the helper's externally visible state between tests.

        The ``podman secret rm`` and ``userdel`` steps are best-effort
        (``|| true``) because a missing secret or user is the expected
        starting point for a fresh test. ``rm -rf`` is *not* best-effort:
        ``-rf`` already silences missing targets, so a non-zero exit there
        means a genuine filesystem error (permission denied, busy file,
        I/O error) that callers must see rather than swallow.

        Any non-zero exit therefore surfaces as a
        :class:`ContainerCommandError`. Callers that want to keep running
        on cleanup failure should wrap the call in their own ``try`` —
        see the ``container_session`` pytest fixture's post-test branch
        for the canonical pattern.
        """

        self.must_run_shell(REPOVEC_CLEANUP_SCRIPT)

    def copy_text(self, path: str, content: str, *, mode: int = 0o755) -> None:
        """Copy a UTF-8 text payload to ``path`` inside the container."""

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
