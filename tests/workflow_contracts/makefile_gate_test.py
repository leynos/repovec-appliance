"""Contract tests for the Make gate recipes CI depends on.

A Make recipe is one shell invocation, run without ``set -e``, so its
exit status is that of its *last* command. A recipe that chains several
commands with ``;`` therefore reports only the last one, and a rejection
from any earlier command is discarded. The ``test`` recipe chains the
unit-test run and a conditional doctest run exactly this way.

That was not hypothetical here. Measured on 2026-09-07, with a
deliberately failing unit test in the workspace, ``make test`` exited 0:
nextest reported ``1 failed`` and the doctest step that ran afterwards
supplied the recipe's zero status. CI's ``test`` job would have passed
with failing tests. Guarding each command with ``|| exit 1`` makes the
recipe exit at the first rejection; the same probe then exited 2.

These tests pin that guard so the hole cannot reopen. They parse the
recipe rather than searching the whole file, so a guarded command in
some unrelated recipe cannot satisfy them.

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

#: Recipes that chain more than one gate command inside a single shell
#: invocation, and so need the guard. ``lint`` is deliberately absent:
#: its commands are separate recipe lines, and Make checks each line's
#: status itself. A recipe belongs here once it joins gate commands with
#: ``;`` or a trailing backslash.
GUARDED_RECIPES = ("test",)

#: Commands that carry a gate's verdict. A recipe may chain shell
#: plumbing freely; it is the tool invocations that must not be dropped.
GATE_COMMAND_RE = re.compile(r"\$\(CARGO\)\s")

#: A statement that opens an ``if``. Cargo appearing in a condition is
#: being asked a question, and a non-zero answer there is meaningful
#: rather than a rejection, so those invocations are not gate commands.
CONDITION_RE = re.compile(r"^@?(if|elif)\s")


def _recipe(target: str) -> list[str]:
    """Return the logical command lines of one Make recipe.

    Recipe lines are tab-indented and may be continued with a trailing
    backslash. Continuations are joined so that a command split across
    source lines is examined as the single shell command it becomes.
    """
    lines = MAKEFILE_PATH.read_text(encoding="utf-8").splitlines()
    start = next(
        index
        for index, line in enumerate(lines)
        if re.match(rf"^{re.escape(target)}:", line)
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


def _gate_commands(recipe: list[str]) -> list[str]:
    """Return the statements in a recipe that invoke a gate tool.

    Statements are separated by ``;``; the split does not honour shell
    quoting, which is sufficient for these recipes and would fail loudly
    rather than silently if one grew a quoted semicolon. Invocations in
    an ``if`` or ``elif`` condition are excluded, because their status is
    an answer rather than a verdict.
    """
    return [
        stripped
        for line in recipe
        for statement in line.split(";")
        if GATE_COMMAND_RE.search(statement)
        and not CONDITION_RE.match(stripped := statement.strip())
    ]


def test_the_makefile_declares_the_guarded_recipes() -> None:
    """Scenario: a recipe is renamed or removed.

    Invariant: each guarded recipe still exists and runs a gate command,
    so the assertions below cannot pass vacuously.
    """
    for target in GUARDED_RECIPES:
        commands = _gate_commands(_recipe(target))
        assert commands, f"the {target} recipe should invoke a gate tool"


def test_every_gate_command_exits_on_rejection() -> None:
    """Scenario: a gate tool rejects part-way through a chained recipe.

    Invariant: every gate command is guarded with ``|| exit 1``, so the
    recipe fails at the rejection instead of reporting the status of
    whatever command happens to run last.
    """
    for target in GUARDED_RECIPES:
        for command in _gate_commands(_recipe(target)):
            assert command.endswith("|| exit 1"), (
                f"the {target} recipe runs {command!r} unguarded; a "
                "rejection here would be discarded by the command that "
                "follows it"
            )
