"""Contract tests for the nextest timeout budget the coverage lane needs.

``crates/repovec-core/tests/compile_time.rs`` drives ``trybuild``, which
compiles each UI fixture as its own crate. Under ``cargo llvm-cov`` every
compilation unit in that fixture build is instrumented, so the default
nextest budget — 60 s per period, terminated after three periods — expires
while the fixtures are still compiling. Measured on PR #216, run
41348216522: 627 tests passed and this one was killed at 180 s, failing the
``coverage`` job without a diagnostic from the test itself.

The remedy is a narrow override in ``.config/nextest.toml``: a longer budget
for that one test, which asserts compile failures and contributes no
coverage. Skipping it would stop guarding the opaque wrapper types, and a
workspace-wide budget increase would hide a genuine hang in every other test.

Both properties are contractual: the override must still select the test by
its exact name, and its total budget must exceed the default it supersedes.

Mutation proof, recorded 2026-11-01; each applied alone and reverted:

- deleting ``.config/nextest.toml`` fails
  ``test_the_compile_time_fixture_build_has_a_longer_budget``;
- restoring ``terminate-after = 1`` fails the same test, which is the
  measured failing configuration;
- replacing the filter with ``all()`` fails
  ``test_the_override_selects_the_compile_time_test_exactly``;
- renaming the test in the filter fails that same test.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from pathlib import Path

import tomllib

CONFIG_PATH = Path(__file__).resolve().parents[2] / ".config" / "nextest.toml"

#: The ``trybuild`` driver the override exists for.
COMPILE_TIME_TEST = "opaque_secrets_and_observer_bounds_fail_to_compile"

#: Nextest's default: 60 s per period, terminated after three periods.
DEFAULT_BUDGET_SECONDS = 180.0

#: Duration suffixes nextest accepts, in seconds.
DURATION_UNITS = {"ms": 0.001, "s": 1.0, "m": 60.0, "h": 3600.0}


def _overrides() -> list[dict[str, object]]:
    """Return the default profile's overrides in declaration order."""
    config = tomllib.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    return config["profile"]["default"].get("overrides", [])


def _seconds(duration: str) -> float:
    """Return a nextest duration string such as ``"120s"`` in seconds."""
    for suffix, factor in DURATION_UNITS.items():
        if duration.endswith(suffix):
            return float(duration.removesuffix(suffix)) * factor
    raise AssertionError(f"unrecognized duration {duration!r}")


def _budget(override: dict[str, object]) -> float:
    """Return the seconds an override allows before terminating a test."""
    timeout = override["slow-timeout"]
    assert isinstance(timeout, dict), f"slow-timeout should be a table: {timeout!r}"
    return _seconds(str(timeout["period"])) * int(timeout.get("terminate-after", 1))


def _compile_time_overrides() -> list[dict[str, object]]:
    """Return the overrides whose filter names the trybuild test."""
    return [
        override
        for override in _overrides()
        if COMPILE_TIME_TEST in str(override.get("filter", ""))
    ]


def test_the_compile_time_fixture_build_has_a_longer_budget() -> None:
    """Scenario: the instrumented fixture build outruns the default budget.

    Invariant: an override selects the trybuild test and allows longer than
    the 180 s default, so instrumentation cannot turn a passing contract
    into a reported timeout.
    """
    overrides = _compile_time_overrides()

    assert overrides, f"no override in {CONFIG_PATH} names {COMPILE_TIME_TEST}"
    budgets = [_budget(override) for override in overrides]
    assert max(budgets) > DEFAULT_BUDGET_SECONDS, (
        f"the longest budget for {COMPILE_TIME_TEST} is {max(budgets)} s, "
        f"which does not exceed the {DEFAULT_BUDGET_SECONDS} s default"
    )


def test_the_override_selects_the_compile_time_test_exactly() -> None:
    """Scenario: the override is broadened or drifts to another test.

    Invariant: the filter selects the test by an exact-match ``test(=…)``
    expression, so the longer budget cannot silently cover unrelated tests
    or stop covering this one.
    """
    exact = f"test(={COMPILE_TIME_TEST})"
    filters = [
        str(override.get("filter", "")) for override in _compile_time_overrides()
    ]

    assert exact in filters, (
        f"an override must select {exact}; found {filters} in {CONFIG_PATH}"
    )
