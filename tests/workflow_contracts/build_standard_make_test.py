"""Contract tests for the Make routes that use the development standard.

Make is where the standard is restated for the routes CI runs. It has to
be restated: ``setup-rust`` exports ``RUSTFLAGS`` for the whole CI job,
and an assigned ``RUSTFLAGS`` displaces every ``rustflags`` table in
``.cargo/config.toml``. A route that assigned its own ``RUSTFLAGS``
without repeating the standard would build with the platform linker and
a single-threaded frontend while still reporting success.

The contracts evaluate ``make --dry-run`` output. Make expands its
variables before printing a recipe, so the line read here is the line a
real run would execute; checking the Makefile text would accept
``STANDARD_RUSTFLAGS`` being defined and never used. The probe sets
``RUSTFLAGS='-D warnings'``, mirroring CI, so every standard route's
evaluated line must carry both the warning denial and the standard's
flags.

``test`` does not run Cargo directly: it hands the flags to
``scripts/run_rust_tests.py``, which owns the gate sequencing. The
contract follows the flags into the runner's ``--rust-flags`` and
checks the doctest routing in the same pass, because a runner that
receives the flags but not the doctest flags still skips a gate.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import shlex

import pytest

from build_standard_probe import (
    CI_RUSTFLAGS,
    MOLD_FLAG,
    PROBE_CARGO,
    THREADS_FLAG,
    WARNING_TOKENS,
    cargo_invocations,
    expected_standard_tokens,
    first_compiling_index,
    option_value,
    preflight_index,
    run_make_dry_run,
    runner_invocations,
)

#: The routes the development standard covers, as Make target names.
STANDARD_ROUTES = (
    "build",
    "build-docs-gate-helper",
    "test",
    "lint",
    "typecheck",
    "validate-systemd",
)

#: Routes that must not depend on the build tools being installed.
TOOL_FREE_ROUTES = ("release", "fmt", "check-fmt")


def _route_flag_tokens(target: str) -> list[list[str]]:
    """Return the flag tokens each compiling invocation receives.

    Direct Cargo routes pass the standard through ``RUSTFLAGS``. The
    ``test`` route passes the same string to the runner, which exports it
    for its own Cargo children, so the value is read from
    ``--rust-flags`` instead.
    """
    lines = run_make_dry_run(target)
    if target == "test":
        runners = runner_invocations(lines)
        assert len(runners) == 1, (
            f"the test dry run should invoke the runner once, found {len(runners)}: "
            f"{[runner.text for runner in runners]}"
        )
        rust_flags = option_value(runners[0], "rust-flags")
        assert rust_flags is not None, (
            "the test route should pass --rust-flags to the runner"
        )
        return [shlex.split(rust_flags)]

    invocations = cargo_invocations(lines)
    assert invocations, f"the {target} dry run should invoke Cargo: {lines}"
    return [
        invocation.environment_flag_tokens("RUSTFLAGS") for invocation in invocations
    ]


@pytest.mark.parametrize("target", STANDARD_ROUTES)
def test_the_preflight_runs_before_any_compilation(target: str) -> None:
    """Scenario: a route compiles before checking its prerequisites.

    Invariant: ``check-build-tools`` runs earlier in the evaluated recipe
    order than the first compiling invocation. Otherwise a missing
    linker surfaces as an opaque failure from inside Cargo, after the
    build has already spent time on unrelated work.
    """
    lines = run_make_dry_run(target)
    check = preflight_index(lines)
    assert check is not None, (
        f"the {target} dry run never runs check-build-tools: {lines}"
    )
    compiling = first_compiling_index(lines)
    assert check < compiling, (
        f"the {target} route compiles at line {compiling} before the "
        f"preflight at line {check}: {lines}"
    )


@pytest.mark.parametrize("target", STANDARD_ROUTES)
def test_the_route_carries_the_warning_denial_and_the_standard(target: str) -> None:
    """Scenario: a route loses a standard flag or the warning denial.

    Invariant: every compiling invocation in the route receives
    ``-D warnings`` and the host's standard flags. The probe environment
    supplies the warning denial exactly as CI's ``setup-rust`` does, so a
    route that fails to append to it loses the standard; a route that
    replaces it loses the denial.
    """
    expected = [*WARNING_TOKENS, *expected_standard_tokens()]
    flag_tokens = _route_flag_tokens(target)

    assert flag_tokens, f"the {target} route should pass flags to Cargo"
    for tokens in flag_tokens:
        for flag in expected:
            assert flag in tokens, (
                f"the {target} route's flags {tokens!r} omit {flag!r}; expected "
                f"{expected!r}"
            )


def test_the_standard_survives_with_no_inherited_flags() -> None:
    """Scenario: an interactive Make run inherits no ``RUSTFLAGS``.

    Invariant: the standard is present even though the caller set no
    flags. The Make recipe still assigns ``RUSTFLAGS`` for its own
    targets, so the assignment must carry the standard rather than rely
    on the configuration file that the assignment displaces.
    """
    lines = run_make_dry_run("build", rustflags=None)
    invocations = cargo_invocations(lines)
    assert len(invocations) == 1, f"expected one Cargo invocation, found {invocations}"

    tokens = invocations[0].environment_flag_tokens("RUSTFLAGS")
    for flag in expected_standard_tokens():
        assert flag in tokens, (
            f"the build RUSTFLAGS {tokens!r} omit {flag!r} when the caller "
            "inherited nothing"
        )


def test_inherited_metacharacters_reach_the_compiler_verbatim() -> None:
    """Scenario: an inherited ``RUSTFLAGS`` carries shell metacharacters.

    Invariant: the caller's flag text arrives unchanged. A double-quoted
    assignment lets the shell re-parse an embedded ``"``, backtick or
    ``$``, and expanding ``$(RUSTFLAGS)`` lets Make consume a literal
    ``$`` as a variable reference before the shell ever sees it.
    """
    hostile = '-D warnings -Copt="a b" -Cvar=$HOME `date` it\'s'
    lines = run_make_dry_run("build", rustflags=hostile)
    invocations = cargo_invocations(lines)
    assert len(invocations) == 1, f"expected one Cargo invocation, found {invocations}"

    expected = f"{hostile} {' '.join(expected_standard_tokens())}"
    assert invocations[0].environment["RUSTFLAGS"] == expected, (
        "the inherited RUSTFLAGS should reach Cargo verbatim; "
        f"expected {expected!r}, found {invocations[0].environment['RUSTFLAGS']!r}"
    )


def test_the_test_route_routes_both_gate_flag_groups_to_the_runner() -> None:
    """Scenario: the runner is invoked without one of its flag groups.

    Invariant: the ``test`` recipe passes the evaluated Rust flags, the
    unit-test flags, and the doctest flags to the runner. A runner
    missing ``--doctest-flags`` still runs the unit tests and reports
    success, so the doctest gate would disappear without a failure.
    """
    lines = run_make_dry_run("test")
    runners = runner_invocations(lines)
    assert len(runners) == 1, f"expected one runner invocation, found {runners}"
    runner = runners[0]

    assert option_value(runner, "cargo") == PROBE_CARGO
    rust_flags = option_value(runner, "rust-flags")
    assert rust_flags is not None
    for flag in [*WARNING_TOKENS, *expected_standard_tokens()]:
        assert flag in shlex.split(rust_flags), (
            f"the runner's --rust-flags {rust_flags!r} omit {flag!r}"
        )
    assert option_value(runner, "test-flags") == "--all-targets --all-features"
    assert option_value(runner, "doctest-flags") == "--workspace --all-features"
    assert option_value(runner, "build-jobs") == ""


def test_release_displaces_the_configuration_without_the_standard() -> None:
    """Scenario: release builds inherit the development linker or frontend.

    Invariant: the release recipe assigns ``RUSTFLAGS``, which displaces
    the configuration file's tables, and the assigned value carries
    neither the parallel frontend nor mold. It keeps only what the caller
    inherited, so CI's denial survives.
    """
    lines = run_make_dry_run("release", rustflags=CI_RUSTFLAGS)
    invocations = cargo_invocations(lines)
    assert len(invocations) == 1, (
        f"the release dry run should invoke Cargo once, found {invocations}"
    )

    invocation = invocations[0]
    assert "RUSTFLAGS" in invocation.environment, (
        "the release recipe should assign RUSTFLAGS to displace the "
        f"configuration file: {invocation.text}"
    )
    tokens = invocation.environment_flag_tokens("RUSTFLAGS")
    assert tokens == list(WARNING_TOKENS), (
        "the release RUSTFLAGS should be exactly "
        f"{list(WARNING_TOKENS)!r}, found {tokens!r}"
    )
    for flag in (THREADS_FLAG, MOLD_FLAG, "-Clink-arg"):
        assert flag not in tokens, (
            f"the release route should not carry {flag!r}; found {tokens!r}"
        )


def test_release_still_displaces_the_configuration_with_no_inherited_flags() -> None:
    """Scenario: an interactive release build inherits no ``RUSTFLAGS``.

    Invariant: the recipe still assigns the variable, even to an empty
    value. Cargo treats an empty assignment as a source and stops reading
    the configuration file's tables; omitting the assignment would let
    the development standard leak into a release build.
    """
    lines = run_make_dry_run("release", rustflags=None)
    invocations = cargo_invocations(lines)
    assert len(invocations) == 1, (
        f"the release dry run should invoke Cargo once, found {invocations}"
    )

    invocation = invocations[0]
    assert "RUSTFLAGS" in invocation.environment, (
        "the release recipe should assign RUSTFLAGS even when the caller set "
        f"none: {invocation.text}"
    )
    assert invocation.environment["RUSTFLAGS"] == "", (
        "the release RUSTFLAGS should be empty when nothing was inherited, "
        f"found {invocation.environment['RUSTFLAGS']!r}"
    )


@pytest.mark.parametrize("target", TOOL_FREE_ROUTES)
def test_the_route_does_not_require_the_build_tools(target: str) -> None:
    """Scenario: formatting or release starts demanding mold.

    Invariant: these routes never run the preflight. A release packager
    has no mold, and formatting does not link; requiring the tools would
    make either route unusable exactly where the standard does not apply.
    """
    lines = run_make_dry_run(target, rustflags=None)
    assert preflight_index(lines) is None, (
        f"the {target} route should not run check-build-tools: {lines}"
    )
