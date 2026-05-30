"""Host-side command execution helpers backed by cuprum.

The harness reaches for the host shell in two situations:

1. Preflight and orchestration tasks (image hygiene, optional smoke runs).
2. cmd-mox tests that execute the helper script through a shimmed ``PATH``.

Both cases benefit from cuprum's typed catalogue: command intent is explicit,
arguments are stringly typed once at the builder level, and failures surface
structured stdout/stderr rather than opaque ``CalledProcessError`` traces.

Container-internal commands deliberately do **not** go through cuprum. The
``ContainerSession`` wrapper in :mod:`integration_tests.lib.container` already
owns the live container handle, so funnelling those calls through a podman
CLI would only add latency and obscure the transport.
"""

from __future__ import annotations

import dataclasses
import os
from collections.abc import Mapping
from pathlib import Path
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from cuprum import CommandResult, Program


def _build_catalogue() -> tuple[Program, ...]:
    """Return the host programs the integration harness is allowed to run.

    The catalogue is intentionally tiny: anything that talks to the live
    container should go via :class:`ContainerSession`, and anything that
    talks to remote services (``gh``, ``coderabbit``) is opt-in by the
    Makefile rather than the harness itself.
    """

    from cuprum import Program

    return (
        Program("python3"),
        Program("podman"),
        Program("git"),
        Program("gh"),
        Program("coderabbit"),
        Program("sh"),
    )


def host_catalogue() -> "ProgramCatalogue":  # type: ignore[name-defined]
    """Return the cuprum :class:`ProgramCatalogue` shared by host-side helpers."""

    from cuprum import ProgramCatalogue, ProjectSettings

    project = ProjectSettings(
        name="repovec-integration-tests",
        programs=_build_catalogue(),
        documentation_locations=("integration-tests/README.md",),
        noise_rules=(r"^Warning:",),
    )
    return ProgramCatalogue(projects=(project,))


@dataclasses.dataclass(frozen=True)
class HostCommandResult:
    """Result wrapper that mirrors :class:`ContainerSession` semantics.

    Tests and helpers consume this rather than the raw cuprum result so
    diagnostics render the same way whether a failure happened on the host
    or inside the container.
    """

    argv: tuple[str, ...]
    exit_code: int
    stdout: str
    stderr: str
    cwd: str

    @property
    def ok(self) -> bool:
        return self.exit_code == 0

    def render(self) -> str:
        parts = [
            f"command: {' '.join(self.argv)}",
            f"cwd: {self.cwd}",
            f"exit_code: {self.exit_code}",
        ]
        if self.stdout:
            parts.append(f"stdout:\n{self.stdout.rstrip()}")
        if self.stderr:
            parts.append(f"stderr:\n{self.stderr.rstrip()}")
        return "\n".join(parts)


def run_host(
    program: str,
    *args: str,
    env: Mapping[str, str] | None = None,
    cwd: str | os.PathLike[str] | None = None,
) -> HostCommandResult:
    """Execute a curated host command via cuprum and normalise its result.

    ``program`` must be present in :func:`host_catalogue`; cuprum raises
    ``UnknownProgramError`` for anything else, which is the intended safety
    rail. Callers wanting a different program should add it to the catalogue
    instead of bypassing this helper.

    Note that cuprum's :class:`ExecutionContext` does not currently expose a
    wall-clock timeout; long-running host commands are the caller's
    responsibility to bound (typically via pytest's own timeout plugin).
    """

    from cuprum import ExecutionContext, Program, sh

    catalogue = host_catalogue()
    builder = sh.make(Program(program), catalogue=catalogue)
    cmd = builder(*[str(a) for a in args])

    resolved_cwd = str(Path(cwd).resolve()) if cwd is not None else str(Path.cwd())
    context = ExecutionContext(
        env=dict(env) if env is not None else None,
        cwd=resolved_cwd,
    )

    result: CommandResult = cmd.run_sync(context=context, capture=True)
    return HostCommandResult(
        argv=(program, *[str(a) for a in args]),
        exit_code=int(result.exit_code),
        stdout=result.stdout or "",
        stderr=result.stderr or "",
        cwd=resolved_cwd,
    )
