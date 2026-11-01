"""Hold the single push-to-main CodeScene publisher (CV-005).

One workflow answers a push to ``main`` and does both persistent things: it
refreshes the ratchet baseline and uploads the report. Its upload is guarded,
serialized and conditioned here; where the token may travel is held by
``coverage_token_rules``, and the coverage selection both lanes share is held
by ``coverage_input_rules``.

The publisher's trigger is ``push`` alone. The Dependabot automerge workflow
merges with ``GITHUB_TOKEN``, and such a merge fires no push event, so those
commits reach this workflow only at the next ordinary push. That is an
operational gap in baseline freshness, not a reason to add a trigger: a
``workflow_dispatch`` would let a branch publish where main's policy has not
reviewed it, and no run of this workflow could then be trusted to be main's.
"""

from __future__ import annotations

import re
import typing as typ

from coverage_token_rules import CHECK_ID, expression, token_violations
from coverage_workflow_reader import (
    Document,
    Step,
    calls,
    continues_on_error,
    folded,
    holding_job,
    jobs,
    scalars,
    steps,
    triggers,
)

UPLOAD_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/upload-codescene-coverage"
)
COVERAGE_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage"
)
CHECKOUT_ACTION: typ.Final[str] = "actions/checkout"

#: The upload runs only when the presence check found the secret and the ref
#: is main. The condition is compared as an exact set of conjuncts rather than
#: a superset: an extra conjunct can only narrow the upload, and ``&& false``
#: narrows it to never. An ``||`` leaves some conjunct unequal to either
#: required one, so widening with ``|| github.event_name ==
#: 'workflow_dispatch'`` is refused without a separate scan.
UPLOAD_GUARD: typ.Final[frozenset[str]] = frozenset(
    {
        f"steps.{CHECK_ID}.outputs.available == 'true'",
        "github.ref == 'refs/heads/main'",
    }
)

#: The only token scope the upload job needs.
READ_ONLY: typ.Final[dict[str, str]] = {"contents": "read"}

#: The publisher answers a push to main and nothing else.
PUBLISHER_EVENTS: typ.Final[frozenset[str]] = frozenset({"push"})

#: The publisher's concurrency group, exactly, keyed on the ref alone. One
#: group per ref means runs never overlap, so the newest trigger's upload is
#: the last to run for that ref. Adding the event would let an earlier trigger
#: finish after a newer one and publish older coverage last.
PUBLISHER_GROUP: typ.Final[str] = "${{ github.workflow }}-${{ github.ref }}"

#: Retired with CV-005 everywhere: the uploader rejects ``installer-checksum``
#: outright, and the variable and refresher workflow pinned an installer
#: script the uploader no longer runs.
RETIRED: typ.Final[tuple[str, ...]] = (
    "installer-checksum",
    "codescene_cli_sha256",
    "get-codescene-sha",
)

PINNED: typ.Final[re.Pattern[str]] = re.compile(r"[0-9a-f]{40}")


def _conjuncts(condition: object) -> frozenset[str]:
    """Split a step condition on ``&&``, normalizing whitespace."""
    text = str(condition).strip()
    if text.startswith("${{") and text.endswith("}}"):
        text = text[3:-2]
    return frozenset(" ".join(part.split()) for part in text.split("&&"))


def upload_steps(documents: dict[str, Document]) -> list[tuple[str, Step]]:
    """Return every step in any workflow that calls the CodeScene uploader.

    Parameters
    ----------
    documents
        Every workflow in the repository, keyed by file name.

    Returns
    -------
    list[tuple[str, Step]]
        Each uploading step with its workflow's file name.
    """
    return [
        (name, step)
        for name, document in documents.items()
        for step in steps(name, document)
        if calls(step, UPLOAD_ACTION)
    ]


def _publisher_triggers(name: str, document: Document) -> list[str]:
    """Report a publisher answering anything but a push to main."""
    events = triggers(name, document)
    found = (
        []
        if events.keys() == PUBLISHER_EVENTS
        else [
            f"{name} must answer exactly {sorted(PUBLISHER_EVENTS)}, "
            f"not {sorted(events)}"
        ]
    )
    if events.get("push") != {"branches": ["main"]}:
        found.append(f"{name} must answer exactly `push: branches: [main]`")
    return found


def _group_is_exact(concurrency: object) -> bool:
    """Return whether a concurrency value names exactly the publisher's group.

    Compared as a whole expression rather than searched for words: a literal
    ``coverage-main-github.ref`` names the ref and evaluates nothing, so every
    run would share one group across refs.
    """
    if not isinstance(concurrency, dict) or "group" not in concurrency:
        return False
    return expression(concurrency["group"]) == PUBLISHER_GROUP


def _publisher_concurrency(name: str, document: Document) -> list[str]:
    """Report a publisher whose uploads could overlap or be cancelled.

    The group sits at workflow scope only. GitHub treats the same group
    declared on a workflow and on one of its jobs as a deadlock and cancels
    the job, so a job-level declaration is refused rather than merged. The
    group must not cancel a run in progress: a cancelled run abandons its
    upload and its baseline write.
    """
    concurrency = document.get("concurrency")
    found: list[str] = []
    if not _group_is_exact(concurrency):
        found.append(f"{name} needs a workflow-level group of {PUBLISHER_GROUP}")
    if isinstance(concurrency, dict) and (
        concurrency.get("cancel-in-progress", False) is not False
    ):
        found.append(f"{name} cancels a publisher run in progress")
    for job_name, job in jobs(name, document).items():
        if "concurrency" in job:
            found.append(
                f"{name} job {job_name} declares concurrency; "
                "the publisher's group belongs at workflow scope only"
            )
    return found


def _publisher_jobs(name: str, document: Document) -> list[str]:
    """Report a publisher job that could be skipped or could fail green."""
    return [
        f"{name} job {job_name} {problem}"
        for job_name, job in jobs(name, document).items()
        for problem, failed in (
            ("must run unconditionally", "if" in job),
            ("must not continue on error", continues_on_error(job)),
        )
        if failed
    ]


def _upload_step(name: str, step: Step) -> list[str]:
    """Report an upload step not guarded and moded as required."""
    inputs = step.get("with")
    inputs = inputs if isinstance(inputs, dict) else {}
    found: list[str] = []
    if _conjuncts(step.get("if", "")) != UPLOAD_GUARD:
        found.append(f"{name} upload must be guarded on exactly {sorted(UPLOAD_GUARD)}")
    if inputs.get("mode") != "upload":
        found.append(f"{name} upload must name `mode: upload`")
    if continues_on_error(step):
        found.append(f"{name} upload must not continue on error")
    return found


def _keeps_no_credentials(checkout: Step) -> bool:
    """Return whether a checkout step sets ``persist-credentials: false``."""
    inputs = checkout.get("with")
    return isinstance(inputs, dict) and inputs.get("persist-credentials") is False


def _upload_job(name: str, document: Document, upload: Step) -> list[str]:
    """Report an upload job with a wider token or persisted Git credentials.

    Nothing in the job writes to the repository, and the coverage run executes
    repository and dependency code after checkout, so the token is read-only
    and the checkout keeps no credentials.
    """
    job = holding_job(name, document, upload)
    found = (
        []
        if job.get("permissions") == READ_ONLY
        else [f"{name} upload job permissions must be exactly {READ_ONLY}"]
    )
    return found + [
        f"{name} checkout must set persist-credentials: false"
        for step in typ.cast("list[Step]", job.get("steps", []))
        if calls(step, CHECKOUT_ACTION) and not _keeps_no_credentials(step)
    ]


def _ref(step: Step) -> str:
    """Return the ref a step's ``uses:`` names."""
    return str(step.get("uses", "")).partition("@")[2]


def _pins(name: str, document: Document, upload: Step) -> list[str]:
    """Report an uploader or generator that is not pinned to one full SHA.

    The generator and the uploader must share a pin, so the report uploaded is
    produced by code at the same revision the uploader was reviewed against.
    """
    coverage = [
        step
        for step in steps(name, document)
        if calls(step, COVERAGE_ACTION)
    ]
    found = [
        f"{name} must pin shared actions by full commit SHA"
        for step in (upload, *coverage)
        if PINNED.fullmatch(_ref(step)) is None
    ]
    found += [
        f"{name} upload pin differs from its coverage pin"
        for step in coverage
        if _ref(step) != _ref(upload)
    ]
    return found


def publisher_violations(documents: dict[str, Document]) -> list[str]:
    """Report anything but one guarded push-to-main publisher.

    Parameters
    ----------
    documents
        Every workflow in the repository, keyed by file name.

    Returns
    -------
    list[str]
        One message per violation; empty when the repository complies.
    """
    uploads = upload_steps(documents)
    if len(uploads) != 1:
        return [f"expected one CodeScene upload step, found {len(uploads)}"]
    name, upload = uploads[0]
    document = documents[name]
    return [
        *_publisher_triggers(name, document),
        *_publisher_concurrency(name, document),
        *_publisher_jobs(name, document),
        *_upload_step(name, upload),
        *_upload_job(name, document, upload),
        *_pins(name, document, upload),
        *token_violations(name, document, upload),
    ]


def retired_names(documents: dict[str, Document]) -> list[str]:
    """Report any retired checksum input, variable or refresher workflow.

    Parameters
    ----------
    documents
        Every workflow in the repository, keyed by file name.

    Returns
    -------
    list[str]
        One message per workflow and retired name; empty when none remains.
    """
    return sorted(
        {
            f"{name} still names {retired}"
            for name, document in documents.items()
            for retired in RETIRED
            if retired in name.casefold()
            or any(retired in folded(text) for text in scalars(document))
        }
    )
