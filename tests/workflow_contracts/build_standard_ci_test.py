"""Contract tests for the build standard's wiring into CI.

Every CI job that compiles or links Rust needs the pinned mold linker and
the pinned toolchain before its compiling step runs. The install step
therefore has to be in the right jobs, at the right place, and
unconditional: a step guarded by an ``if`` that happens to be false
leaves the command visible on its own line while never running it.

The compiling jobs are derived rather than listed. A job compiles when a
``run`` value invokes a Make target that compiles or a Cargo subcommand
that compiles. The set of jobs that install is then compared with that
derived set in both directions, so a new compiling job without an
install fails, and an install in a job that does not compile is reviewed
rather than silently accepted.

The Make targets themselves are held to the same rule by
``build_standard_make_test.py``: each standard route reaches
``check-build-tools`` before compiling. This file proves the jobs that
run those routes have the tools available to pass that check.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import pytest

from build_standard_probe import (
    COMPILING_CARGO_SUBCOMMANDS,
    COMPILING_MAKE_TARGETS,
    WORKFLOW_PATH,
    cargo_subcommands,
    make_targets,
    run_value_compiles,
)
from workflow_loader import repository_workflows

pytestmark = pytest.mark.skipif(
    not WORKFLOW_PATH.exists(),
    reason="workflow file not present in this working copy (e.g. inside "
    "mutmut's mutants/ sandbox, which does not copy .github/)",
)

INSTALL_COMMAND = "make install-build-tools"

#: The compiling jobs as of this change. Pinned so a new job that
#: compiles is reviewed even if it also installs the tools: the derived
#: comparison alone would accept a new pairing without anyone deciding it
#: belongs.
EXPECTED_COMPILING_JOBS = frozenset(
    {"build", "lint", "test", "docs-gate", "systemd-gate"}
)


def _workflow() -> dict[object, object]:
    """Return this repository's parsed CI workflow."""
    workflows = repository_workflows()
    assert "ci.yml" in workflows, (
        f"the repository should define ci.yml; found {sorted(workflows)}"
    )
    return workflows["ci.yml"]


def _jobs() -> dict[str, dict[object, object]]:
    """Return the workflow's jobs."""
    jobs = _workflow().get("jobs")
    assert isinstance(jobs, dict), "the CI workflow should define jobs"
    return jobs


def _steps(job: dict[object, object]) -> list[dict[object, object]]:
    """Return one job's steps, in order."""
    steps = job.get("steps", [])
    assert isinstance(steps, list), "a job's steps should be a list"
    return [step for step in steps if isinstance(step, dict)]


def _compiling_jobs() -> set[str]:
    """Return every job whose steps compile or link Rust."""
    return {
        name
        for name, job in _jobs().items()
        if any(run_value_compiles(step.get("run")) for step in _steps(job))
    }


def _install_indexes(job: dict[object, object]) -> list[int]:
    """Return the indexes of steps that install the build tools."""
    return [
        index
        for index, step in enumerate(_steps(job))
        if "install-build-tools" in str(step.get("run", ""))
    ]


def _first_compiling_index(job: dict[object, object]) -> int:
    """Return the index of a job's first compiling step."""
    return next(
        index
        for index, step in enumerate(_steps(job))
        if run_value_compiles(step.get("run"))
    )


def _rust_setup_index(job: dict[object, object]) -> int | None:
    """Return the index of the shared Rust setup step, if any."""
    for index, step in enumerate(_steps(job)):
        if "shared-actions/.github/actions/setup-rust" in str(step.get("uses", "")):
            return index
    return None


def test_the_derived_compiling_jobs_are_the_expected_ones() -> None:
    """Scenario: the derivation stops seeing a job that compiles.

    Invariant: the derivation matches the known set. A contract that
    derives an empty set would pass every other test in this file while
    checking nothing.
    """
    assert _compiling_jobs() == EXPECTED_COMPILING_JOBS, (
        "the derived compiling jobs changed; update the expectation only "
        f"after deciding where the new job installs the tools: "
        f"{sorted(_compiling_jobs())}"
    )


def test_every_compiling_job_installs_before_compiling() -> None:
    """Scenario: a compiling job lacks an install, or installs too late.

    Invariant: every compiling job has exactly one install step, it runs
    after the shared Rust setup, and it precedes the first compiling
    step. Installing after the compile leaves the first build with the
    platform linker; installing before setup is overwritten by the
    action's environment.
    """
    for name in sorted(_compiling_jobs()):
        job = _jobs()[name]
        indexes = _install_indexes(job)
        assert len(indexes) == 1, (
            f"the {name} job should install the build tools exactly once, "
            f"found {len(indexes)}"
        )

        install = indexes[0]
        setup = _rust_setup_index(job)
        assert setup is not None, (
            f"the {name} job compiles but has no shared Rust setup step"
        )
        assert setup < install, (
            f"the {name} job installs at step {install}, before Rust setup at "
            f"step {setup}; the setup would replace the installed environment"
        )

        compiling = _first_compiling_index(job)
        assert install < compiling, (
            f"the {name} job installs at step {install} but compiles at step "
            f"{compiling}; the compile would run without the standard"
        )


def test_only_compiling_jobs_install_the_build_tools() -> None:
    """Scenario: an install is copied into a job that does not compile.

    Invariant: the set of jobs that install equals the derived set of
    compiling jobs. The forward direction is the requirement; the
    reverse keeps an unnecessary download out of spelling and formatting.
    """
    installing = {name for name, job in _jobs().items() if _install_indexes(job)}
    assert installing == _compiling_jobs(), (
        "the jobs that install the build tools should be exactly the jobs "
        f"that compile Rust; installing={sorted(installing)}, "
        f"compiling={sorted(_compiling_jobs())}"
    )


def test_the_install_step_is_unconditional_and_exact() -> None:
    """Scenario: the install step is disabled, wrapped, or rewritten.

    Invariant: the step's whole ``run`` value is the install command and
    the step carries no ``if``. A condition that never holds leaves the
    command visible while the job compiles without the tools.
    """
    for name in sorted(_compiling_jobs()):
        for index in _install_indexes(_jobs()[name]):
            step = _steps(_jobs()[name])[index]
            assert str(step.get("run", "")).strip() == INSTALL_COMMAND, (
                f"the {name} job's install step should run exactly "
                f"{INSTALL_COMMAND!r}, found {step.get('run')!r}"
            )
            assert step.get("if") is None, (
                f"the {name} job's install step carries an if condition "
                f"({step.get('if')!r}); it must run unconditionally"
            )


def test_the_docs_gate_builds_its_helper_through_the_standard() -> None:
    """Scenario: the helper build reverts to a bare Cargo invocation.

    Invariant: the docs-gate job builds ``repovec-ci`` with the Make
    target that composes the standard. A direct ``cargo build`` runs
    under the ``RUSTFLAGS`` that ``setup-rust`` exports, which displaces
    ``.cargo/config.toml`` and silently loses the parallel frontend and
    mold.
    """
    runs = [str(step.get("run", "")).strip() for step in _steps(_jobs()["docs-gate"])]
    assert "make build-docs-gate-helper" in runs, (
        "the docs-gate job should build its helper with "
        f"'make build-docs-gate-helper'; found {runs}"
    )
    bare_cargo = [
        run
        for run in runs
        if any(
            subcommand in COMPILING_CARGO_SUBCOMMANDS
            for subcommand in cargo_subcommands(run)
        )
    ]
    assert not bare_cargo, (
        "the docs-gate job should not compile the workspace with Cargo "
        f"directly, because setup-rust's RUSTFLAGS would displace the "
        f"standard; found {bare_cargo}"
    )


def test_the_derivation_covers_every_compiling_make_target() -> None:
    """Scenario: a standard route is added and CI wiring is not reviewed.

    Invariant: the Make targets named by the derivation are exactly the
    standard routes the Make contract covers. Both sides import the same
    constant, so this fails only if the constant and the expectation are
    changed separately.
    """
    assert COMPILING_MAKE_TARGETS == {
        "build",
        "build-docs-gate-helper",
        "test",
        "lint",
        "typecheck",
        "validate-systemd",
    }, f"the compiling Make targets changed: {sorted(COMPILING_MAKE_TARGETS)}"
    assert "fmt" not in COMPILING_CARGO_SUBCOMMANDS, (
        "the derivation should not treat `cargo fmt` as compiling the "
        f"workspace; found {sorted(COMPILING_CARGO_SUBCOMMANDS)}"
    )
    assert "metadata" not in COMPILING_CARGO_SUBCOMMANDS, (
        "the derivation should not treat `cargo metadata` as compiling the "
        f"workspace; found {sorted(COMPILING_CARGO_SUBCOMMANDS)}"
    )


def test_make_and_cargo_detection_read_run_values() -> None:
    """Scenario: the detectors match prose or comments instead of commands.

    Invariant: the detectors see commands in a ``run`` value and ignore
    comments. These are the functions the job derivation rests on, so
    their behaviour is asserted directly rather than inferred from a
    derived set that could be empty.
    """
    run_value = "\n".join(
        [
            "set -euo pipefail",
            "# make lint is not run here",
            "cargo build --quiet -p repovec-ci",
            "make validate-systemd",
        ]
    )
    assert make_targets(run_value) == ["validate-systemd"], (
        "the Make detector should find only the command's target, not the "
        f"target named in a comment; found {make_targets(run_value)}"
    )
    assert cargo_subcommands(run_value) == ["build"], (
        "the Cargo detector should find only the command's subcommand; "
        f"found {cargo_subcommands(run_value)}"
    )
    assert run_value_compiles(run_value), (
        "a run value invoking `make validate-systemd` and `cargo build` "
        f"should count as compiling; found {run_value!r}"
    )
    assert not run_value_compiles("# make lint is not run here"), (
        "a comment naming a compiling Make target should not count as "
        f"compiling; found {run_value!r}"
    )
    assert not run_value_compiles("make install-build-tools"), (
        "`make install-build-tools` installs tools but does not compile the "
        "workspace, so it should not count as a compiling run value"
    )
