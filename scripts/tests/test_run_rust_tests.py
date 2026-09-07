"""Unit coverage for the Rust test-gate runner.

The runner exists because sequencing gates in shell lost failures. These
tests hold it to the property the shell version lacked: a rejection from
any gate ends the run, later gates do not run, and the process exits
non-zero.

``cmd-mox`` supplies the ``cargo`` executable, so no Rust toolchain is
touched and a failing gate can be staged deterministically. Assertions
are made on the recorded invocation sequence rather than only on the
exit code, because "stopped at the first rejection" and "ran everything
and reported the last one" produce the same code in some orderings and
differ only in what ran.
"""

from __future__ import annotations

import json
import os
from collections.abc import Iterator
from contextlib import contextmanager
from dataclasses import dataclass

import pytest
from plumbum import local

import run_rust_tests
from run_rust_tests import Gate, main, plan_gates

#: Metadata for a workspace whose library target declares doctests.
METADATA_WITH_DOCTESTS = json.dumps(
    {"packages": [{"targets": [{"name": "demo", "doctest": True}]}]}
)

#: Metadata for a workspace where no target declares doctests.
METADATA_WITHOUT_DOCTESTS = json.dumps(
    {"packages": [{"targets": [{"name": "demo", "doctest": False}]}]}
)

#: The two questions the runner asks before planning any gate.
NEXTEST_PROBE = ["nextest", "--version"]
METADATA_QUERY = ["metadata", "--no-deps", "--format-version", "1"]


@dataclass
class CargoScript:
    """Scripted Cargo behaviour, dispatched on the leading arguments.

    ``cmd-mox`` keeps one double per command name, so the several
    distinct Cargo invocations a run makes are answered by one handler
    rather than by several mocks.

    Attributes
    ----------
    nextest_available
        Whether the nextest probe succeeds.
    metadata
        Standard output returned for the metadata query.
    unit_exit
        Exit code for the unit-test gate.
    doctest_exit
        Exit code for the doctest gate.
    """

    nextest_available: bool = True
    metadata: str = METADATA_WITH_DOCTESTS
    unit_exit: int = 0
    doctest_exit: int = 0

    def __call__(self, invocation) -> tuple[str, str, int]:
        """Answer one Cargo invocation."""
        match invocation.args:
            case ["nextest", "--version"]:
                return ("", "", 0 if self.nextest_available else 1)
            case ["metadata", *_]:
                return (self.metadata, "", 0)
            case ["nextest", "run", *_]:
                return ("", "", self.unit_exit)
            case ["test", "--doc", *_]:
                return ("", "", self.doctest_exit)
            case ["test", *_]:
                return ("", "", self.unit_exit)
            case unexpected:
                message = f"unexpected cargo invocation: {unexpected}"
                raise AssertionError(message)


def stage_cargo(cmd_mox, script: CargoScript):
    """Install the scripted Cargo double and return its spy."""
    return cmd_mox.spy("cargo").runs(script)


def invocations(spy) -> list[list[str]]:
    """Return the argument list of every recorded Cargo invocation."""
    return [list(invocation.args) for invocation in spy.invocations]


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


def test_a_failing_unit_test_run_stops_the_runner(cmd_mox) -> None:
    """Scenario: nextest rejects the workspace.

    Invariant: the doctest gate never runs and the process exits with
    nextest's own code. This is the defect that let the shell recipe
    report a green suite while tests failed.
    """
    cargo = stage_cargo(cmd_mox, CargoScript(unit_exit=100))

    with replaying(cmd_mox), pytest.raises(SystemExit) as exit_info:
        main()

    assert exit_info.value.code == 100
    assert invocations(cargo) == [
        NEXTEST_PROBE,
        METADATA_QUERY,
        ["nextest", "run", "--no-tests", "pass"],
    ]


def test_a_failing_doctest_run_fails_the_runner(cmd_mox) -> None:
    """Scenario: unit tests pass and the doctest gate rejects.

    Invariant: the passing gate that ran first does not mask the later
    rejection.
    """
    cargo = stage_cargo(cmd_mox, CargoScript(doctest_exit=101))

    with replaying(cmd_mox), pytest.raises(SystemExit) as exit_info:
        main()

    assert exit_info.value.code == 101
    assert invocations(cargo)[-1] == ["test", "--doc"]


def test_all_gates_passing_exits_cleanly(cmd_mox) -> None:
    """Scenario: every gate accepts the workspace.

    Invariant: the runner returns without raising, and both gates ran, so
    a green result is not reached by skipping one.
    """
    cargo = stage_cargo(cmd_mox, CargoScript())

    with replaying(cmd_mox):
        main()

    assert invocations(cargo) == [
        NEXTEST_PROBE,
        METADATA_QUERY,
        ["nextest", "run", "--no-tests", "pass"],
        ["test", "--doc"],
    ]


def test_flags_reach_the_gate_they_belong_to(cmd_mox) -> None:
    """Scenario: the caller supplies distinct unit-test and doctest flags.

    Invariant: each flag group reaches its own gate and the job count
    reaches both, so a Makefile change cannot silently narrow one gate.
    """
    cargo = stage_cargo(cmd_mox, CargoScript())

    with replaying(cmd_mox):
        main(
            test_flags="--all-targets --all-features",
            doctest_flags="--workspace --all-features",
            build_jobs="-j2",
        )

    assert invocations(cargo)[2:] == [
        [
            "nextest",
            "run",
            "--no-tests",
            "pass",
            "--all-targets",
            "--all-features",
            "-j2",
        ],
        ["test", "--doc", "--workspace", "--all-features", "-j2"],
    ]


def test_rust_flags_are_exported_to_the_gates(cmd_mox) -> None:
    """Scenario: the caller denies warnings through RUSTFLAGS.

    Invariant: the gate runs see the value, since warnings-as-errors is
    the point of this gate.
    """
    cargo = stage_cargo(cmd_mox, CargoScript())

    with replaying(cmd_mox):
        main(rust_flags="-D warnings")

    gate_runs = [
        invocation
        for invocation in cargo.invocations
        if list(invocation.args)[:2] in (["nextest", "run"], ["test", "--doc"])
    ]
    assert gate_runs
    for invocation in gate_runs:
        assert invocation.env.get("RUSTFLAGS") == "-D warnings"


def test_without_nextest_a_single_gate_runs_the_whole_suite(cmd_mox) -> None:
    """Scenario: cargo-nextest is not installed.

    Invariant: one ``cargo test`` gate is planned, because it already
    runs doctests, so no separate doctest gate is added and the metadata
    question is never asked.
    """
    cargo = stage_cargo(cmd_mox, CargoScript(nextest_available=False))

    with replaying(cmd_mox):
        gates = plan_gates(
            local["cargo"], test_flags=[], doctest_flags=[], build_jobs=[]
        )

    assert gates == [Gate("unit tests", ["test"])]
    assert invocations(cargo) == [NEXTEST_PROBE]


def test_metadata_without_doctests_plans_no_doctest_gate(cmd_mox) -> None:
    """Scenario: no workspace target declares doctests.

    Invariant: the doctest gate is omitted rather than run against a
    workspace that has none.
    """
    stage_cargo(cmd_mox, CargoScript(metadata=METADATA_WITHOUT_DOCTESTS))

    with replaying(cmd_mox):
        gates = plan_gates(
            local["cargo"], test_flags=[], doctest_flags=[], build_jobs=[]
        )

    assert [gate.name for gate in gates] == ["unit tests"]


@pytest.mark.parametrize(
    "metadata",
    ["not json at all", json.dumps({"packages": []})],
    ids=["unparseable", "no-packages"],
)
def test_unusable_metadata_keeps_the_unit_test_gate(cmd_mox, metadata: str) -> None:
    """Scenario: ``cargo metadata`` returns something unusable.

    Invariant: the unit-test gate still runs. The doctest gate is an
    addition, so an unanswerable question must never remove the gate that
    carries the suite.
    """
    stage_cargo(cmd_mox, CargoScript(metadata=metadata))

    with replaying(cmd_mox):
        gates = plan_gates(
            local["cargo"], test_flags=[], doctest_flags=[], build_jobs=[]
        )

    assert [gate.name for gate in gates] == ["unit tests"]


@pytest.mark.parametrize("flags", ["", None], ids=["empty-string", "unset"])
def test_empty_flag_strings_contribute_no_arguments(flags: str | None) -> None:
    """Scenario: Make expands an unset variable to the empty string.

    Invariant: the gate receives no argument at all, rather than an empty
    one that Cargo would reject.
    """
    assert run_rust_tests.split_flags(flags) == []
