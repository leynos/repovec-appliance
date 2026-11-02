"""Strict readings of workflow documents for the CV-005 coverage contract.

The strict parser lives in ``workflow_loader``, which every workflow contract
in this directory shares: it refuses a mapping that declares a key twice and a
document whose root is not a mapping. This module adds the readings the
coverage rules need - triggers, jobs, steps, action calls and scalar leaves -
and the refusals that come from an empty or ambiguous tree.

A reading that finds nothing is a fault of the reader, not a pass: every rule
built on these readings is a refusal, and a refusal over an empty subject set
is satisfied by any repository at all. Those faults raise ``WorkflowError``.
"""

from __future__ import annotations

import re
import typing as typ
from pathlib import Path

import yaml

from workflow_loader import (
    WORKFLOW_DIR,
    Document,
    read_workflows,
)

if typ.TYPE_CHECKING:
    import collections.abc as cabc

type Step = dict[str, object]


class WorkflowError(ValueError):
    """Raised when the workflows cannot be read as the rules require."""


def read_documents(directory: Path = WORKFLOW_DIR) -> dict[str, Document]:
    """Parse every workflow in a directory through the shared strict loader.

    Parameters
    ----------
    directory
        The directory holding the workflow files, usually
        ``.github/workflows``.

    Returns
    -------
    dict[str, Document]
        Each workflow's parsed document, keyed by file name.

    Raises
    ------
    WorkflowError
        If the directory holds no workflow, or a workflow cannot be listed, is
        not UTF-8 text, declares a mapping key twice, or has a non-mapping
        root. An empty reading would satisfy every refusal below, so it is
        refused here.

    Examples
    --------
    >>> read_documents(Path("/nonexistent"))
    Traceback (most recent call last):
    ...
    coverage_workflow_reader.WorkflowError: ...
    """
    try:
        documents = read_workflows(directory)
    except (OSError, UnicodeDecodeError, yaml.YAMLError) as error:
        raise WorkflowError(_listing_failure(directory, error)) from error
    if not documents:
        message = f"no workflows were read from {directory}"
        raise WorkflowError(message)
    return documents


def _listing_failure(directory: Path, error: Exception) -> str:
    """Return the message describing why the workflow directory was refused."""
    match error:
        case UnicodeDecodeError():
            return f"{directory}: a workflow cannot be read as UTF-8: {error}"
        case yaml.YAMLError():
            return f"{directory}: a workflow cannot be read strictly: {error}"
        case _:
            return f"cannot list workflows in {directory}: {error}"


def triggers(name: str, document: Document) -> dict[str, object]:
    """Return a workflow's events in mapping form.

    YAML 1.1 reads a bare ``on`` as boolean true, and a quoted ``'on'`` as the
    string. GitHub merges the two, so a workflow declaring both is refused
    rather than read by half.

    Parameters
    ----------
    name
        The workflow's file name, for messages.
    document
        The parsed workflow.

    Returns
    -------
    dict[str, object]
        Each event name mapped to its filter, or None where the scalar or
        sequence form gives none.

    Raises
    ------
    WorkflowError
        If ``on`` is declared under neither or both spellings, or has a shape
        other than a scalar, a sequence of names or a mapping.

    Examples
    --------
    >>> triggers("ci.yml", {True: "push"})
    {'push': None}
    """
    spellings = [key for key in ("on", True) if key in document]
    if len(spellings) != 1:
        message = f"{name}: declares `on` {len(spellings)} times, not once"
        raise WorkflowError(message)
    match document[spellings[0]]:
        case str() as event:
            return {event: None}
        case list() as events if all(isinstance(event, str) for event in events):
            return dict.fromkeys(typ.cast("list[str]", events))
        case dict() as events:
            return {str(event): value for event, value in events.items()}
        case other:
            message = f"{name}: cannot read the trigger {other!r}"
            raise WorkflowError(message)


def jobs(name: str, document: Document) -> dict[str, dict[str, object]]:
    """Return a workflow's jobs, refusing a malformed ``jobs`` block.

    Parameters
    ----------
    name
        The workflow's file name, for messages.
    document
        The parsed workflow.

    Returns
    -------
    dict[str, dict[str, object]]
        Each job's mapping, keyed by job id.

    Raises
    ------
    WorkflowError
        If ``jobs`` is missing or does not map job ids to mappings.
    """
    found = document.get("jobs")
    if not isinstance(found, dict) or not all(
        isinstance(job, dict) for job in found.values()
    ):
        message = f"{name}: `jobs` must map job names to mappings"
        raise WorkflowError(message)
    return typ.cast("dict[str, dict[str, object]]", found)


def steps(name: str, document: Document) -> cabc.Iterator[Step]:
    """Yield every step of every job in one workflow.

    Parameters
    ----------
    name
        The workflow's file name, for messages.
    document
        The parsed workflow.

    Yields
    ------
    Step
        Each step mapping, in job and step order.

    Raises
    ------
    WorkflowError
        If the jobs are malformed or a step is not a mapping.
    """
    for job in jobs(name, document).values():
        for step in typ.cast("list[object]", job.get("steps", [])):
            if not isinstance(step, dict):
                message = f"{name}: a step must be a mapping"
                raise WorkflowError(message)
            yield typ.cast("Step", step)


def calls(step: Step, action: str) -> bool:
    """Return whether a step calls one shared action, at any ref.

    Parameters
    ----------
    step
        The step mapping.
    action
        The action path, without a ref.

    Returns
    -------
    bool
        True when the step's ``uses:`` names the action, compared without case.

    Examples
    --------
    >>> calls({"uses": "actions/checkout@abc"}, "actions/checkout")
    True
    """
    uses = str(step.get("uses", ""))
    return uses.partition("@")[0].casefold() == action.casefold()


def scalars(value: object) -> cabc.Iterator[str]:
    """Yield every key and value in a parsed document as text.

    Keys are yielded as well as values: an ``env`` key or a ``workflow_call``
    secret declaration names the token with no value that refers to it.

    Parameters
    ----------
    value
        A parsed document or any part of one.

    Yields
    ------
    str
        Each key and each non-null leaf value, as text.
    """
    match value:
        case dict():
            for key, child in value.items():
                yield str(key)
                yield from scalars(child)
        case list():
            for child in value:
                yield from scalars(child)
        case None:
            return
        case _:
            yield str(value)


def folded(text: str) -> str:
    """Return text case-folded with all whitespace removed.

    Parameters
    ----------
    text
        The text to normalize.

    Returns
    -------
    str
        The text, so that ``toJSON( secrets )`` and ``API.CODESCENE.IO`` match
        their plain spellings.

    Examples
    --------
    >>> folded("${{ secrets.CS_ACCESS_TOKEN }}")
    '${{secrets.cs_access_token}}'
    """
    return re.sub(r"\s+", "", text).casefold()


def continues_on_error(mapping: dict[str, object]) -> bool:
    """Return whether a step or job may fail without failing its run.

    ``continue-on-error`` keeps the step running while its failure turns green,
    which silences a ratchet or an upload as surely as ``if: false`` does.

    Parameters
    ----------
    mapping
        A step or job mapping from a parsed workflow.

    Returns
    -------
    bool
        True when ``continue-on-error`` is set to any value other than false.
    """
    return mapping.get("continue-on-error", False) is not False


def holding_job(name: str, document: Document, step: Step) -> dict[str, object]:
    """Return the job in one workflow whose steps include this step.

    Parameters
    ----------
    name
        The workflow's file name, for messages.
    document
        The parsed workflow.
    step
        The step mapping to find among the workflow's jobs.

    Returns
    -------
    dict[str, object]
        The job mapping whose ``steps`` list holds ``step``.

    Raises
    ------
    StopIteration
        If the step is not part of any job in the document, which means the
        caller holds a step from another reading.
    """
    return next(
        job
        for job in jobs(name, document).values()
        if any(
            held is step for held in typ.cast("list[object]", job.get("steps", []))
        )
    )