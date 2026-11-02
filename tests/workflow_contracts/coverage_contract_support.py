"""Shared inputs for the CV-005 coverage contract tests.

The contract tests mutate a private copy of this repository's workflows in the
way a later edit could, and assert that the clause meant to catch that edit
does. These helpers hand each test its own copy and locate the parts it
mutates. A shared copy would let one test's mutation leak into the next, so
each test reads the tree afresh.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

from coverage_input_rules import EXPECTED_SELECTION
from coverage_publisher_rules import upload_steps
from coverage_workflow_reader import Document, Step, read_documents

if typ.TYPE_CHECKING:
    import collections.abc as cabc

type Documents = dict[str, Document]
type Rule = cabc.Callable[[Documents], list[str]]

WORKFLOWS: typ.Final[Path] = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows"
)

#: The pull-request lane, which the mutation cases extend.
LANE: typ.Final[str] = "ci.yml"
#: The single publisher.
PUBLISHER: typ.Final[str] = "coverage-main.yml"
#: A probe workflow the closure tests add and remove.
PROBE: typ.Final[str] = "probe.yml"

CREDENTIAL_REFERENCE: typ.Final[str] = "${{ secrets.CS_ACCESS_TOKEN }}"

__all__ = [
    "CREDENTIAL_REFERENCE",
    "EXPECTED_SELECTION",
    "LANE",
    "PROBE",
    "PUBLISHER",
    "WORKFLOWS",
    "Documents",
    "Rule",
    "assert_clean",
    "assert_reports",
    "coverage_job",
    "coverage_step",
    "find_publisher",
    "first_job",
    "fresh_documents",
    "job_steps",
    "lane_jobs",
    "replace_triggers",
]


def fresh_documents(directory: Path = WORKFLOWS) -> Documents:
    """Read a private copy of the workflows for one test to mutate.

    Parameters
    ----------
    directory
        The workflow directory to read.

    Returns
    -------
    Documents
        Every workflow's parsed document, keyed by file name.

    Raises
    ------
    WorkflowError
        If the directory cannot be read as workflows; see
        `coverage_workflow_reader.read_documents`.
    """
    return read_documents(directory)


def assert_clean(rule: Rule, documents: Documents) -> None:
    """Fail unless a rule reports nothing."""
    found = rule(documents)
    if found:
        message = f"expected no violations, got {found}"
        raise AssertionError(message)


def assert_reports(rule: Rule, documents: Documents, fragment: str) -> None:
    """Fail unless a rule reports a violation containing a fragment."""
    found = rule(documents)
    if not any(fragment in problem for problem in found):
        message = f"expected a violation naming {fragment!r}, got {found}"
        raise AssertionError(message)


def find_publisher(documents: Documents) -> tuple[Document, Step]:
    """Return the publisher document and its upload step.

    Raises
    ------
    ValueError
        If the repository does not hold exactly one upload step.
    """
    [(name, step)] = upload_steps(documents)
    return documents[name], step


def first_job(document: Document) -> dict[str, object]:
    """Return a workflow's first job."""
    jobs = typ.cast("dict[str, dict[str, object]]", document["jobs"])
    return next(iter(jobs.values()))


def _job_holding(document: Document, *markers: str) -> dict[str, object] | None:
    """Return the job whose steps call an action whose name holds a marker."""
    for job in typ.cast(
        "dict[str, dict[str, object]]", document["jobs"]
    ).values():
        for step in typ.cast("list[Step]", job.get("steps", [])):
            if any(marker in str(step.get("uses", "")) for marker in markers):
                return job
    return None


def coverage_job(document: Document) -> dict[str, object]:
    """Return the job holding a generate-coverage step.

    Raises
    ------
    LookupError
        If no job in the workflow calls the coverage action.
    """
    job = _job_holding(document, "generate-coverage")
    if job is None:
        message = "no job calls the coverage action"
        raise LookupError(message)
    return job


def job_steps(document: Document) -> list[Step]:
    """Return the steps of the job the coverage tests mutate.

    That is the job calling the uploader or the coverage action, so a test
    names the lane's job rather than whichever job happens to come first.
    """
    job = _job_holding(document, "upload-codescene-coverage", "generate-coverage")
    return typ.cast("list[Step]", (job or first_job(document)).get("steps", []))


def coverage_step(document: Document) -> Step:
    """Return a workflow's generate-coverage step.

    Raises
    ------
    StopIteration
        If no job in the workflow calls the coverage action.
    """
    return next(
        step
        for step in job_steps(document)
        if "generate-coverage" in str(step.get("uses"))
    )


def replace_triggers(document: Document, events: object) -> None:
    """Replace a workflow's triggers under whichever ``on`` spelling it used.

    Setting one spelling beside the other would make the reader refuse the
    document for declaring ``on`` twice.
    """
    for spelling in ("on", True):
        document.pop(spelling, None)
    document[True] = events


def lane_jobs(documents: Documents) -> dict[str, object]:
    """Return the pull-request lane's jobs, so a caller job can be added."""
    return typ.cast("dict[str, object]", documents[LANE]["jobs"])
