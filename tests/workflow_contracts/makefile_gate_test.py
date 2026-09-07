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

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

MAKEFILE_PATH = Path(__file__).resolve().parents[2] / "Makefile"

pytestmark = pytest.mark.skipif(
    not MAKEFILE_PATH.exists(),
    reason="Makefile not present in this working copy (e.g. inside "
    "mutmut's mutants/ sandbox)",
)

#: Recipes whose gate logic must stay out of the Makefile entirely.
SINGLE_COMMAND_RECIPES = ("test",)

#: Make variables that expand to a tool whose rejection is a gate
#: verdict. Extend this when a recipe starts running a new one; a tool
#: missing from here is simply not checked.
GATE_TOOLS = (
    "$(CARGO)",
    "$(WHITAKER)",
    "$(MDLINT)",
    "$(NIXIE)",
    "$(RUST_TEST_RUNNER)",
    "$(SCRIPT_PYTEST)",
    "$(SPELLING_HELPER_PYTEST)",
    "$(TYPOS_CONFIG_BUILDER)",
)

#: A statement that opens a condition. A tool named in an ``if`` is being
#: asked a question, and a non-zero answer there is meaningful rather
#: than a rejection, so those invocations are not gate commands.
CONDITION_RE = re.compile(r"^@?(if|elif|while|until)\s")

#: A target definition line, which ends the preceding recipe.
TARGET_RE = re.compile(r"^([A-Za-z_][A-Za-z0-9_.-]*)\s*:(?!=)")

#: Make's line prefixes. ``-`` makes Make ignore the command's status
#: entirely, which discards a gate verdict as surely as deleting the
#: command, while leaving it in place and readable.
RECIPE_PREFIXES = "@+-"

#: Shell operators that make a command's status something other than its
#: own. Enumerating the succeeding right-hand sides (``|| true``, ``|| :``,
#: ``|| exit 0``) is the wrong shape: the next one written will not be on
#: the list. A gate command is a bare invocation, so any disjunction or
#: conjunction is rejected and ``|| exit 1`` is the sole exception, since
#: it strengthens the status rather than discarding it.
STATUS_ALTERING_OPERATORS = ("||", "&&")


def _logical_lines(target: str) -> list[str]:
    """Return one Make recipe's lines, with continuations joined.

    Recipe lines are tab-indented and may be continued with a trailing
    backslash. Each joined line is one shell invocation, which is the
    unit Make takes a status from.
    """
    lines = MAKEFILE_PATH.read_text(encoding="utf-8").splitlines()
    start = next(
        index
        for index, line in enumerate(lines)
        if TARGET_RE.match(line) and TARGET_RE.match(line).group(1) == target
    )

    recipe: list[str] = []
    pending = ""
    for line in lines[start + 1 :]:
        if not line.startswith("\t"):
            break
        body = line.lstrip("\t").rstrip()
        if body.endswith("\\"):
            pending += body[:-1]
            continue
        recipe.append((pending + body).strip())
        pending = ""
    if pending:
        recipe.append(pending.strip())
    return recipe


def _targets() -> list[str]:
    """Return every target the Makefile defines, in file order."""
    seen: list[str] = []
    for line in MAKEFILE_PATH.read_text(encoding="utf-8").splitlines():
        match = TARGET_RE.match(line)
        if match and match.group(1) not in seen:
            seen.append(match.group(1))
    return seen


def _strip_prefixes(statement: str) -> str:
    """Return a recipe statement without Make's line prefixes."""
    return statement.lstrip(RECIPE_PREFIXES).lstrip()


def _ignores_errors(statement: str) -> bool:
    """Report whether Make is told to ignore this command's status."""
    return statement.lstrip("@+").startswith("-")


def _gate_statements(logical_line: str) -> list[str]:
    """Return the gate invocations in one shell invocation, in order.

    Statements are separated by ``;``; the split does not honour shell
    quoting, which is sufficient for these recipes. Conditions are
    excluded, because their status is an answer rather than a verdict.
    """
    return [
        statement
        for raw in logical_line.split(";")
        if (statement := raw.strip())
        and any(tool in statement for tool in GATE_TOOLS)
        and not CONDITION_RE.match(statement)
    ]


@pytest.mark.parametrize("target", SINGLE_COMMAND_RECIPES)
def test_the_recipe_runs_exactly_one_command(target: str) -> None:
    """Scenario: gate sequencing drifts back into the Makefile.

    Invariant: the recipe is one shell invocation running one command,
    so there is no second command whose status could mask the first.
    """
    recipe = _logical_lines(target)

    assert len(recipe) == 1, (
        f"the {target} recipe should be one command, found {len(recipe)}: {recipe}"
    )
    statements = [part for part in recipe[0].split(";") if part.strip()]
    assert len(statements) == 1, (
        f"the {target} recipe chains {len(statements)} commands; move the "
        "sequencing into a tested script per docs/scripting-standards.md"
    )

    command = statements[0].strip()
    assert not _ignores_errors(command), (
        f"the {target} recipe is prefixed with '-', so Make ignores its "
        "status and the gate can never fail the build"
    )
    for operator in STATUS_ALTERING_OPERATORS:
        assert operator not in _strip_prefixes(command), (
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
    assert "$(RUST_TEST_RUNNER)" in _logical_lines("test")[0], (
        "the test recipe should invoke the Rust test-gate runner"
    )


def test_no_gate_command_has_its_status_ignored() -> None:
    """Scenario: a gate command is prefixed with Make's ``-``.

    Invariant: no recipe tells Make to ignore a gate tool's status. The
    prefix leaves the command in place and readable, so a contract that
    only looks for the command cannot see it.
    """
    for target in _targets():
        for logical_line in _logical_lines(target):
            for statement in _gate_statements(logical_line):
                assert not _ignores_errors(statement), (
                    f"the {target} recipe runs {statement!r} with Make's "
                    "ignore-errors prefix, so its rejection is discarded"
                )


def test_every_chained_recipe_guards_its_gate_commands() -> None:
    """Scenario: a recipe chains two gate commands in one shell run.

    Invariant: every gate command but the last carries ``|| exit 1``, so
    a rejection ends the recipe instead of being replaced by the status
    of whatever runs next.
    """
    for target in _targets():
        for logical_line in _logical_lines(target):
            statements = _gate_statements(logical_line)
            for statement in statements[:-1]:
                assert statement.endswith("|| exit 1"), (
                    f"the {target} recipe runs {statement!r} before another "
                    "gate command without a guard; a rejection here would be "
                    "discarded"
                )
