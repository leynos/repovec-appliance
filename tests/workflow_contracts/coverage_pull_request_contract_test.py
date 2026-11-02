"""Prove the pull-request half of CV-005: no PR lane reaches CodeScene.

The first test judges this repository's workflows. Every other test mutates a
copy of them, or extends the closure with a probe workflow, in the way a later
edit could, and asserts that the clause meant to catch that edit does. A clause
no mutation fails is indistinguishable from one never written.
"""

from __future__ import annotations

import typing as typ

import pytest
from coverage_contract_support import (
    CREDENTIAL_REFERENCE,
    LANE,
    PROBE,
    Documents,
    job_steps,
    lane_jobs,
)
from coverage_pull_request_rules import (
    pull_request_closure,
    pull_request_contacts,
)
from coverage_workflow_reader import Document, WorkflowError
from workflow_loader import load_workflow

UPLOADER = "leynos/shared-actions/.github/actions/upload-codescene-coverage"


def test_repository_keeps_codescene_off_pull_requests(documents: Documents) -> None:
    """Hold every pull-request clause over the workflows as committed."""
    found = pull_request_contacts(documents)
    assert found == [], f"pull-request lanes reach CodeScene: {found}"


def _assert_contact(documents: Documents, expected: str) -> None:
    """Assert that the pull-request rule reports one expected contact."""
    found = pull_request_contacts(documents)
    assert expected in found, f"missing {expected!r} in {found}"


def _probe(prefix: str = "./") -> tuple[Document, Document]:
    """Return a pull-request caller job and a callee reaching CodeScene.

    The callee declares only ``workflow_call``, so it serves no pull request by
    its own trigger, and receives the token through ``secrets: inherit``.
    """
    callee = load_workflow(
        "on:\n  workflow_call:\njobs:\n  leak:\n    runs-on: ubuntu-latest\n"
        '    steps:\n      - run: curl -H "$T" https://api.codescene.io/v2\n'
        f"        env:\n          T: '{CREDENTIAL_REFERENCE}'\n"
    )
    caller = {"uses": f"{prefix}.github/workflows/{PROBE}", "secrets": "inherit"}
    return caller, callee


@pytest.mark.parametrize("prefix", ["./", "$/"])
def test_closure_follows_a_called_workflow(
    documents: Documents, prefix: str
) -> None:
    """A workflow_call callee of a pull-request job is judged as a PR lane."""
    caller, callee = _probe(prefix)
    documents[PROBE] = callee
    lane_jobs(documents)["probe"] = caller
    assert PROBE in pull_request_closure(documents), "the callee left the closure"
    found = pull_request_contacts(documents)
    for expected in (
        f"{PROBE} names the CodeScene host",
        f"{PROBE} puts CS_ACCESS_TOKEN in reach",
        f"{LANE} job probe forwards every secret with `secrets: inherit`",
    ):
        assert expected in found, f"missing {expected!r} in {found}"


def test_closure_follows_a_workflow_run_chain(documents: Documents) -> None:
    """A workflow_run chained onto a pull-request workflow is a PR lane."""
    _, callee = _probe()
    lane = str(documents[LANE].get("name", LANE))
    callee[True] = {"workflow_run": {"workflows": [lane]}}
    documents[PROBE] = callee
    _assert_contact(documents, f"{PROBE} names the CodeScene host")


def test_workflow_run_matches_an_unnamed_workflow_by_path(
    documents: Documents,
) -> None:
    """GitHub names a workflow without ``name:`` by its path from the root."""
    _, callee = _probe()
    documents[LANE].pop("name", None)
    callee[True] = {"workflow_run": {"workflows": [f".github/workflows/{LANE}"]}}
    documents[PROBE] = callee
    _assert_contact(documents, f"{PROBE} names the CodeScene host")


def test_workflow_run_matches_a_glob_pattern(documents: Documents) -> None:
    """GitHub matches a ``workflows:`` entry as a glob against the name."""
    _, callee = _probe()
    lane = str(documents[LANE].get("name", LANE))
    callee[True] = {"workflow_run": {"workflows": [f"{lane}*"]}}
    documents[PROBE] = callee
    _assert_contact(documents, f"{PROBE} names the CodeScene host")


@pytest.mark.parametrize(
    ("uses", "reason"),
    [
        ("$/.github/workflows/ci.yml@main", "cannot name a ref"),
        (
            "leynos/repovec-appliance/.github/workflows/ci.yml@main",
            "runs this repository's workflow at a ref",
        ),
        ("./.github/workflows/missing.yml", "names no workflow in this repository"),
    ],
)
def test_closure_refuses_calls_it_cannot_read(
    documents: Documents, uses: str, reason: str
) -> None:
    """A call the closure cannot follow to a checked-out file is refused."""
    lane_jobs(documents)["probe"] = {"uses": uses}
    with pytest.raises(WorkflowError, match=reason):
        pull_request_closure(documents)


@pytest.mark.parametrize(
    "event",
    [
        "check_run",
        "check_suite",
        "issue_comment",
        "merge_group",
        "pull_request_review",
        "pull_request_review_comment",
        "pull_request_target",
        "push",
        "status",
        "{push: {branches: ['**']}}",
        "an_event_github_adds_later",
    ],
)
def test_closure_starts_from_every_pull_request_event(
    documents: Documents, event: str
) -> None:
    """Any event a pull request can start runs with the repository's secrets."""
    documents[PROBE] = load_workflow(
        f"on: {event}\njobs:\n  a:\n    steps:\n"
        "      - run: curl https://codescene.io\n"
    )
    _assert_contact(documents, f"{PROBE} names the CodeScene host")


@pytest.mark.parametrize(
    "trigger",
    [
        "{push: {branches: [main]}}",
        "{push: {tags: ['v*']}}",
        "{release: {types: [published]}}",
        "{schedule: [{cron: '0 0 * * *'}]}",
        "workflow_call",
        "workflow_dispatch",
        "{workflow_run: {workflows: [Unrelated]}}",
    ],
)
def test_events_no_pull_request_starts_stay_off_the_surface(
    documents: Documents, trigger: str
) -> None:
    """The seed rule is narrow: these runs never start for a pull request."""
    documents[PROBE] = load_workflow(
        f"on: {trigger}\njobs:\n  a:\n    steps:\n"
        "      - run: curl https://codescene.io\n"
    )
    reachable = pull_request_closure(documents)
    assert PROBE not in reachable, f"{trigger} reached the pull-request surface"


def test_callee_secret_declaration_is_refused(documents: Documents) -> None:
    """A called workflow declaring the secret by name is refused.

    The declaration is a mapping key with no reference in any value, so only a
    reading of keys as well as values sees it.
    """
    documents[PROBE] = load_workflow(
        "on:\n  workflow_call:\n    secrets:\n      CS_ACCESS_TOKEN:\n"
        "        required: false\njobs: {}\n"
    )
    lane_jobs(documents)["probe"] = {"uses": f"./.github/workflows/{PROBE}"}
    _assert_contact(documents, f"{PROBE} puts CS_ACCESS_TOKEN in reach")


@pytest.mark.parametrize(
    ("step", "reason"),
    [
        ({"run": f"echo {CREDENTIAL_REFERENCE}"}, "puts CS_ACCESS_TOKEN in reach"),
        (
            {"uses": "x/y@v1", "with": {"t": CREDENTIAL_REFERENCE}},
            "puts CS_ACCESS_TOKEN in reach",
        ),
        (
            {"run": "true", "env": {"OTHER": CREDENTIAL_REFERENCE}},
            "puts CS_ACCESS_TOKEN in reach",
        ),
        ({"run": "echo '${{ toJSON( secrets ) }}'"}, "serializes the secrets context"),
        ({"run": "echo ${{ secrets['CS_' + 'X'] }}"}, "indexes the secrets context"),
        ({"run": "curl https://API.CODESCENE.IO"}, "names the CodeScene host"),
        ({"run": "cs-coverage check coverage.xml"}, "names the cs-coverage client"),
        ({"uses": f"{UPLOADER}@x"}, "calls the CodeScene uploader"),
    ],
)
def test_pull_request_lane_cannot_reach_codescene(
    documents: Documents, step: dict[str, object], reason: str
) -> None:
    """Every route to CodeScene or its token from a PR step is refused."""
    job_steps(documents[LANE]).append(step)
    _assert_contact(documents, f"{LANE} {reason}")


def test_token_in_workflow_env_is_refused(documents: Documents) -> None:
    """A workflow-level env reaches every step of every job."""
    documents[LANE]["env"] = {"CS_ACCESS_TOKEN": CREDENTIAL_REFERENCE}
    _assert_contact(documents, f"{LANE} puts CS_ACCESS_TOKEN in reach")


def test_host_in_workflow_defaults_is_refused(documents: Documents) -> None:
    """A default shell runs before every step, naming no step at all."""
    documents[LANE]["defaults"] = {
        "run": {"shell": "curl -s https://Api.CodeScene.io >/dev/null; bash {0}"}
    }
    _assert_contact(documents, f"{LANE} names the CodeScene host")


def test_both_trigger_spellings_are_refused() -> None:
    """GitHub merges ``on`` and ``'on'``; a reader of either is blind to the other."""
    text = "on: push\n'on': pull_request\njobs: {}\n"
    with pytest.raises(WorkflowError, match="declares `on` 2 times"):
        pull_request_closure({"both.yml": load_workflow(text)})


def test_lane_own_code_is_not_judged_as_a_callee(documents: Documents) -> None:
    """A pruning rule would hide the lane itself from the closure."""
    assert LANE in pull_request_closure(documents)