"""Hold the coverage lanes to one measurement and one baseline writer (CV-005).

The publisher's generator refreshes the ratchet baseline on a push to main,
and every pull-request generator compares against that baseline. So each
pull-request generator must ratchet, publish no artefact and select exactly
what the publisher selects, at the same pin. If both lanes drifted together,
the ratchet would compare one measurement against another; the selection is
therefore pinned by value here as well as compared between the lanes.
"""

from __future__ import annotations

import typing as typ

from coverage_publisher_rules import READ_ONLY, upload_steps
from coverage_pull_request_rules import (
    closure,
    pull_request_closure,
    serves_pull_requests,
)
from coverage_workflow_reader import (
    Document,
    Step,
    calls,
    continues_on_error,
    holding_job,
    steps,
    triggers,
)

COVERAGE_ACTION: typ.Final[str] = (
    "leynos/shared-actions/.github/actions/generate-coverage"
)
ARTEFACT_ACTION: typ.Final[str] = "actions/upload-artifact"

#: The one condition a pull-request coverage job may carry. ``ci.yml`` also
#: answers a push to main, where the publisher owns the measurement, so the
#: lane must be confined to pull requests. A missing guard, another event, or
#: ``false`` are all refusals; no other condition is accepted.
PULL_REQUEST_GUARD: typ.Final[str] = "github.event_name == 'pull_request'"

#: The publisher's measurement, pinned by value so that both lanes changing
#: their selection together cannot satisfy the parity rule unseen.
EXPECTED_SELECTION: typ.Final[dict[str, object]] = {
    "output-path": "lcov.info",
    "format": "lcov",
    "with-ratchet": "true",
    "baseline-rust-file": ".coverage-baseline-v2.rust",
    "publish-artefact": "false",
    "language": "rust",
    "all-targets": "true",
    "all-features": "true",
}


def coverage_steps(name: str, document: Document) -> list[Step]:
    """Return one workflow's generate-coverage steps, in order.

    Parameters
    ----------
    name
        The workflow's file name, for messages.
    document
        The parsed workflow.

    Returns
    -------
    list[Step]
        Every step calling the shared coverage action.
    """
    return [step for step in steps(name, document) if calls(step, COVERAGE_ACTION)]


def _with(step: Step, key: str) -> object:
    """Return one input of a step, or None."""
    inputs = step.get("with")
    return inputs.get(key) if isinstance(inputs, dict) else None


def _selection(step: Step) -> dict[str, object]:
    """Return a coverage step's inputs as a plain mapping."""
    inputs = step.get("with")
    return dict(inputs) if isinstance(inputs, dict) else {}


def _condition(scope: dict[str, object]) -> str:
    """Return a step's or job's ``if`` value with whitespace normalized."""
    return " ".join(str(scope.get("if", "")).split())


def _pull_request_lane(
    name: str, document: Document, step: Step, trunk: Step
) -> list[str]:
    """Report a pull-request coverage step that cannot ratchet like main.

    The job runs for pull requests exactly and for nothing else, because the
    workflow also answers a push to main, where the publisher owns the
    measurement. The step carries no condition of its own, so it cannot be
    switched off inside a job that ran. The lane's whole input mapping is
    compared with the publisher's, because every input selects part of what is
    measured.
    """
    job = holding_job(name, document, step)
    conditions = (
        ("must not continue on error", continues_on_error(step)),
        ("step must not carry a condition", "if" in step),
        (
            f"job must run exactly for pull requests "
            f"(`if: {PULL_REQUEST_GUARD}`)",
            _condition(job) != PULL_REQUEST_GUARD,
        ),
        ("job must not continue on error", continues_on_error(job)),
        ("must set with-ratchet 'true'", _with(step, "with-ratchet") != "true"),
        (
            "must set publish-artefact 'false'",
            _with(step, "publish-artefact") != "false",
        ),
        (
            "selection differs from the publisher's",
            _selection(step) != _selection(trunk),
        ),
        ("pin differs from the publisher's", step.get("uses") != trunk.get("uses")),
        (
            f"job permissions must be exactly {READ_ONLY}",
            job.get("permissions") != READ_ONLY,
        ),
    )
    return [
        f"{name} coverage {problem}" for problem, failed in conditions if failed
    ]


def _trunk_violations(publisher: str, trunk: Step, upload: Step) -> list[str]:
    """Report a baseline writer that could skip, fail green or drift its inputs."""
    conditions = (
        ("coverage must run unconditionally", "if" in trunk),
        ("coverage must not continue on error", continues_on_error(trunk)),
        (
            "coverage must set with-ratchet 'true'",
            _with(trunk, "with-ratchet") != "true",
        ),
        (
            "coverage selection must be the pinned one",
            _selection(trunk) != EXPECTED_SELECTION,
        ),
        (
            "upload must read the coverage step's output-path",
            _with(upload, "path") != _with(trunk, "output-path"),
        ),
        (
            "upload must name the coverage step's format",
            _with(upload, "format") != _with(trunk, "format"),
        ),
    )
    return [
        f"{publisher} {problem}" for problem, failed in conditions if failed
    ]


def _artefact_uploads(documents: dict[str, Document], report: object) -> list[str]:
    """Report a pull-request step that publishes the coverage report.

    ``publish-artefact: 'false'`` keeps the shared action from uploading it; a
    separate ``upload-artifact`` step would publish it anyway.
    """
    return [
        f"{name} must not upload the coverage report as an artefact"
        for name, document in pull_request_closure(documents).items()
        for step in steps(name, document)
        if calls(step, ARTEFACT_ACTION) and str(report) in str(_with(step, "path"))
    ]


def _push_writers(documents: dict[str, Document], publisher: str) -> list[str]:
    """Report coverage a push-only workflow can run outside the publisher.

    A workflow that also serves pull requests measures for its own ratchet as
    well, so it is held by the lane rules rather than here. This rule reads
    the push-only workflows, following local calls, so a second baseline
    writer cannot appear beside the publisher.
    """
    seeds = {
        name
        for name, document in documents.items()
        if name != publisher
        and "push" in triggers(name, document)
        and not serves_pull_requests(name, document)
    }
    return [
        f"{name} coverage can run on a push outside the publisher"
        for name, document in closure(seeds, documents).items()
        if name != publisher
        for step in coverage_steps(name, document)
    ]


def _baseline_writers(documents: dict[str, Document]) -> list[str]:
    """Report a coverage step that may write the baseline off main's push.

    The default, ``auto``, saves the baseline only on a push to
    ``refs/heads/main``. ``always`` hands that restriction to the calling
    workflow, so on a pull-request lane each push could lower the baseline its
    next push ratchets against.
    """
    return [
        f"{name} coverage must leave publish-baseline at `auto`"
        for name, document in documents.items()
        for step in coverage_steps(name, document)
        if _with(step, "publish-baseline") not in {None, "auto"}
    ]


def coverage_violations(documents: dict[str, Document]) -> list[str]:
    """Report coverage lanes that no longer ratchet against main's baseline.

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
        return ["coverage lanes need exactly one publisher to compare against"]
    publisher = uploads[0][0]
    trunk_steps = coverage_steps(publisher, documents[publisher])
    if len(trunk_steps) != 1:
        return [f"{publisher} must generate coverage exactly once"]
    trunk = trunk_steps[0]
    found = _trunk_violations(publisher, trunk, uploads[0][1])
    lanes = [
        (name, document, step)
        for name, document in pull_request_closure(documents).items()
        for step in coverage_steps(name, document)
    ]
    if not lanes:
        found.append("no pull-request lane generates coverage for the ratchet")
    for name, document, step in lanes:
        found += _pull_request_lane(name, document, step, trunk)
    found += _artefact_uploads(documents, _with(trunk, "output-path"))
    return found + _baseline_writers(documents) + _push_writers(documents, publisher)
