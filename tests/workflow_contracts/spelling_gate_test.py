"""Exercise the repository's all-source spelling gate and precise exemptions.

These contracts drive Make and the pinned builder instead of recreating the
estate auditor or inspecting generated regular expressions. They run through
``make test-workflow-contracts`` in fresh CI as well as developer checkouts.
"""

from __future__ import annotations

import json
import shutil
import subprocess
from pathlib import Path
from typing import TypedDict

import pytest
from spelling_contract_helpers import (
    BUILDER_SOURCE,
    CACHE_FILES,
    MAKEFILE,
    ROOT,
    initialize_probe,
    input_hashes,
    recorded_arguments,
    run_spelling,
    spelling_tool_environment,
    write_offline_builder,
)


class PrecisionCase(TypedDict):
    """One authored input and its expected gate verdict and diagnostic."""

    case_id: str
    filename: str
    text: str
    expected_exit: int
    diagnostic: str | None


PRECISION_FIXTURE = ROOT / "tests/workflow_contracts/fixtures/spelling_precision_cases.json"
PRECISION_CASES: list[PrecisionCase] = json.loads(PRECISION_FIXTURE.read_text(encoding="utf-8"))


def assert_builder_arguments(arguments: list[str]) -> None:
    """Assert the evaluated consumer contract, including its release and scope."""
    assert arguments == [
        "tool", "run", "--python", "3.14", "--from", BUILDER_SOURCE,
        "typos-config-builder", "gate", "--repository", ".", "--scope", "all",
    ], "The evaluated spelling recipe must invoke the pinned all-source gate"


def test_spelling_invokes_the_pinned_all_source_gate(tmp_path: Path) -> None:
    """The actual Make recipe selects every tracked file through the release."""
    arguments, result = recorded_arguments(tmp_path)
    assert result.returncode == 0, result.stdout
    assert_builder_arguments(arguments)


def test_missing_source_scope_is_detected_by_the_invocation_contract(tmp_path: Path) -> None:
    """A mutated recipe reverting to the builder default fails the same check."""
    original = MAKEFILE.read_text(encoding="utf-8")
    mutation = original.replace("gate --repository . --scope all", "gate --repository .")
    assert mutation != original, "The intended mutation must reach the real recipe"
    makefile = tmp_path / "Makefile"
    makefile.write_text(mutation, encoding="utf-8")
    arguments, result = recorded_arguments(tmp_path, makefile=makefile)
    assert result.returncode == 0, result.stdout
    with pytest.raises(AssertionError, match="pinned all-source gate"):
        assert_builder_arguments(arguments)


def test_builder_failure_rejects_the_enclosing_make_target(tmp_path: Path) -> None:
    """A controlled rejection remains nonzero through the actual Make boundary."""
    arguments, result = recorded_arguments(tmp_path, exit_code=19)
    assert_builder_arguments(arguments)
    assert result.returncode != 0, result.stdout
    assert "Error 19" in result.stdout


@pytest.fixture(scope="module")
def observed_spelling_inputs(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Refresh the live dictionary once with the real pinned production route."""
    directory = tmp_path_factory.mktemp("spelling-live-inputs") / "repository"
    bootstrap = PRECISION_CASES[0]
    initialize_probe(directory, bootstrap["filename"], bootstrap["text"])
    result = run_spelling(
        directory, overrides=("UV_ENV=",),
        environment=spelling_tool_environment(directory),
    )
    if result.returncode != 0:
        raise RuntimeError(f"Live spelling gate could not establish its inputs:\n{result.stdout}")
    (directory.parent / "observed-inputs.json").write_text(
        json.dumps(input_hashes(directory), indent=2) + "\n", encoding="utf-8"
    )
    return directory


@pytest.mark.parametrize(
    "case", PRECISION_CASES,
    ids=[case["case_id"] for case in PRECISION_CASES],
)
def test_real_make_gate_preserves_only_intended_external_spellings(
    tmp_path: Path,
    observed_spelling_inputs: Path,
    record_property,
    case: PrecisionCase,
) -> None:
    """Replay the observed dictionary while UV resolves the pinned release."""
    directory = tmp_path / "repository"
    initialize_probe(directory, case["filename"], case["text"])
    for cache in CACHE_FILES:
        shutil.copyfile(observed_spelling_inputs / cache, directory / cache)
    before = input_hashes(directory)
    record_property("shared_dictionary_sha256", before[CACHE_FILES[0]])
    wrapper = write_offline_builder(tmp_path)
    result = run_spelling(
        directory, overrides=("UV_ENV=", f"TYPOS_CONFIG_BUILDER={wrapper}"),
        environment=spelling_tool_environment(observed_spelling_inputs),
    )
    assert result.returncode == case["expected_exit"], result.stdout
    assert f"spelling-builder-exit={case['expected_exit']}" in result.stdout
    assert f"spelling-uv-cache={observed_spelling_inputs / '.uv-cache'}" in result.stdout
    assert f"spelling-uv-tools={observed_spelling_inputs / '.uv-tools'}" in result.stdout
    assert "spelling-uv-offline=0" in result.stdout
    if case["diagnostic"] is not None:
        assert case["diagnostic"] in result.stdout and case["filename"] in result.stdout
    assert input_hashes(directory) == before, "Offline case changed its measured inputs"


def test_authored_fixture_exclusion_does_not_exempt_ordinary_source(
    tmp_path: Path, observed_spelling_inputs: Path,
) -> None:
    """Only the exact authored data path is exempt; materialized source fails."""
    directory = tmp_path / "repository"
    fixture_name = str(PRECISION_FIXTURE.relative_to(ROOT))
    initialize_probe(directory, fixture_name, PRECISION_FIXTURE.read_text(encoding="utf-8"))
    for cache in CACHE_FILES:
        shutil.copyfile(observed_spelling_inputs / cache, directory / cache)
    wrapper = write_offline_builder(tmp_path)
    environment = spelling_tool_environment(observed_spelling_inputs)
    overrides = ("UV_ENV=", f"TYPOS_CONFIG_BUILDER={wrapper}")
    excluded_result = run_spelling(directory, overrides=overrides, environment=environment)
    assert excluded_result.returncode == 0, excluded_result.stdout
    source_case = next(
        case for case in PRECISION_CASES if case["case_id"] == "ordinary-colour-source"
    )
    source_path = directory / source_case["filename"]
    source_path.write_text(source_case["text"], encoding="utf-8")
    subprocess.run(
        ["git", "add", "--", source_case["filename"]], cwd=directory,
        env=environment, check=True,
    )
    source_result = run_spelling(directory, overrides=overrides, environment=environment)
    assert source_result.returncode == source_case["expected_exit"], source_result.stdout
    assert source_case["diagnostic"] in source_result.stdout
    assert source_case["filename"] in source_result.stdout
