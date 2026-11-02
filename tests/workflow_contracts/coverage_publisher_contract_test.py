"""Prove the publisher half of CV-005: one guarded push-to-main uploader.

The first test judges this repository's workflows. Every other test mutates a
copy of them in the way a later edit could and asserts that the clause meant
to catch that edit does.
"""

from __future__ import annotations

import copy
import typing as typ

import pytest
from coverage_contract_support import (
    LANE,
    Documents,
    assert_clean,
    assert_reports,
    coverage_step,
    find_publisher,
    first_job,
    job_steps,
    replace_triggers,
)
from coverage_input_rules import coverage_violations
from coverage_publisher_rules import publisher_violations, retired_names

if typ.TYPE_CHECKING:
    import collections.abc as cabc

DISPATCH = "github.event_name == 'workflow_dispatch'"
PRESENCE_GUARD = "steps.codescene-token.outputs.available == 'true'"
MAIN_GUARD = f"{PRESENCE_GUARD} && github.ref == 'refs/heads/main'"


def test_repository_has_one_guarded_publisher(documents: Documents) -> None:
    """Hold every publisher clause over the workflows as committed."""
    for rule in (publisher_violations, coverage_violations, retired_names):
        assert_clean(rule, documents)


@pytest.mark.parametrize(
    "guard",
    [
        PRESENCE_GUARD,
        "env.CS_ACCESS_TOKEN != '' && github.ref == 'refs/heads/main'",
        "github.ref == 'refs/heads/main'",
        f"{MAIN_GUARD} || {DISPATCH}",
        # Both required conjuncts stay whole and the `||` hides inside an
        # extra one. Exact conjunct equality refuses it, which is why this
        # contract needs no separate `||` scan.
        f"{MAIN_GUARD} && github.actor != 'x' || {DISPATCH}",
        f"{MAIN_GUARD} && false",
        # A split on `&&` alone would accept the negation of the whole guard.
        f"!({MAIN_GUARD})",
    ],
)
def test_upload_guard_is_exactly_token_and_main(
    documents: Documents, guard: str
) -> None:
    """The upload runs only with the token, and only for main."""
    _, upload = find_publisher(documents)
    upload["if"] = guard
    assert_reports(publisher_violations, documents, "upload must be guarded")


def test_upload_guard_accepts_the_expression_wrapper(documents: Documents) -> None:
    """``${{ }}`` around the condition is the same condition."""
    _, upload = find_publisher(documents)
    upload["if"] = f"${{{{ github.ref == 'refs/heads/main' && {PRESENCE_GUARD} }}}}"
    assert_clean(publisher_violations, documents)


@pytest.mark.parametrize(
    ("concurrency", "expected"),
    [
        (
            {
                "group": "${{ github.workflow }}-${{ github.ref }}",
                "cancel-in-progress": True,
            },
            "cancels a publisher run in progress",
        ),
        (
            {
                "group": "${{ github.workflow }}-${{ github.ref }}",
                "cancel-in-progress": "${{ true }}",
            },
            "cancels a publisher run in progress",
        ),
        (None, "workflow-level group"),
    ],
)
def test_publisher_never_cancels(
    documents: Documents, concurrency: object, expected: str
) -> None:
    """A cancelled publisher abandons its upload and its baseline write."""
    publisher, _ = find_publisher(documents)
    publisher["concurrency"] = concurrency
    assert_reports(publisher_violations, documents, expected)


def test_publisher_group_is_declared_at_one_scope(documents: Documents) -> None:
    """A group repeated on the upload job is a deadlock GitHub resolves by cancelling."""
    publisher, _ = find_publisher(documents)
    first_job(publisher)["concurrency"] = copy.deepcopy(publisher["concurrency"])
    assert_reports(publisher_violations, documents, "workflow scope only")


@pytest.mark.parametrize(
    "group",
    [
        "coverage-main",
        # Keyed on the event as well, an earlier trigger could finish after a
        # newer one and publish older coverage last.
        "${{ github.workflow }}-${{ github.ref }}-${{ github.event_name }}",
        # Names the ref as a word and evaluates nothing.
        "coverage-main-github.ref",
    ],
)
def test_publisher_group_is_exactly_workflow_and_ref(
    documents: Documents, group: str
) -> None:
    """Only one group per ref keeps triggered uploads in commit order."""
    publisher, _ = find_publisher(documents)
    publisher["concurrency"] = {"group": group, "cancel-in-progress": False}
    assert_reports(publisher_violations, documents, "workflow-level group")


def test_publisher_group_accepts_expression_spacing(documents: Documents) -> None:
    """``${{github.ref}}`` is the same expression as ``${{ github.ref }}``."""
    publisher, _ = find_publisher(documents)
    publisher["concurrency"] = {
        "group": "${{github.workflow}}-${{  github.ref }}",
        "cancel-in-progress": False,
    }
    assert_clean(publisher_violations, documents)


@pytest.mark.parametrize(
    ("events", "expected"),
    [
        (
            {"push": {"branches": ["main"]}, "workflow_dispatch": None},
            "must answer exactly [",
        ),
        ({"workflow_dispatch": None}, "must answer exactly ["),
        ({"push": {"branches": ["**"]}}, "`push: branches: [main]`"),
        ({"push": {"tags": ["v*"]}}, "`push: branches: [main]`"),
        ({"pull_request": None, "push": {"branches": ["main"]}}, "must answer exactly"),
    ],
)
def test_publisher_answers_only_a_push_to_main(
    documents: Documents, events: object, expected: str
) -> None:
    """Only main's pushes may write the baseline CodeScene is given."""
    publisher, _ = find_publisher(documents)
    replace_triggers(publisher, events)
    assert_reports(publisher_violations, documents, expected)


@pytest.mark.parametrize("scope", ["upload step", "upload job"])
def test_upload_cannot_fail_green(documents: Documents, scope: str) -> None:
    """``continue-on-error`` hides every failed upload behind a green run."""
    publisher, upload = find_publisher(documents)
    target = upload if scope == "upload step" else first_job(publisher)
    target["continue-on-error"] = True
    assert_reports(publisher_violations, documents, "must not continue on error")


def test_publisher_job_runs_unconditionally(documents: Documents) -> None:
    """A job-level ``if: false`` skips the upload with every step intact."""
    publisher, _ = find_publisher(documents)
    first_job(publisher)["if"] = "false"
    assert_reports(publisher_violations, documents, "must run unconditionally")


@pytest.mark.parametrize(
    ("key", "value", "expected"),
    [
        ("access-token", "${{ secrets.OTHER }}", "must pass access-token"),
        ("mode", "check", "must name `mode: upload`"),
    ],
)
def test_upload_inputs_are_asserted(
    documents: Documents, key: str, value: str, expected: str
) -> None:
    """The upload passes the bound token, and says that it uploads."""
    _, upload = find_publisher(documents)
    typ.cast("dict[str, object]", upload["with"])[key] = value
    assert_reports(publisher_violations, documents, expected)


def test_second_uploader_is_refused(documents: Documents) -> None:
    """A second uploader could publish where nothing here looks."""
    publisher, upload = find_publisher(documents)
    job_steps(publisher).append(copy.deepcopy(upload))
    found = publisher_violations(documents)
    expected = ["expected one CodeScene upload step, found 2"]
    assert found == expected, f"unexpected violations: {found}"


@pytest.mark.parametrize(
    "ref",
    ["@main", ""],
    ids=["branch", "missing"],
)
def test_publisher_actions_are_pinned(documents: Documents, ref: str) -> None:
    """A branch-named or unpinned action runs code nobody reviewed."""
    publisher, _ = find_publisher(documents)
    coverage_step(publisher)["uses"] = (
        "leynos/shared-actions/.github/actions/generate-coverage" + ref
    )
    assert_reports(publisher_violations, documents, "full commit SHA")


def test_upload_pin_matches_its_coverage_pin(documents: Documents) -> None:
    """An uploader at another revision was reviewed against other code."""
    _, upload = find_publisher(documents)
    upload["uses"] = (
        "leynos/shared-actions/.github/actions/upload-codescene-coverage@"
        + "0" * 40
    )
    assert_reports(publisher_violations, documents, "upload pin differs")


@pytest.mark.parametrize("permissions", [{"contents": "write"}, None])
def test_upload_job_token_is_read_only(
    documents: Documents, permissions: object
) -> None:
    """Nothing in the upload job writes to the repository."""
    publisher, _ = find_publisher(documents)
    first_job(publisher)["permissions"] = permissions
    assert_reports(publisher_violations, documents, "upload job permissions")


def test_publisher_checkout_keeps_no_credentials(documents: Documents) -> None:
    """The coverage run executes repository and dependency code after checkout."""
    publisher, _ = find_publisher(documents)
    checkout = next(
        step
        for step in job_steps(publisher)
        if "actions/checkout" in str(step.get("uses"))
    )
    checkout.pop("with", None)
    assert_reports(publisher_violations, documents, "persist-credentials: false")


def _restore_refresher(documents: Documents) -> None:
    """Bring back the workflow that refreshed the installer checksum."""
    documents["get-codescene-sha.yml"] = {True: "workflow_dispatch", "jobs": {}}


def _restore_installer_checksum(documents: Documents) -> None:
    """Pass the uploader the input it now rejects."""
    _, upload = find_publisher(documents)
    typ.cast("dict[str, object]", upload["with"])["installer-checksum"] = "abc"


def _restore_variable(documents: Documents) -> None:
    """Read the retired checksum variable into a lane."""
    documents[LANE]["env"] = {"CODESCENE_CLI_SHA256": "${{ vars.X }}"}


@pytest.mark.parametrize(
    "mutation",
    [_restore_refresher, _restore_installer_checksum, _restore_variable],
)
def test_retired_checksum_machinery_stays_gone(
    documents: Documents, mutation: cabc.Callable[[Documents], None]
) -> None:
    """The retired installer checksum is refused wherever it reappears."""
    mutation(documents)
    assert retired_names(documents) != [], "the retired machinery went unreported"
