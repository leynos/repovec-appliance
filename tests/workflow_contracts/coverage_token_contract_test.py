"""Prove where the publisher may name the CodeScene token.

No ``env`` binds the token. A check step runs one exact command that Actions
expands to ``available=true`` or ``available=false``, and the token reaches the
uploader only as its ``access-token`` input: the uploader is a composite
action, so a token in its step's ``env`` would reach the nested steps too.
Every test mutates a copy of this repository's workflows and asserts that the
clause meant to catch the edit does.
"""

from __future__ import annotations

import typing as typ

import pytest
from coverage_contract_support import (
    CREDENTIAL_REFERENCE,
    Documents,
    assert_clean,
    assert_reports,
    find_publisher,
    first_job,
    job_steps,
)
from coverage_publisher_rules import publisher_violations

if typ.TYPE_CHECKING:
    from coverage_workflow_reader import Step


def _check_step(documents: Documents) -> Step:
    """Return the publisher's token presence check."""
    publisher, _ = find_publisher(documents)
    return next(
        step for step in job_steps(publisher) if step.get("id") == "codescene-token"
    )


def test_repository_places_the_token(documents: Documents) -> None:
    """The token reaches only the check command and the upload input."""
    assert_clean(publisher_violations, documents)


def test_token_cannot_return_to_the_upload_env(documents: Documents) -> None:
    """The composite uploader would pass its step env to nested steps."""
    _, upload = find_publisher(documents)
    upload["env"] = {"CS_ACCESS_TOKEN": CREDENTIAL_REFERENCE}
    assert_reports(publisher_violations, documents, "must not bind CS_ACCESS_TOKEN")


def test_token_cannot_move_to_the_job(documents: Documents) -> None:
    """A binding in a wider scope reaches every step of the job."""
    publisher, _ = find_publisher(documents)
    first_job(publisher)["env"] = {"CS_ACCESS_TOKEN": CREDENTIAL_REFERENCE}
    assert_reports(publisher_violations, documents, "outside its two steps")


def test_token_cannot_move_to_the_workflow(documents: Documents) -> None:
    """Workflow scope is wider still, and reaches every job."""
    publisher, _ = find_publisher(documents)
    publisher["env"] = {"CS_ACCESS_TOKEN": CREDENTIAL_REFERENCE}
    assert_reports(publisher_violations, documents, "outside its two steps")


def test_check_step_must_exist(documents: Documents) -> None:
    """Without it the guard's output is never written and the upload never runs."""
    publisher, _ = find_publisher(documents)
    job_steps(publisher).remove(_check_step(documents))
    assert_reports(publisher_violations, documents, "needs one `codescene-token`")


def test_check_step_must_precede_the_upload(documents: Documents) -> None:
    """An output written after the upload cannot enable it."""
    publisher, upload = find_publisher(documents)
    held = job_steps(publisher)
    check = _check_step(documents)
    held.remove(check)
    held.insert(held.index(upload) + 1, check)
    assert_reports(publisher_violations, documents, "must run before the upload")


def test_token_cannot_be_bound_in_the_check_step(documents: Documents) -> None:
    """The check step needs no binding: its expression reads the secret."""
    _check_step(documents)["env"] = {"CS_ACCESS_TOKEN": CREDENTIAL_REFERENCE}
    assert_reports(publisher_violations, documents, "may carry only")


@pytest.mark.parametrize(
    ("change", "expected"),
    [
        ({"if": "false"}, "may carry only"),
        ({"shell": "python {0}"}, "may carry only"),
        ({"continue-on-error": True}, "must not continue on error"),
        ({"run": 'echo "available=true" >> "$GITHUB_OUTPUT"'}, "must run exactly"),
        (
            {"run": 'test -z "$CS_ACCESS_TOKEN" || echo "available=true" >> x'},
            "must run exactly",
        ),
        (
            {
                "run": 'false && echo "available=${{ secrets.CS_ACCESS_TOKEN '
                '!= \'\' }}" >> "$GITHUB_OUTPUT"'
            },
            "must run exactly",
        ),
    ],
)
def test_check_step_runs_one_exact_command(
    documents: Documents, change: dict[str, object], expected: str
) -> None:
    """The check step can neither be skipped nor run anything else."""
    _check_step(documents).update(change)
    assert_reports(publisher_violations, documents, expected)


@pytest.mark.parametrize(
    ("key", "value", "expected"),
    [
        ("access-token", "${{ secrets.OTHER }}", "must pass access-token"),
        ("access-token", "", "must pass access-token"),
    ],
)
def test_upload_receives_the_bound_token(
    documents: Documents, key: str, value: str, expected: str
) -> None:
    """An upload of another value publishes nothing this run measured."""
    _, upload = find_publisher(documents)
    typ.cast("dict[str, object]", upload["with"])[key] = value
    assert_reports(publisher_violations, documents, expected)