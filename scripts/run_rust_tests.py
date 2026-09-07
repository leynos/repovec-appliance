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

Two decisions the recipe made with shell are made properly here. Whether
``cargo nextest`` is installed decides which runner is used, and whether
any package declares doctests decides whether the doctest gate runs at
all; ``cargo test`` already runs doctests itself, so the extra gate
applies only to the nextest path. The doctest question is answered by
parsing ``cargo metadata`` rather than grepping its JSON for a substring.

Gates run in order and the first rejection ends the run, with the gate
named and Cargo's own exit code propagated so nextest's 100 is not
flattened to 1.

Child processes go through Plumbum because that is the runner
``docs/scripting-standards.md`` names. The estate is moving to Cuprum,
which this repository already uses for the integration harness in
``integration-tests/lib/commands.py``; switch when the standard does.
"""

from __future__ import annotations

import json
import shlex
import sys
from collections.abc import Iterator
from dataclasses import dataclass

import cyclopts
from cyclopts import App
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
    """

    name: str
    argv: list[str]


def split_flags(flags: str | None) -> list[str]:
    """Split a Make-supplied flag string into arguments.

    Make passes flag groups as one string, and an unset variable arrives
    as the empty string, which must contribute no argument at all rather
    than an empty one.
    """
    return shlex.split(flags or "")


def _nextest_available(cargo: BaseCommand) -> bool:
    """Report whether ``cargo nextest`` can be run."""
    code, _, _ = cargo["nextest", "--version"].run(retcode=None)
    return code == 0


def _declares_doctests(cargo: BaseCommand) -> bool:
    """Report whether any workspace target declares doctests.

    The metadata is parsed rather than pattern-matched, so a target named
    in a path or a feature string cannot be mistaken for a doctest
    declaration. Metadata that cannot be read or parsed answers no,
    matching the previous behaviour: the doctest gate is an addition to
    the unit-test gate, never the only one.
    """
    code, out, _ = cargo["metadata", "--no-deps", "--format-version", "1"].run(
        retcode=None
    )
    if code != 0:
        return False
    try:
        metadata = json.loads(out)
    except json.JSONDecodeError:
        return False
    return any(
        target.get("doctest", False)
        for package in metadata.get("packages", [])
        for target in package.get("targets", [])
    )


def plan_gates(
    cargo: BaseCommand,
    *,
    test_flags: list[str],
    doctest_flags: list[str],
    build_jobs: list[str],
) -> list[Gate]:
    """Return the ordered gates for the installed toolchain.

    With nextest present the unit tests run through it, and doctests need
    a separate gate because nextest does not run them. Without nextest,
    ``cargo test`` covers both, so one gate is the whole suite.
    """
    if not _nextest_available(cargo):
        return [Gate("unit tests", ["test", *test_flags, *build_jobs])]

    gates = [
        Gate(
            "unit tests",
            ["nextest", "run", "--no-tests", "pass", *test_flags, *build_jobs],
        )
    ]
    if _declares_doctests(cargo):
        gates.append(Gate("doctests", ["test", "--doc", *doctest_flags, *build_jobs]))
    return gates


def _run_gates(cargo: BaseCommand, gates: list[Gate]) -> Iterator[tuple[Gate, int]]:
    """Run each gate in turn, yielding its exit code, until one rejects.

    Cargo's output is inherited rather than captured, so a developer sees
    the failures as they happen instead of after the run.
    """
    for gate in gates:
        code, _, _ = cargo[gate.argv].run(retcode=None, stdout=None, stderr=None)
        yield gate, code
        if code != 0:
            return


@app.default
def main(
    *,
    cargo: str = "cargo",
    rust_flags: str = "-D warnings",
    test_flags: str = "",
    doctest_flags: str = "",
    build_jobs: str = "",
) -> None:
    """Run every Rust test gate, exiting at the first rejection.

    Parameters
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
    command = local[cargo]
    gates = plan_gates(
        command,
        test_flags=split_flags(test_flags),
        doctest_flags=split_flags(doctest_flags),
        build_jobs=split_flags(build_jobs),
    )
    with local.env(RUSTFLAGS=rust_flags):
        for gate, code in _run_gates(command, gates):
            if code != 0:
                print(
                    f"error: the {gate.name} gate rejected the workspace "
                    f"(exit code {code}); fix the reported failures and re-run",
                    file=sys.stderr,
                )
                sys.exit(code)


if __name__ == "__main__":
    app()
