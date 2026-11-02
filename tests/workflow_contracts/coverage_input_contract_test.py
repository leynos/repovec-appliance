"""Prove the pull-request lane measures exactly what main publishes.

Each test mutates a copy of the workflows the way a later edit could and
asserts that the clause meant to catch it does: the lane's coverage step
ratchets against main's baseline at the publisher's pin and selection, it
publishes nothing, and nothing else writes a baseline.
"""

from __future__ import annotations

import copy
import typing as typ

import pytest
from coverage_contract_support import (
    EXPECTED_SELECTION,
    LANE,
    PUBLISHER,
    Documents,
    assert_clean,
    assert_reports,
    coverage_job,
    coverage_step,
    find_publisher,
    job_steps,
)
from coverage_input_rules import coverage_violations


def test_repository_ratchets_like_main(documents: Documents) -> None:
    """The lane and the publisher select the same measurement, as committed."""
    assert_clean(coverage_violations, documents)


@pytest.mark.parametrize(
    ("key", "value", "expected"),
    [
        ("with-ratchet", "false", "with-ratchet 'true'"),
        ("publish-artefact", "true", "publish-artefact 'false'"),
        ("language", "python", "selection differs"),
        ("all-targets", "false", "selection differs"),
        ("python-source", "./tests", "selection differs"),
    ],
)
def test_pull_request_coverage_ratchets_like_main(
    documents: Documents, key: str, value: str, expected: str
) -> None:
    """The PR lane ratchets against main's baseline and publishes nothing."""
    typ.cast("dict[str, object]", coverage_step(documents[LANE])["with"])[key] = value
    assert_reports(coverage_violations, documents, expected)


@pytest.mark.parametrize("guard", ["false", "github.event_name == 'push'", None])
def test_pull_request_coverage_cannot_be_switched_off(
    documents: Documents, guard: object
) -> None:
    """Any condition on the step keeps it while the ratchet may never run."""
    coverage_step(documents[LANE])["if"] = guard
    assert_reports(coverage_violations, documents, "step must not carry a condition")


@pytest.mark.parametrize(
    "guard",
    [
        "false",
        "github.event_name == 'push'",
        "github.event_name == 'pull_request' || github.event_name == 'push'",
    ],
)
def test_pull_request_coverage_job_runs_only_for_pull_requests(
    documents: Documents, guard: object
) -> None:
    """The workflow answers pushes too; the lane must not measure there."""
    coverage_job(documents[LANE])["if"] = guard
    assert_reports(coverage_violations, documents, "job must run exactly for pull requests")


def test_pull_request_coverage_job_guard_cannot_be_removed(
    documents: Documents,
) -> None:
    """Without the guard the lane also measures main, beside the publisher."""
    coverage_job(documents[LANE]).pop("if")
    assert_reports(coverage_violations, documents, "job must run exactly for pull requests")


@pytest.mark.parametrize("scope", ["step", "job"])
def test_pull_request_coverage_cannot_fail_green(
    documents: Documents, scope: str
) -> None:
    """A ratchet failure that turns green no longer holds the lane."""
    target = (
        coverage_step(documents[LANE])
        if scope == "step"
        else coverage_job(documents[LANE])
    )
    target["continue-on-error"] = True
    assert_reports(coverage_violations, documents, "must not continue on error")


def test_pull_request_coverage_pin_matches_the_publisher(
    documents: Documents,
) -> None:
    """A lane at another pin measures with different code from the baseline's."""
    step = coverage_step(documents[LANE])
    step["uses"] = str(step["uses"]).partition("@")[0] + "@" + "0" * 40
    assert_reports(coverage_violations, documents, "pin differs from the publisher's")


def test_pull_request_coverage_job_is_read_only(documents: Documents) -> None:
    """The lane runs contributed code with its job's token."""
    coverage_job(documents[LANE]).pop("permissions", None)
    assert_reports(coverage_violations, documents, "coverage job permissions")


def test_repository_selection_is_pinned(documents: Documents) -> None:
    """Both lanes changing their selection together would pass parity alone."""
    found = coverage_step(find_publisher(documents)[0]).get("with")
    assert found == EXPECTED_SELECTION, f"coverage selection is {found}"


def test_both_lanes_drifting_together_is_refused(documents: Documents) -> None:
    """Parity does not replace a pinned selection, only guard it."""
    for document in (find_publisher(documents)[0], documents[LANE]):
        inputs = typ.cast("dict[str, object]", coverage_step(document)["with"])
        inputs["language"] = "python"
    assert_reports(
        coverage_violations, documents, "coverage selection must be the pinned one"
    )


def test_pull_request_lane_cannot_upload_the_report(documents: Documents) -> None:
    """``publish-artefact: 'false'`` is moot if another step uploads the report."""
    report = typ.cast("dict[str, object]", coverage_step(documents[LANE])["with"])
    job_steps(documents[LANE]).append(
        {
            "uses": "actions/upload-artifact@v4",
            "with": {"name": "coverage", "path": report["output-path"]},
        }
    )
    assert_reports(coverage_violations, documents, "must not upload the coverage")


@pytest.mark.parametrize("lane", ["publisher", "pull request"])
def test_only_the_publisher_writes_the_baseline(
    documents: Documents, lane: str
) -> None:
    """``publish-baseline: always`` lets a branch write the ratchet baseline."""
    document = find_publisher(documents)[0] if lane == "publisher" else documents[LANE]
    inputs = typ.cast("dict[str, object]", coverage_step(document)["with"])
    inputs["publish-baseline"] = "always"
    assert_reports(coverage_violations, documents, "publish-baseline")


def test_push_only_callee_cannot_write_a_second_baseline(
    documents: Documents,
) -> None:
    """A push workflow's local callee runs on the push, so its coverage counts."""
    publisher, _ = find_publisher(documents)
    documents["cov.yml"] = {
        True: {"workflow_call": None},
        "jobs": {"c": {"steps": [copy.deepcopy(coverage_step(publisher))]}},
    }
    documents["caller.yml"] = {
        True: {"push": {"branches": ["main"]}},
        "jobs": {"call": {"uses": "./.github/workflows/cov.yml"}},
    }
    assert_reports(
        coverage_violations,
        documents,
        "cov.yml coverage can run on a push outside the publisher",
    )


def test_pull_request_coverage_must_exist(documents: Documents) -> None:
    """Deleting the PR coverage step deletes the ratchet."""
    steps = job_steps(documents[LANE])
    steps.remove(coverage_step(documents[LANE]))
    assert_reports(
        coverage_violations,
        documents,
        "no pull-request lane generates coverage for the ratchet",
    )


def test_upload_reads_what_coverage_wrote(documents: Documents) -> None:
    """An upload of another file publishes nothing the run measured."""
    publisher, upload = find_publisher(documents)
    typ.cast("dict[str, object]", upload["with"])["path"] = "other-report.info"
    assert_reports(
        coverage_violations, documents, "coverage step's output-path"
    )


def test_publisher_must_be_the_single_uploader(documents: Documents) -> None:
    """Without one publisher there is no baseline for the lanes to compare with."""
    publisher, upload = find_publisher(documents)
    job_steps(publisher).remove(upload)
    found = coverage_violations(documents)
    expected = ["coverage lanes need exactly one publisher to compare against"]
    assert found == expected, f"unexpected violations: {found}"


def test_publisher_generates_coverage_once(documents: Documents) -> None:
    """Two generators in the publisher leave no single selection to compare."""
    publisher, _ = find_publisher(documents)
    job_steps(publisher).append(copy.deepcopy(coverage_step(publisher)))
    found = coverage_violations(documents)
    expected = [f"{PUBLISHER} must generate coverage exactly once"]
    assert found == expected, f"unexpected violations: {found}"
