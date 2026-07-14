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

import functools
import os
from collections.abc import Mapping
from pathlib import Path
from typing import TYPE_CHECKING

from .result import CommandResult

if TYPE_CHECKING:
    from cuprum import CommandResult as CuprumCommandResult
    from cuprum import Program, ProgramCatalogue


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


@functools.lru_cache(maxsize=None)
def host_catalogue() -> ProgramCatalogue:
    """Build the host-side cuprum program catalogue.

    Returns
    -------
    ProgramCatalogue
        Curated catalogue shared by every host-side helper in the
        harness. Programs not present in this catalogue raise
        ``cuprum.UnknownProgramError`` when invoked through
        :func:`run_host`; that exception is the safety rail that
        keeps ad hoc shell-outs from sneaking into test code.

    Notes
    -----
    The result is cached for the lifetime of the process so the
    ``ProjectSettings`` and ``ProgramCatalogue`` are constructed once
    rather than on every :func:`run_host` call.
    """

    from cuprum import ProgramCatalogue, ProjectSettings

    project = ProjectSettings(
        name="repovec-integration-tests",
        programs=_build_catalogue(),
        documentation_locations=("integration-tests/README.md",),
        noise_rules=(r"^Warning:",),
    )
    return ProgramCatalogue(projects=(project,))


def run_host(
    program: str,
    *args: str,
    env: Mapping[str, str] | None = None,
    cwd: str | os.PathLike[str] | None = None,
) -> CommandResult:
    """Execute a curated host command via cuprum and normalize its result.

    Parameters
    ----------
    program : str
        Name of the program to run. Must be present in
        :func:`host_catalogue`; anything else raises
        :class:`cuprum.UnknownProgramError`.
    *args : str
        Positional arguments passed to ``program``. Each value is
        coerced via :class:`str` before reaching cuprum.
    env : Mapping[str, str] or None, optional
        Environment overlay applied on top of the live ``os.environ``
        at invocation time (cuprum semantics). ``None`` runs the
        command with the unmodified ``os.environ``.
    cwd : str or os.PathLike[str] or None, optional
        Working directory for the child process. Defaults to the
        current Python working directory.

    Returns
    -------
    CommandResult
        The unified :class:`lib.result.CommandResult`, with ``cwd``
        populated so host-side diagnostics include the working
        directory the command ran in. The same type is returned by
        :class:`lib.container.ContainerSession` (with ``cwd=None``),
        so failures look identical regardless of which side of the
        boundary the command lived on.

    Raises
    ------
    cuprum.UnknownProgramError
        If ``program`` is not in :func:`host_catalogue`. This is the
        intended safety rail against ad hoc shell-outs leaking into
        test code; callers wanting a different program should extend
        the catalogue rather than bypassing this helper.

    Notes
    -----
    Cuprum's :class:`cuprum.ExecutionContext` does not currently
    expose a wall-clock timeout, so long-running host commands are
    the caller's responsibility to bound (typically via pytest's own
    timeout plugin).
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

    cuprum_result: CuprumCommandResult = cmd.run_sync(
        context=context,
        capture=True,
    )
    return CommandResult(
        argv=(program, *[str(a) for a in args]),
        exit_code=int(cuprum_result.exit_code),
        stdout=cuprum_result.stdout or "",
        stderr=cuprum_result.stderr or "",
        cwd=resolved_cwd,
    )
