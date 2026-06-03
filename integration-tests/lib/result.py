"""Shared command-result value type.

Both the host-side ``cuprum`` runner in :mod:`lib.commands` and the
in-container docker-py exec API wrapped by :class:`lib.container.ContainerSession`
produce the same shape of result: an ``argv``, an exit code, captured streams,
and (for host-side calls) the working directory the command ran in. Keeping a
single :class:`CommandResult` makes test diagnostics render identically
regardless of which side of the boundary the command lived on, and removes
the maintenance cost of two near-identical render implementations.
"""

from __future__ import annotations

import dataclasses
import shlex


@dataclasses.dataclass(frozen=True)
class CommandResult:
    """Outcome of a single command execution.

    Attributes
    ----------
    argv : tuple[str, ...]
        Command line as a sequence of tokens.
    exit_code : int
        Exit status of the spawned process.
    stdout : str
        Captured standard output, decoded as UTF-8 with replacement.
    stderr : str
        Captured standard error, decoded as UTF-8 with replacement.
    cwd : str or None
        Working directory the command ran in. ``None`` for in-container
        invocations where ``cwd`` is set by the container runtime rather
        than by the harness.
    """

    argv: tuple[str, ...]
    exit_code: int
    stdout: str
    stderr: str
    cwd: str | None = None

    @property
    def ok(self) -> bool:
        """Return ``True`` iff the command exited with status zero."""

        return self.exit_code == 0

    def render(self) -> str:
        """Return a multi-line diagnostic suitable for assertion failures.

        ``argv`` is joined with :func:`shlex.join` so tokens with spaces or
        shell metacharacters round-trip cleanly. ``cwd`` is omitted when
        absent, so container-side diagnostics stay tight.

        Returns
        -------
        str
            Multi-line diagnostic containing ``command:``, an optional
            ``cwd:`` line, ``exit_code:``, and trimmed ``stdout:`` /
            ``stderr:`` blocks when present.
        """

        parts = [f"command: {shlex.join(self.argv)}"]
        if self.cwd is not None:
            parts.append(f"cwd: {self.cwd}")
        parts.append(f"exit_code: {self.exit_code}")
        if self.stdout:
            parts.append(f"stdout:\n{self.stdout.rstrip()}")
        if self.stderr:
            parts.append(f"stderr:\n{self.stderr.rstrip()}")
        return "\n".join(parts)
