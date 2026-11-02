"""Parsing helpers for the Make gate contract tests.

The contracts read the Makefile as text because they assert on the shape
of recipes and prerequisites rather than on the result of running them.
Keeping that parsing here leaves the test module to state the invariants.
"""

from __future__ import annotations

import re
from collections.abc import Iterator
from pathlib import Path

MAKEFILE_PATH = Path(__file__).resolve().parents[2] / "Makefile"

#: Make variables that expand to a tool whose rejection is a gate
#: verdict. Extend this when a recipe starts running a new one; a tool
#: missing from here is simply not checked.
GATE_TOOLS = (
    "$(CARGO)",
    "$(WHITAKER)",
    "$(WHITAKER_RUNNER)",
    "$(MDLINT)",
    "$(MDTABLEFIX)",
    "$(NIXIE)",
    "$(RUST_TEST_RUNNER)",
    "$(SCRIPT_PYTEST)",
    "$(TYPOS_CONFIG_BUILDER)",
)

#: A statement that opens a condition. A tool named in an ``if`` is being
#: asked a question, and a non-zero answer there is meaningful rather
#: than a rejection, so those invocations are not gate commands.
CONDITION_RE = re.compile(r"^@?(if|elif|while|until)\s")

#: Statements that close a block. They carry the status of whatever the
#: taken branch last ran, so a gate command followed only by these is
#: still the statement the recipe reports.
BLOCK_CLOSERS_RE = re.compile(r"^(fi|done|esac|;;)\b")

#: Statements that open an alternative branch. Everything from here to
#: the matching closer runs only when the gate's own branch did not, so
#: it cannot mask the gate's status.
ALTERNATIVE_RE = re.compile(r"^(else|elif)\b")

#: The one disjunction a gate command may carry: it turns a rejection
#: into an immediate exit rather than discarding it.
APPROVED_GUARD = "|| exit 1"

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


def logical_lines(target: str) -> list[str]:
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


def targets() -> list[str]:
    """Return every target the Makefile defines, in file order."""
    seen: list[str] = []
    for line in MAKEFILE_PATH.read_text(encoding="utf-8").splitlines():
        match = TARGET_RE.match(line)
        if match and match.group(1) not in seen:
            seen.append(match.group(1))
    return seen


def strip_prefixes(statement: str) -> str:
    """Return a recipe statement without Make's line prefixes."""
    return statement.lstrip(RECIPE_PREFIXES).lstrip()


def ignores_errors(statement: str) -> bool:
    """Report whether Make is told to ignore this command's status."""
    return statement.lstrip("@+").startswith("-")


def statements(logical_line: str) -> list[str]:
    """Return every non-empty statement in one shell invocation.

    Statements are separated by ``;``; the split does not honour shell
    quoting, which is sufficient for these recipes.
    """
    return [part.strip() for part in logical_line.split(";") if part.strip()]


def is_gate_command(statement: str) -> bool:
    """Report whether a statement runs a gate tool as a command.

    A tool named in a condition is being asked a question, and a non-zero
    answer there is meaningful rather than a rejection, so it is not a
    gate command.
    """
    if CONDITION_RE.match(statement):
        return False
    return any(tool in statement for tool in GATE_TOOLS)


def gate_statements(logical_line: str) -> list[str]:
    """Return the gate invocations in one shell invocation, in order."""
    return [
        statement
        for statement in statements(logical_line)
        if is_gate_command(statement)
    ]


def gate_commands_in(
    target: str, logical_line: str
) -> Iterator[tuple[str, list[str], int]]:
    """Yield the gate commands in one shell invocation."""
    parts = statements(logical_line)
    for index, statement in enumerate(parts):
        if is_gate_command(statement):
            yield target, parts, index


def gate_commands() -> Iterator[tuple[str, list[str], int]]:
    """Yield every gate command in the Makefile with its position.

    Each item is the recipe's target, the statements of the one shell
    invocation the command sits in, and its index among them. Tests then
    hold a single loop rather than three, which keeps the rule they
    assert legible.
    """
    for target in targets():
        for logical_line in logical_lines(target):
            yield from gate_commands_in(target, logical_line)


def masking_followers(parts: list[str], index: int) -> list[str]:
    """Return statements after *index* that could supply the exit status.

    A block closer carries the taken branch's status rather than setting
    its own. An alternative branch runs only when the gate's branch did
    not, so everything from ``else`` to its closer is skipped. Whatever
    remains is a command that runs after the gate and replaces its
    status.
    """
    masking: list[str] = []
    skipping = False
    for statement in parts[index + 1 :]:
        if BLOCK_CLOSERS_RE.match(statement):
            skipping = False
            continue
        if ALTERNATIVE_RE.match(statement):
            skipping = True
            continue
        if skipping:
            continue
        masking.append(statement)
    return masking
