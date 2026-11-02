"""Contract tests for runner placement, ceilings, and the actionlint registry.

Developer-blocking lanes run on Ubicloud, falling back to a GitHub-hosted
runner for pull requests from forks, which cannot obtain an Ubicloud runner.
The scheduled mutation lane and the Dependabot automerge lane are thin
callers of reusable workflows, so this repository declares no runner for
them at all.

Three things here are easy to get wrong in ways a green run does not show.

First, the folded scalar. Written as

.. code-block:: yaml

    runs-on: >-
      ${{ github.event.pull_request.head.repo.fork
          && 'ubuntu-latest' || 'ubicloud-standard-2' }}

the more-indented continuation keeps its line break, so the parsed value
carries a newline in the middle of the expression. GitHub evaluates it
regardless and the job runs, so nothing fails and nothing is reported. The
guard is therefore on the *parsed* value, not on the file's text.

Second, the narrowness of the expression check. A contract that matches the
shape of the expression rather than its content passes when
``head.repo.fork`` is replaced by a sibling field that reads just as
plausibly and selects the wrong runner. The guard expression and both arms
are compared by equality against exact strings.

Third, the registry. ``.github/actionlint.yaml`` exists only because
actionlint does not know Ubicloud's labels. A registry checked in one
direction rots: an entry left behind after a lane moves lets a later lane
take a runner family nobody reviewed while actionlint stays quiet. The
registered labels and the labels in use are compared as sets.

Mutation proof, recorded 2026-09-16; each applied alone and reverted, with
the failures observed rather than the ones intended:

- indenting the ``docs-gate`` continuation line one level deeper fails
  ``test_the_runner_expression_parses_to_one_line`` for that job, and
  ``test_the_runner_expression_is_exactly_the_reviewed_one`` with it, since
  the embedded newline also stops the expression matching;
- replacing ``github.event.pull_request.head.repo.fork`` with
  ``github.event.pull_request.head.repo.private`` on the ``test`` job, an
  otherwise identical expression, fails
  ``test_the_runner_expression_is_exactly_the_reviewed_one`` for that job
  alone and nothing else, which is what makes the check narrow rather than
  merely sufficient;
- removing the ``docs-gate`` entry from ``UBICLOUD_PLACEMENT`` fails
  ``test_every_job_is_pinned_by_coordinate`` alone;
- changing ``lint``'s pinned label from ``ubicloud-standard-4`` back to
  ``ubicloud-standard-2`` fails
  ``test_the_runner_expression_is_exactly_the_reviewed_one`` for ``lint``
  alone, so a right-sized shape cannot be quietly reverted;
- removing ``ubicloud-standard-4`` from the registry fails
  ``test_the_actionlint_registry_matches_the_labels_in_use``. This one was
  not contrived: the registry check failed the moment ``lint`` moved to four
  vCPU and before the registry was updated, which is the contract doing its
  job unprompted;
- deleting ``timeout-minutes`` from the ``lint`` job fails
  ``test_the_job_declares_its_reviewed_ceiling`` for that job and
  ``test_every_ubicloud_job_has_a_ceiling``, which is both halves of the
  ceiling rule reporting independently;
- removing ``ubicloud-standard-2`` from ``.github/actionlint.yaml`` fails
  ``test_the_actionlint_registry_matches_the_labels_in_use``; adding an
  unused ``ubicloud-standard-8`` to it fails the same test, so the registry
  is proved in both directions and not only against omission;
- adding ``.github/workflows/zz-probe.yaml``, one job on an unregistered
  ``ubicloud-standard-8``, fails ``test_every_job_is_pinned_by_coordinate``
  and ``test_the_actionlint_registry_matches_the_labels_in_use``. Narrowing
  ``WORKFLOW_FILE_PATTERNS`` back to ``("*.yml",)`` with that file still in
  place turns the whole suite green again, which is the hole this reader
  closes: GitHub runs a workflow written either way, so reading one spelling
  leaves the other outside every assertion here.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pytest

from ci_runner_placement_support import (
    DELEGATED_JOBS,
    FOREIGN_RUNNER_FRAGMENTS,
    FORK_FALLBACK_LABEL,
    GITHUB_HOSTED_LABELS,
    HOSTED_GUARD,
    MAIN_ONLY_PLACEMENT,
    RUNNER_EXPRESSION,
    UBICLOUD_LABEL_PREFIX,
    UBICLOUD_PLACEMENT,
    WORKFLOW_DIR,
    _case_id,
    _job_labels,
    _jobs,
    _labels_in_use,
    _registered_labels,
    _runner_value,
)

pytestmark = pytest.mark.skipif(
    not WORKFLOW_DIR.is_dir(),
    reason="workflow files not present in this working copy (e.g. inside "
    "mutmut's mutants/ sandbox, which does not copy .github/)",
)


def test_every_job_is_pinned_by_coordinate() -> None:
    """Scenario: a lane is added and nobody decides where it runs.

    Invariant: the pinned coordinates and the tree's jobs are the same set.
    Iterating only the pinned coordinates would never examine a job nobody
    listed, so a new lane could carry any label at all and satisfy every
    other assertion here.
    """
    observed = set(_jobs())
    expected = set(UBICLOUD_PLACEMENT) | set(MAIN_ONLY_PLACEMENT) | DELEGATED_JOBS
    assert observed == expected, (
        "runner placement is pinned per job; unpinned jobs "
        f"{sorted(observed - expected)} and stale pins "
        f"{sorted(expected - observed)} must be reconciled"
    )


@pytest.mark.parametrize("coordinate", sorted(UBICLOUD_PLACEMENT), ids=_case_id)
def test_the_runner_expression_parses_to_one_line(
    coordinate: tuple[str, str],
) -> None:
    """Scenario: the folded scalar's continuation is indented one level deeper.

    Invariant: the parsed value is a single line. A more-indented
    continuation keeps its line break, putting a newline inside the
    expression. GitHub evaluates the broken value and the job runs, so a
    green run is not evidence; only the parsed value shows it.
    """
    value = _runner_value(_jobs()[coordinate])
    assert isinstance(value, str), (
        f"{coordinate[0]}:{coordinate[1]} should declare runs-on as one "
        f"scalar, got {value!r}"
    )
    assert "\n" not in value, (
        f"{coordinate[0]}:{coordinate[1]} has a line break inside its runs-on "
        f"expression ({value!r}); the folded scalar's continuation line must "
        "sit at the same indent as the line above it"
    )


@pytest.mark.parametrize(
    ("coordinate", "label"),
    sorted((key, value[0]) for key, value in UBICLOUD_PLACEMENT.items()),
    ids=_case_id,
)
def test_the_runner_expression_is_exactly_the_reviewed_one(
    coordinate: tuple[str, str], label: str
) -> None:
    """Scenario: the fork guard is swapped for a plausible sibling field.

    Invariant: the guard and both arms equal the reviewed strings. Matching
    the expression's shape rather than its content passes when
    ``head.repo.fork`` becomes, say, ``head.repo.private``: an expression of
    exactly the same form that sends every fork pull request to a runner it
    cannot obtain, and every other pull request to the wrong place.
    """
    value = str(_runner_value(_jobs()[coordinate]))
    match = RUNNER_EXPRESSION.match(value)
    assert match is not None, (
        f"{coordinate[0]}:{coordinate[1]} should declare the reviewed fork "
        f"fallback expression; got {value!r}"
    )
    assert match["guard"] == HOSTED_GUARD, (
        f"{coordinate[0]}:{coordinate[1]} keys its fallback on "
        f"{match['guard']!r}; only {HOSTED_GUARD!r} identifies a fork or a "
        "Dependabot pull request"
    )
    assert match["fork_arm"] == FORK_FALLBACK_LABEL, (
        f"{coordinate[0]}:{coordinate[1]} sends forks to "
        f"{match['fork_arm']!r}; a fork cannot obtain an Ubicloud runner, so "
        f"the fork arm must be {FORK_FALLBACK_LABEL!r}"
    )
    assert match["default_arm"] == label, (
        f"{coordinate[0]}:{coordinate[1]} runs non-fork events on "
        f"{match['default_arm']!r}, not the reviewed {label!r}"
    )


@pytest.mark.parametrize(
    ("coordinate", "label"),
    sorted((key, value[0]) for key, value in MAIN_ONLY_PLACEMENT.items()),
    ids=_case_id,
)
def test_a_main_only_runner_is_exactly_the_reviewed_label(
    coordinate: tuple[str, str], label: str
) -> None:
    """Scenario: a main-only lane acquires a fallback arm it cannot need.

    Invariant: the declaration is the bare reviewed label. A fork fallback is
    written for a pull request, which cannot start such a workflow; carrying
    one would make widening the trigger look already provided for.
    """
    value = _runner_value(_jobs()[coordinate])
    assert value == label, (
        f"{coordinate[0]}:{coordinate[1]} runs on {value!r}; a main-only lane "
        f"declares {label!r} with no fork-fallback expression"
    )


@pytest.mark.parametrize(
    ("coordinate", "minutes"),
    sorted(
        (
            (key, value[1])
            for key, value in {**UBICLOUD_PLACEMENT, **MAIN_ONLY_PLACEMENT}.items()
        )
    ),
    ids=_case_id,
)
def test_the_job_declares_its_reviewed_ceiling(
    coordinate: tuple[str, str], minutes: int
) -> None:
    """Scenario: a ceiling drifts to a number nobody reviewed.

    Invariant: each job carries the exact ceiling the guide records. A
    per-minute runner bills until something stops it, so the six-hour
    default is one failure mode; a ceiling near the measured work is the
    other, because it cancels the run at the moment the overrun becomes
    interesting and discards the log that would explain it.
    """
    declared = _jobs()[coordinate].get("timeout-minutes")
    assert declared == minutes, (
        f"{coordinate[0]}:{coordinate[1]} must set timeout-minutes: "
        f"{minutes}, got {declared!r}"
    )


def test_every_ubicloud_job_has_a_ceiling() -> None:
    """Scenario: a new Ubicloud lane inherits GitHub's six-hour default.

    Invariant: every job that can select an Ubicloud label declares a
    ceiling. The pinned table above is keyed by coordinate; this reads the
    tree instead, so a lane cannot escape by being absent from a list.
    """
    unbounded = [
        coordinate
        for coordinate, definition in _jobs().items()
        if any(
            label.startswith(UBICLOUD_LABEL_PREFIX) for label in _job_labels(definition)
        )
        and definition.get("timeout-minutes") is None
    ]
    assert not unbounded, (
        f"jobs on an Ubicloud runner without a ceiling: {sorted(unbounded)}"
    )


@pytest.mark.parametrize("coordinate", sorted(DELEGATED_JOBS), ids=_case_id)
def test_a_reusable_caller_declares_no_runner(
    coordinate: tuple[str, str],
) -> None:
    """Scenario: a scheduled or administrative lane acquires an Ubicloud label.

    Invariant: these coordinates call a reusable workflow and declare no
    runner, so the callee places them. Scheduled, mutation and Dependabot
    lanes stay GitHub-hosted, where public-repository minutes are free; a
    ``runs-on`` appearing here is the repository taking a placement decision
    it has not reviewed.
    """
    definition = _jobs()[coordinate]
    assert "uses" in definition, (
        f"{coordinate[0]}:{coordinate[1]} is recorded as a reusable-workflow "
        "caller but declares no uses key"
    )
    assert _runner_value(definition) is None, (
        f"{coordinate[0]}:{coordinate[1]} calls a reusable workflow, so its "
        f"runner is the callee's; found runs-on "
        f"{_runner_value(definition)!r}"
    )


def test_the_actionlint_registry_matches_the_labels_in_use() -> None:
    """Scenario: the registry and the workflows drift apart.

    Invariant: the registered labels are exactly the non-GitHub-hosted
    labels in use, compared in both directions. An unregistered label makes
    actionlint report a lane nobody broke; a registered label no lane uses
    is worse, because it silently permits a runner family nobody
    reviewed for whatever lane adopts it next.
    """
    needing_registration = _labels_in_use() - set(GITHUB_HOSTED_LABELS)
    registered = _registered_labels()
    assert registered == needing_registration, (
        "the actionlint runner registry must hold exactly the labels in use; "
        f"unregistered {sorted(needing_registration - registered)} and stale "
        f"{sorted(registered - needing_registration)}"
    )


def test_no_lane_uses_a_foreign_runner_family() -> None:
    """Scenario: a Windows, macOS or ad-hoc self-hosted lane appears.

    Invariant: this repository has no such lane. The reading is by
    substring, which is the safe direction for a prohibition: a renamed or
    neutered label still leaves its family's text behind.
    """
    offenders = sorted(
        label
        for label in _labels_in_use()
        if any(fragment in label.lower() for fragment in FOREIGN_RUNNER_FRAGMENTS)
    )
    assert not offenders, (
        "this repository has no Windows, macOS or ad-hoc self-hosted lane; "
        f"found {offenders}"
    )
