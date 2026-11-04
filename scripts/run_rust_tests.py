#!/usr/bin/env -S uv run python
# /// script
# requires-python = ">=3.13"
# dependencies = ["cyclopts==4.25.0", "plumbum==2.0.2"]
# ///
"""Run the workspace's Rust test gates, stopping at the first rejection.

This logic used to be a chain of shell commands in the ``test`` recipe.
A Make recipe is one shell invocation without ``set -e``, so its status
is that of its last command: the unit-test rejection was discarded by
the doctest step that ran after it, and ``make test`` reported success
with failing tests. Rather than sprinkle ``|| exit 1`` through a growing
shell chain, the sequencing lives here, where it is unit-tested.

One decision the recipe made with shell is made here: whether
``cargo nextest`` is installed, which selects the unit-test runner. The
doctest gate is not conditional on anything, because every condition the
recipe applied to it could remove it silently.

Gates run in order and the first rejection ends the run, with the gate
named and Cargo's own exit code propagated so nextest's 100 is not
flattened to 1.

The doctest gate must also *find* doctests. Cargo reports success when it
collects none, so an unconditional gate that passes with zero tests
cannot tell a documented workspace from one whose examples were lost in
a refactor. The gate therefore reads the harness summary and rejects a
run that reported no passing doctests.

Child processes go through Plumbum because that is the runner
``docs/scripting-standards.md`` names. The estate is moving to Cuprum,
which this repository already uses for the integration harness in
``integration-tests/lib/commands.py``; switch when the standard does.
"""

from __future__ import annotations

import re
import shlex
import sys
from collections.abc import Iterator
from dataclasses import dataclass

import cyclopts
from cyclopts import App, Parameter
from plumbum import local
from plumbum.commands.base import BaseCommand

app = App(config=cyclopts.config.Env("INPUT_", command=False))


@dataclass(frozen=True)
class Gate:
    """One Cargo invocation and the name reported when it rejects.

    Attributes
    ----------
    name
        Human-readable gate name, used in the failure message.
    argv
        Arguments passed to Cargo, excluding the executable itself.
    requires_tests
        Whether a successful run must also report at least one passing
        test. This is what makes a doctest gate fail on an empty
        collection rather than on a non-zero exit alone.

    Examples
    --------
    >>> Gate("unit tests", ["nextest", "run"]).requires_tests
    False
    >>> Gate("doctests", ["test", "--doc"], requires_tests=True).name
    'doctests'
    """

    name: str
    argv: list[str]
    requires_tests: bool = False


@Parameter(name="*")
@dataclass(frozen=True)
class GateOptions:
    """Configuration the caller supplies, one field per flag.

    Attributes
    ----------
    cargo
        Cargo executable, by name or absolute path.
    rust_flags
        Value exported as ``RUSTFLAGS`` for the gate runs.
    test_flags
        Flags for the unit-test run, as one shell-quoted string.
    doctest_flags
        Flags for the doctest run, as one shell-quoted string.
    build_jobs
        Optional job-count flag applied to every gate.
    """

    cargo: str = "cargo"
    rust_flags: str = "-D warnings"
    test_flags: str = ""
    doctest_flags: str = ""
    build_jobs: str = ""


def split_flags(flags: str | None) -> list[str]:
    """Split a Make-supplied flag string into arguments.

    Make passes flag groups as one string, and an unset variable arrives
    as the empty string, which must contribute no argument at all rather
    than an empty one.
    """
    return shlex.split(flags or "")


DOCTEST_RESULT = re.compile(r"^test result: ok\. (\d+) passed", re.MULTILINE)


def count_passed(output: str) -> int:
    """Count the passing tests in Cargo's harness summaries.

    Cargo prints one summary line per test binary, so the workspace total
    is the sum of every ``passed`` field rather than the last one.

    Examples
    --------
    >>> count_passed("test result: ok. 7 passed; 0 failed; 0 ignored")
    7
    >>> count_passed("running 0 tests")
    0
    """
    return sum(int(match.group(1)) for match in DOCTEST_RESULT.finditer(output))


def _nextest_available(cargo: BaseCommand) -> bool:
    """Report whether ``cargo nextest`` can be run."""
    code, _, _ = cargo["nextest", "--version"].run(retcode=None)
    return code == 0


def plan_gates(
    cargo: BaseCommand,
    *,
    test_flags: list[str],
    doctest_flags: list[str],
    build_jobs: list[str],
) -> list[Gate]:
    """Return the ordered gates for the installed toolchain.

    Only the unit-test runner varies: nextest when it is installed,
    ``cargo test`` otherwise. The doctest gate is unconditional, and runs
    under its own flags on both paths.

    It used to be conditional, scheduled only on the nextest path and
    only when ``cargo metadata`` said some target declared doctests. Both
    conditions were wrong. Metadata that could not be read or parsed
    answered "no", so an unreadable manifest silently removed a gate.
    And on the ``cargo test`` path the doctests were assumed covered by
    the unit-test run, which holds only until the caller passes
    ``--all-targets``; that flag excludes the doc target, so this
    repository's continuous integration ran no doctests at all
    (repovec-appliance #107).

    Running the gate unconditionally costs one extra compilation. The
    gate also requires at least one passing doctest, so a run that
    collects nothing is a rejection rather than a pass: this repository
    documents its public API with examples, and losing them must be
    visible to the gate rather than to a later reviewer.
    """
    unit_test_argv = (
        ["nextest", "run", "--no-tests", "pass", *test_flags, *build_jobs]
        if _nextest_available(cargo)
        else ["test", *test_flags, *build_jobs]
    )
    return [
        Gate("unit tests", unit_test_argv),
        Gate(
            "doctests",
            ["test", "--doc", *doctest_flags, *build_jobs],
            requires_tests=True,
        ),
    ]


def _run_gates(
    cargo: BaseCommand, gates: list[Gate]
) -> Iterator[tuple[Gate, int, str]]:
    """Run each gate in turn, yielding it, its exit code and its output.

    A gate that must count tests has its output captured and replayed, so
    the count can be read from the harness summary. Every other gate
    inherits the terminal, so a developer sees failures as they happen.

    Examples
    --------
    Running ``[Gate("doctests", ["test", "--doc"], requires_tests=True)]``
    yields one ``(gate, code, out)`` tuple whose ``out`` holds the
    captured harness summary, so the caller can count passing tests. A
    gate without ``requires_tests`` yields ``out == ""`` because its
    output went straight to the terminal.
    """
    for gate in gates:
        if gate.requires_tests:
            code, out, err = cargo[gate.argv].run(retcode=None)
            print(out, end="")
            print(err, end="", file=sys.stderr)
        else:
            code, _, _ = cargo[gate.argv].run(retcode=None, stdout=None, stderr=None)
            out = ""
        yield gate, code, out
        if code != 0:
            return


@app.default
def main(*, options: GateOptions = GateOptions()) -> None:
    """Run every Rust test gate, exiting at the first rejection.

    Parameters
    ----------
    options
        Runner configuration. ``Parameter(name="*")`` flattens it, so
        each field is still spelled as its own command-line flag.

    Examples
    --------
    ``make test`` runs both gates and exits ``0`` when they pass.
    A failing unit test propagates the runner's exit code instead of
    being masked by the doctest gate that follows. A doctest gate that
    collects nothing exits ``1`` with a message naming the gate.
    """
    command = local[options.cargo]
    gates = plan_gates(
        command,
        test_flags=split_flags(options.test_flags),
        doctest_flags=split_flags(options.doctest_flags),
        build_jobs=split_flags(options.build_jobs),
    )
    with local.env(RUSTFLAGS=options.rust_flags):
        for gate, code, output in _run_gates(command, gates):
            if code != 0:
                print(
                    f"error: the {gate.name} gate rejected the workspace "
                    f"(exit code {code}); fix the reported failures and re-run",
                    file=sys.stderr,
                )
                sys.exit(code)
            if gate.requires_tests and count_passed(output) == 0:
                print(
                    f"error: the {gate.name} gate ran no tests; a gate that "
                    "passes on an empty collection cannot detect lost examples",
                    file=sys.stderr,
                )
                sys.exit(1)


if __name__ == "__main__":
    app()
