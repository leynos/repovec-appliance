"""Unit coverage for the Whitaker gate runner.

The runner exists because the Make recipe treated a missing Whitaker as a
skip, which made an absent gate indistinguishable from a passing one.
These tests hold the script to the properties the recipe lacked: an
absent executable fails with a message that says how to install it, a
rejecting suite propagates its own status, a passing suite returns zero,
and the arguments reach the executable unchanged.

``cmd-mox`` supplies the Whitaker executable, so no suite is installed and
no Rust is compiled. Assertions are made on the recorded invocation as
well as the exit status, because a script that silently rewrites an
argument can still exit zero.
"""

from __future__ import annotations

import os
from collections.abc import Iterator
from contextlib import contextmanager

import pytest
from plumbum import local

from run_whitaker import WhitakerOptions, main

#: A name that only the cmd-mox shim answers, so a real installation on
#: the host PATH cannot satisfy the lookup by accident.
WHITAKER_STUB = "whitaker-cmd-mox-stub"


def stage_whitaker(cmd_mox, exit_code: int):
    """Install a Whitaker double that exits with *exit_code* and return its spy."""
    return cmd_mox.spy(WHITAKER_STUB).runs(lambda _invocation: ("", "", exit_code))


@contextmanager
def replaying(cmd_mox) -> Iterator[None]:
    """Enter replay with plumbum able to see the shims.

    Plumbum snapshots the environment when it is imported, so a child it
    starts would neither find the shim that ``replay()`` puts on ``PATH``
    nor carry the address the shim reports back through. Overlaying the
    live environment fixes both, and must happen after ``replay()``
    rather than at fixture setup.
    """
    cmd_mox.replay()
    with local.env(**os.environ):
        yield
    cmd_mox.verify()


def test_an_absent_executable_fails_with_an_actionable_message(
    tmp_path, capsys
) -> None:
    """Scenario: Whitaker is not installed.

    Invariant: the run fails instead of reporting a skip, and the message
    names both the missing executable and the variable that can point at
    another one.
    """
    missing = str(tmp_path / "not-installed")

    with pytest.raises(SystemExit) as exit_info:
        main(options=WhitakerOptions(whitaker=missing))

    assert exit_info.value.code == 1
    stderr = capsys.readouterr().err
    assert missing in stderr
    assert "set WHITAKER to its path" in stderr


def test_a_failing_suite_propagates_its_exit_status(cmd_mox) -> None:
    """Scenario: the suite rejects the workspace with status 7.

    Invariant: the process exits 7, so a Make recipe reports the gate's
    own rejection rather than a generic or successful status.
    """
    stage_whitaker(cmd_mox, 7)

    with replaying(cmd_mox), pytest.raises(SystemExit) as exit_info:
        main(options=WhitakerOptions(whitaker=WHITAKER_STUB))

    assert exit_info.value.code == 7


def test_a_successful_suite_returns_cleanly(cmd_mox) -> None:
    """Scenario: the suite accepts the workspace.

    Invariant: the runner returns without raising, so the target succeeds
    only after the suite has actually run.
    """
    stage_whitaker(cmd_mox, 0)

    with replaying(cmd_mox):
        main(options=WhitakerOptions(whitaker=WHITAKER_STUB))


def test_arguments_reach_the_executable_unchanged(cmd_mox) -> None:
    """Scenario: the Make recipe supplies package and Cargo flags.

    Invariant: every argument reaches Whitaker in order and without
    rewriting, so the reviewed ``--all --workspace`` route cannot drift
    into a narrower check unnoticed.
    """
    spy = stage_whitaker(cmd_mox, 0)

    with replaying(cmd_mox):
        main(
            options=WhitakerOptions(
                whitaker=WHITAKER_STUB,
                packages="--workspace",
                cargo_flags="--all-targets --all-features",
            )
        )

    assert [list(invocation.args) for invocation in spy.invocations] == [
        ["--all", "--workspace", "--", "--all-targets", "--all-features"]
    ]
