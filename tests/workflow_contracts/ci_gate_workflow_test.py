"""Contract tests for the CI jobs that run the repository's gates.

A contract that finds a command's text in the workflow file certifies
that the text is present, not that it runs. Two mutations defeat a
textual check while leaving the command visible: wrapping the run value
so nothing executes, as in

.. code-block:: yaml

    run: |
      if false; then
        make lint
      fi

and disabling the step or its job with ``if: false``, which leaves the
command untouched on its own line. The first was already caught by the
Rust contract this file replaces, because that check required the whole
``run: make lint`` on one line. The second passed it: measured on
2026-09-07, adding ``if: false`` to the Lint step left
``continuous_integration_runs_the_lint_gate`` green while the step ran
nothing.

So the workflow is parsed rather than searched. A gate is satisfied only
when the job exists, neither the job nor the step is disabled, and one of
the job's steps has a ``run`` value that *is* the command, rather than a
value that merely contains it.

Mutation proof, recorded 2026-09-07; each applied alone and reverted:

- ``if: false`` on the Lint step fails ``test_the_gate_step_runs``;
- ``if: false`` on the lint job fails ``test_the_gate_step_runs``;
- a plausible-looking ``if: github.event_name == 'push'`` on the Lint
  step fails it too, which is the point of rejecting any condition
  rather than enumerating falsy ones;
- wrapping the Lint run value in ``if false; then ... fi`` fails
  ``test_the_gate_step_runs``;
- changing the Test step's run value to ``make build`` fails it for the
  ``test`` gate;
- removing the ``pull_request`` trigger fails
  ``test_the_workflow_runs_on_pull_requests``.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from pathlib import Path

import pytest
import yaml

WORKFLOW_PATH = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "ci.yml"

pytestmark = pytest.mark.skipif(
    not WORKFLOW_PATH.exists(),
    reason="workflow file not present in this working copy (e.g. inside "
    "mutmut's mutants/ sandbox, which does not copy .github/)",
)

#: Gates this pull request's claims rest on, as (job, exact run value).
#: ``lint`` enforces the environment-access policy and ``test`` runs the
#: Rust test-gate runner; both are worthless if the step does not run.
REQUIRED_GATE_STEPS = [
    pytest.param("lint", "make lint", id="lint"),
    pytest.param("lint", "make test-workflow-contracts", id="workflow-contracts"),
    pytest.param("test", "make test", id="test"),
]

#: A required gate must run on every pull request, so it carries no
#: condition at all. Enumerating falsy spellings is not enough: ``if:
#: false`` parses as a boolean rather than the string "false", and a
#: condition that reads as plausible, such as ``github.event_name ==
#: 'push'``, skips the gate on exactly the runs it exists to guard.


def _workflow() -> dict[str, object]:
    """Parse the CI workflow."""
    return yaml.safe_load(WORKFLOW_PATH.read_text(encoding="utf-8"))


def _job(name: str) -> dict[str, object]:
    """Return one job definition, failing clearly when it is absent."""
    jobs = _workflow().get("jobs", {})
    assert name in jobs, f"the CI workflow should define a {name!r} job"
    return jobs[name]


def _condition(definition: dict[str, object]) -> object | None:
    """Return a job's or step's ``if`` condition, if it carries one."""
    return definition.get("if")


def test_the_workflow_runs_on_pull_requests() -> None:
    """Scenario: the workflow stops triggering on pull requests.

    Invariant: the gates run before merge, not only after it. PyYAML
    parses the bare ``on`` key as the boolean ``True``, so both spellings
    are accepted.
    """
    workflow = _workflow()
    triggers = workflow.get("on", workflow.get(True))

    assert triggers is not None, "the CI workflow should declare triggers"
    assert "pull_request" in triggers, (
        f"the CI workflow should run on pull requests; found {sorted(triggers)}"
    )


@pytest.mark.parametrize(("job_name", "command"), REQUIRED_GATE_STEPS)
def test_the_gate_step_runs(job_name: str, command: str) -> None:
    """Scenario: a gate step is disabled or wrapped so it executes nothing.

    Invariant: neither the job nor the step carries a condition, and the
    step's whole ``run`` value is the command. Matching the whole value
    rather than a substring is what rejects a value wrapping the command
    in a condition that never holds.
    """
    job = _job(job_name)
    assert _condition(job) is None, (
        f"the {job_name} job carries an if condition ({_condition(job)!r}); a "
        "required gate must run unconditionally on every pull request"
    )

    matching = [
        step
        for step in job.get("steps", [])
        if isinstance(step, dict) and str(step.get("run", "")).strip() == command
    ]
    assert matching, (
        f"the {job_name} job should have a step whose run value is exactly "
        f"{command!r}; found "
        f"{[str(step.get('run', '')).strip() for step in job.get('steps', []) if isinstance(step, dict) and 'run' in step]}"
    )

    unconditional = [step for step in matching if _condition(step) is None]
    assert unconditional, (
        f"every {job_name} step running {command!r} carries an if condition "
        f"({[_condition(step) for step in matching]!r}), so the gate may not run"
    )
