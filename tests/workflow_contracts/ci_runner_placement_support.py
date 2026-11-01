"""Shared inputs for the runner placement contract tests.

This module holds what ``ci_runner_placement_test.py`` reads before it can
assert anything: the pinned placement and ceiling tables, the reviewed guard
strings, the registry and runner-family constants, and the small reader that
parses workflow documents and derives each lane's labels. Separating the
support layer keeps both files inside the repository's size rule without
weakening a check.

The reader is deliberately derived rather than listed: it globs both workflow
extensions, parses every document, and computes the labels in use, so a lane
absent from a table is still examined by the checks that read the tree.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

import yaml

REPO_ROOT: typ.Final = Path(__file__).resolve().parents[2]
WORKFLOW_DIR: typ.Final = REPO_ROOT / ".github" / "workflows"
ACTIONLINT_CONFIG: typ.Final = REPO_ROOT / ".github" / "actionlint.yaml"


#: Both spellings GitHub accepts for a workflow file's extension. Reading one
#: of them would let a lane in the other sit outside every check here.
WORKFLOW_FILE_PATTERNS: typ.Final = ("*.yml", "*.yaml")

#: Every Ubicloud label starts with this. Used where the question is "is this
#: lane on Ubicloud at all", which must stay true as shapes are right-sized.
UBICLOUD_LABEL_PREFIX: typ.Final = "ubicloud-"
FORK_FALLBACK_LABEL: typ.Final = "ubuntu-latest"

#: The one guard the fork fallback may key on. Compared by equality, because a
#: sibling field such as ``head.repo.private`` yields an expression of exactly
#: the same shape that selects the wrong runner on every fork pull request.
FORK_GUARD: typ.Final = "github.event.pull_request.head.repo.fork"

#: The pull request's author, not ``github.actor``: the actor is whoever
#: triggered the run, so a person pushing to a Dependabot branch would flip the
#: runner. Hosted minutes cost nothing on a public repository, so Dependabot's
#: pull requests take the fork arm too.
DEPENDABOT_GUARD: typ.Final = (
    "github.event.pull_request.user.login == 'dependabot[bot]'"
)

#: The exact guard every fallback lane keys on.
HOSTED_GUARD: typ.Final = f"({FORK_GUARD} || {DEPENDABOT_GUARD})"

#: Every Ubicloud job, with the reviewed label and the reviewed ceiling it
#: must carry. Both are pinned per coordinate rather than assumed uniform:
#: `lint` was right-sized to four vCPU on measurement, and a shape that is
#: merely "some Ubicloud label" is not a reviewed decision. Ceilings are
#: pinned by value rather than bounded, because a ceiling can drift to a
#: number nobody chose while every inequality still holds. The measurements
#: behind both are in docs/developers-guide.md, section 2.2.
UBICLOUD_PLACEMENT: typ.Final = {
    ("ci.yml", "spelling"): ("ubicloud-standard-2", 10),
    ("ci.yml", "build"): ("ubicloud-standard-2", 15),
    ("ci.yml", "check-fmt"): ("ubicloud-standard-2", 15),
    ("ci.yml", "test"): ("ubicloud-standard-2", 25),
    ("ci.yml", "lint"): ("ubicloud-standard-4", 30),
    ("ci.yml", "docs-gate"): ("ubicloud-standard-2", 30),
    ("ci.yml", "systemd-gate"): ("ubicloud-standard-2", 15),
    ("ci.yml", "coverage"): ("ubicloud-standard-2", 25),
}

#: Ubicloud jobs that answer main alone and therefore carry no fork fallback.
#: A pull request cannot start their workflow, so a fallback arm here would
#: only make it possible to widen the lane to a caller nobody reviewed. Pinned
#: by value for the same reason as the table above.
MAIN_ONLY_PLACEMENT: typ.Final = {
    ("coverage-main.yml", "coverage-upload"): ("ubicloud-standard-2", 25),
}

#: Jobs this repository does not place: thin callers of reusable workflows,
#: whose runner is the callee's to choose. Listing them is what makes the
#: coordinate comparison total, so a new lane cannot appear unplaced.
DELEGATED_JOBS: typ.Final = {
    ("mutation-testing.yml", "mutation"),
    ("dependabot-automerge.yml", "automerge"),
}

#: GitHub-hosted labels this repository may use without registering them.
#: actionlint knows these already; registering one would be the stale entry
#: the registry contract exists to refuse.
GITHUB_HOSTED_LABELS: typ.Final = frozenset({FORK_FALLBACK_LABEL})

#: Prohibited runner families. A prohibition reads by substring on purpose: a
#: renamed or neutered label still leaves its family's text behind.
FOREIGN_RUNNER_FRAGMENTS: typ.Final = ("windows", "macos", "self-hosted")

#: One expression, anchored end to end. ``[^'\n]`` in the arms, and the
#: guard's single-line forms (a bare context or one parenthesized condition),
#: keep a value carrying an embedded line break from matching here as well, so
#: the line-break failure is reported by its own test.
RUNNER_EXPRESSION: typ.Final = re.compile(
    r"^\$\{\{ (?P<guard>\([^\n]*?\)|\S+)"
    r" && '(?P<fork_arm>[^'\n]*)'"
    r" \|\| '(?P<default_arm>[^'\n]*)' \}\}$"
)

#: Every quoted literal in an expression, used to read the labels a lane can
#: actually select.
EXPRESSION_LITERAL: typ.Final = re.compile(r"'([^'\n]*)'")


def _case_id(value: object) -> str:
    """Render one parametrized case identifier."""
    if isinstance(value, tuple):
        return "-".join(str(item) for item in value)
    return str(value)


def _workflow_paths() -> list[Path]:
    """Return every workflow document's path, in a stable order.

    GitHub accepts both spellings of the extension and runs a workflow
    written either way. Reading only one of them would leave a lane in
    ``.github/workflows/*.yaml`` outside every assertion below, including
    the coordinate comparison that exists to make the coverage total.
    """
    return sorted(
        path
        for pattern in WORKFLOW_FILE_PATTERNS
        for path in WORKFLOW_DIR.glob(pattern)
    )


def _workflows() -> dict[str, dict[str, object]]:
    """Parse every workflow document, keyed by file name."""
    documents = {
        path.name: yaml.safe_load(path.read_text(encoding="utf-8"))
        for path in _workflow_paths()
    }
    assert documents, "the repository should define at least one workflow"
    return documents


def _jobs() -> dict[tuple[str, str], dict[str, object]]:
    """Return every job in the repository, keyed by ``(workflow, job id)``."""
    keyed: dict[tuple[str, str], dict[str, object]] = {}
    for name, document in _workflows().items():
        for job_id, definition in (document or {}).get("jobs", {}).items():
            keyed[(name, job_id)] = definition
    return keyed


def _runner_value(definition: dict[str, object]) -> object | None:
    """Return a job's ``runs-on`` value, if it declares one."""
    return definition.get("runs-on")


def _runner_declarations(definition: dict[str, object]) -> list[object]:
    """Return a job's ``runs-on`` declarations, which may be a list of labels."""
    value = _runner_value(definition)
    if value is None:
        return []
    return value if isinstance(value, list) else [value]


def _declaration_labels(declaration: object) -> set[str]:
    """Return every label one ``runs-on`` declaration can select.

    Both arms of an expression count. A label reachable only on the fork
    branch is as much in use as one reachable on the other.
    """
    text = str(declaration)
    if "${{" in text:
        # The author test compares against `dependabot[bot]`: an operand of
        # the condition, not a runner label.
        return set(EXPRESSION_LITERAL.findall(text)) - {"dependabot[bot]"}
    return {text.strip()}


def _job_labels(definition: dict[str, object]) -> set[str]:
    """Return every label one job can select, across all its declarations."""
    return {
        label
        for declaration in _runner_declarations(definition)
        for label in _declaration_labels(declaration)
    }


def _labels_in_use() -> set[str]:
    """Return every label any lane in the repository can select."""
    return {
        label for definition in _jobs().values() for label in _job_labels(definition)
    }


def _registered_labels() -> set[str]:
    """Return the labels ``.github/actionlint.yaml`` registers."""
    assert ACTIONLINT_CONFIG.exists(), (
        "this repository uses a runner label actionlint does not know, so "
        f"{ACTIONLINT_CONFIG.relative_to(REPO_ROOT)} must exist"
    )
    config = yaml.safe_load(ACTIONLINT_CONFIG.read_text(encoding="utf-8")) or {}
    return set((config.get("self-hosted-runner") or {}).get("labels") or [])

