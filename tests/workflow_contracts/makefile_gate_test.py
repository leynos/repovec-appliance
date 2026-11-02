"""Contract tests for the Make gate recipes CI depends on.

A Make recipe is one shell invocation, run without ``set -e``, so its
exit status is that of its *last* command. A recipe that chains several
commands with ``;`` therefore reports only the last one, and a rejection
from any earlier command is discarded.

That was not hypothetical here. The ``test`` recipe used to chain a
nextest run and a conditional doctest run. Measured on 2026-09-07, with
a deliberately failing unit test in the workspace, ``make test`` exited
0: nextest reported ``1 failed`` and the doctest step that ran
afterwards supplied the recipe's zero status. CI's ``test`` job would
have passed with failing tests.

The remedy is not a guard on the shell but the removal of the shell.
Per ``docs/scripting-standards.md``, multi-command gate logic belongs in
a Python script where it is unit-tested, so the recipe now runs
``scripts/run_rust_tests.py`` as a single command and the sequencing is
covered by ``scripts/tests/test_run_rust_tests.py``.

Two contracts follow. The first keeps that recipe a single command, so
the logic cannot drift back into the Makefile. The second is the general
rule for any recipe that still chains gate commands: every one but the
last must carry ``|| exit 1``.

Mutation proof, recorded 2026-09-07; each applied alone and reverted:

- appending ``; echo done`` to the ``test`` recipe fails
  ``test_the_recipe_runs_exactly_one_command``;
- replacing the runner invocation with a direct ``cargo test`` fails
  ``test_the_single_command_recipe_runs_the_tested_runner``;
- joining the ``lint`` recipe's first two commands with ``;`` fails
  ``test_every_chained_recipe_guards_its_gate_commands``;
- appending ``|| true`` to the ``test`` recipe fails
  ``test_the_recipe_runs_exactly_one_command``;
- prefixing the ``test`` recipe or the ``lint`` recipe's Clippy command
  with Make's ``-`` fails ``test_the_recipe_runs_exactly_one_command``
  and ``test_no_gate_command_has_its_status_ignored`` respectively.

Both of those last two leave the command in place and readable, which is
why a contract that only looks for the command certifies nothing.

Added 2026-09-08, after review found the guard rule too narrow:

- appending ``; echo done`` to the ``lint`` recipe's Clippy command fails
  ``test_no_gate_command_is_followed_by_an_unguarded_command``, and the
  same line with ``|| exit 1`` before the ``echo`` passes, so the rule
  rejects the masking rather than the sequencing;
- the same trailing ``echo`` inside the ``whitaker-lint`` ``then``
  branch fails it too;
- ``|| true`` on the Whitaker command fails
  ``test_no_gate_command_alters_its_own_status``.

The earlier rule compared each gate command only against *later gate*
commands, so ``$(CARGO) test; echo done`` was checked against an empty
list and the successful ``echo`` supplied the recipe's status.

Added 2026-11-02, when the Whitaker leg became binding; each mutation was
applied alone and reverted, and the observed failure is recorded:

- removing ``lint-whitaker`` from the ``lint`` recipe fails
  ``test_lint_delegates_to_both_lint_routes``;
- replacing the recursive-make sequencing with a plain prerequisite list
  fails ``test_lint_delegates_to_both_lint_routes`` and
  ``test_make_keeps_the_lint_routes_sequential``;
- restoring the ``command -v`` skip in the Whitaker recipe fails
  ``test_the_whitaker_recipe_runs_the_tested_script_as_one_command``,
  ``test_lint_routes_are_unconditional``, and
  ``test_a_missing_whitaker_fails_the_target``;
- appending ``|| true`` to the Whitaker command fails
  ``test_lint_routes_are_unconditional``;
- replacing ``scripts/run_whitaker.py`` with a direct ``whitaker`` call
  fails ``test_the_whitaker_recipe_runs_the_tested_script_as_one_command``;
- discarding a rejecting stub's status fails
  ``test_a_rejecting_whitaker_fails_the_target``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import os
import shlex
import subprocess

import pytest

from makefile_gate_support import (
    APPROVED_GUARD,
    CONDITION_RE,
    MAKEFILE_PATH,
    STATUS_ALTERING_OPERATORS,
    gate_commands,
    ignores_errors,
    logical_lines,
    masking_followers,
    strip_prefixes,
)

pytestmark = pytest.mark.skipif(
    not MAKEFILE_PATH.exists(),
    reason="Makefile not present in this working copy (e.g. inside "
    "mutmut's mutants/ sandbox)",
)

#: Recipes whose gate logic must stay out of the Makefile entirely.
SINGLE_COMMAND_RECIPES = ("test",)


@pytest.mark.parametrize("target", SINGLE_COMMAND_RECIPES)
def test_the_recipe_runs_exactly_one_command(target: str) -> None:
    """Scenario: gate sequencing drifts back into the Makefile.

    Invariant: the recipe is one shell invocation running one command,
    so there is no second command whose status could mask the first.
    """
    recipe = logical_lines(target)

    assert len(recipe) == 1, (
        f"the {target} recipe should be one command, found {len(recipe)}: {recipe}"
    )
    statements = [part for part in recipe[0].split(";") if part.strip()]
    assert len(statements) == 1, (
        f"the {target} recipe chains {len(statements)} commands; move the "
        "sequencing into a tested script per docs/scripting-standards.md"
    )

    command = statements[0].strip()
    assert not ignores_errors(command), (
        f"the {target} recipe is prefixed with '-', so Make ignores its "
        "status and the gate can never fail the build"
    )
    for operator in STATUS_ALTERING_OPERATORS:
        assert operator not in strip_prefixes(command), (
            f"the {target} recipe joins its command with {operator!r}, so the "
            "recipe's status is no longer the gate's verdict; a gate command "
            "is a bare invocation"
        )


def test_the_single_command_recipe_runs_the_tested_runner() -> None:
    """Scenario: the recipe stays one command but stops running the script.

    Invariant: `make test` invokes the runner that carries the unit-tested
    sequencing, so the single-command contract cannot be satisfied by an
    untested one-liner.
    """
    assert "$(RUST_TEST_RUNNER)" in logical_lines("test")[0], (
        "the test recipe should invoke the Rust test-gate runner"
    )


def test_no_gate_command_has_its_status_ignored() -> None:
    """Scenario: a gate command is prefixed with Make's ``-``.

    Invariant: no recipe tells Make to ignore a gate tool's status. The
    prefix leaves the command in place and readable, so a contract that
    only looks for the command cannot see it.
    """
    for target, statements, index in gate_commands():
        statement = statements[index]
        assert not ignores_errors(statement), (
            f"the {target} recipe runs {statement!r} with Make's "
            "ignore-errors prefix, so its rejection is discarded"
        )


def test_no_gate_command_is_followed_by_an_unguarded_command() -> None:
    """Scenario: any command follows a gate command in one shell run.

    Invariant: a gate command is either the last thing that runs in its
    shell invocation or carries ``|| exit 1``. Checking only for a later
    *gate* command was too narrow: ``$(CARGO) test; echo done`` has one
    gate statement, so nothing was checked, and the successful ``echo``
    supplied the recipe's status.
    """
    for target, statements, index in gate_commands():
        statement = statements[index]
        followers = masking_followers(statements, index)
        if followers:
            assert statement.endswith(APPROVED_GUARD), (
                f"the {target} recipe runs {statement!r} followed by "
                f"{followers!r}; without {APPROVED_GUARD!r} the recipe "
                "reports the follower's status, not the gate's"
            )


def test_no_gate_command_alters_its_own_status() -> None:
    """Scenario: a gate command is joined to another with ``||`` or ``&&``.

    Invariant: a gate command is a bare invocation, apart from the
    approved ``|| exit 1`` guard. Enumerating the succeeding right-hand
    sides would hold only until someone writes the next one.
    """
    for target, statements, index in gate_commands():
        statement = statements[index]
        remainder = statement.removesuffix(APPROVED_GUARD)
        for operator in STATUS_ALTERING_OPERATORS:
            assert operator not in remainder, (
                f"the {target} recipe joins {statement!r} with {operator!r}; "
                f"a gate command is a bare invocation, apart from "
                f"{APPROVED_GUARD!r}"
            )


def test_lint_delegates_to_both_lint_routes() -> None:
    """Scenario: the composite lint target stops running one route.

    Invariant: ``lint`` runs both routes through recursive make, in order,
    with no condition or status-altering operator. A plain prerequisite
    list is unordered under ``make -j``, so the two routes would otherwise
    run concurrently and an earlier rejection would not stop the later one.
    """
    recipe = logical_lines("lint")

    assert recipe == ["$(MAKE) lint-clippy", "$(MAKE) lint-whitaker"]
    for line in recipe:
        assert not CONDITION_RE.match(line), (
            f"the lint recipe conditions {line!r}, so a route can be skipped"
        )
        for operator in STATUS_ALTERING_OPERATORS:
            assert operator not in line, (
                f"the lint recipe joins {line!r} with {operator!r}, so the "
                "first route's rejection can be discarded"
            )


@pytest.mark.parametrize("target", ["lint-clippy", "lint-whitaker"])
def test_lint_routes_are_unconditional(target: str) -> None:
    """Scenario: a lint route is guarded so it can silently skip.

    Invariant: each route's recipe is a plain command sequence with no
    shell condition and no status-altering operator, so neither route can
    be skipped while the composite target still reports success.
    """
    recipe = logical_lines(target)

    assert recipe
    for line in recipe:
        assert not CONDITION_RE.match(line), (
            f"the {target} recipe conditions {line!r}, so it can skip"
        )
        for operator in STATUS_ALTERING_OPERATORS:
            assert operator not in line, (
                f"the {target} recipe joins {line!r} with {operator!r}, so "
                "its status is no longer the gate's verdict"
            )


def test_the_whitaker_recipe_runs_the_tested_script_as_one_command() -> None:
    """Scenario: the Whitaker logic drifts back into the Makefile.

    Invariant: ``lint-whitaker`` is one shell invocation running the
    tested script, so the fail-closed resolution and status propagation
    are covered by unit tests rather than shell conditions.
    """
    recipe = logical_lines("lint-whitaker")

    assert len(recipe) == 1, (
        f"the lint-whitaker recipe should be one command, found {recipe}"
    )
    statements = [part for part in recipe[0].split(";") if part.strip()]
    assert len(statements) == 1, (
        "the lint-whitaker recipe chains commands; move the logic into "
        "scripts/run_whitaker.py per docs/scripting-standards.md"
    )
    command = statements[0].strip()
    assert "$(WHITAKER_RUNNER)" in command, (
        "the lint-whitaker recipe should invoke the tested Whitaker runner"
    )
    assert not ignores_errors(command)


def _run_make(*arguments: str) -> subprocess.CompletedProcess[str]:
    """Run Make in the repository root without inheriting a jobserver."""
    environment = {**os.environ, "MAKEFLAGS": "", "MFLAGS": ""}
    return subprocess.run(
        ["make", *arguments],
        cwd=MAKEFILE_PATH.parent,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
    )


def test_make_keeps_the_lint_routes_sequential(tmp_path) -> None:
    """Scenario: ``make -j`` is free to reorder the lint prerequisites.

    Invariant: the routes run one after the other. Parsing ``.WAIT``
    proves the arrangement; this runs ``make -j4`` with recording stubs
    and proves Make honours it, because without ``.WAIT`` the Whitaker
    stub records itself before the Clippy stub finishes.
    """
    log = tmp_path / "order.log"
    cargo = tmp_path / "cargo"
    cargo.write_text(
        f"#!/bin/sh\nprintf 'cargo %s\\n' \"$1\" >> {shlex.quote(str(log))}\n",
        encoding="utf-8",
    )
    whitaker_runner = tmp_path / "whitaker-runner"
    whitaker_runner.write_text(
        f"#!/bin/sh\nprintf 'whitaker\\n' >> {shlex.quote(str(log))}\n",
        encoding="utf-8",
    )
    cargo.chmod(0o755)
    whitaker_runner.chmod(0o755)

    result = _run_make(
        "-j4",
        "-o",
        "ensure-cargo",
        "-o",
        "check-build-tools",
        f"CARGO={cargo}",
        f"WHITAKER_RUNNER={whitaker_runner}",
        "lint",
    )

    assert result.returncode == 0, result.stderr
    assert log.read_text(encoding="utf-8").splitlines() == [
        "cargo doc",
        "cargo clippy",
        "whitaker",
    ]


def test_a_missing_whitaker_fails_the_target() -> None:
    """Scenario: ``make whitaker-lint`` runs without Whitaker installed.

    Invariant: the target exits non-zero and the runner's actionable
    message reaches the caller, so an absent suite cannot look green.
    """
    result = _run_make("whitaker-lint", "WHITAKER=/nonexistent")

    assert result.returncode != 0
    assert "Whitaker executable '/nonexistent'" in result.stderr, result.stderr


def test_a_rejecting_whitaker_fails_the_target(tmp_path) -> None:
    """Scenario: the installed suite rejects the workspace with status 7.

    Invariant: the target exits non-zero and Make reports the suite's own
    status, so the recipe neither flattens nor discards it.
    """
    stub = tmp_path / "whitaker"
    stub.write_text("#!/bin/sh\nexit 7\n", encoding="utf-8")
    stub.chmod(0o755)

    result = _run_make("whitaker-lint", f"WHITAKER={stub}")

    assert result.returncode != 0
    assert "Error 7" in result.stdout + result.stderr
