"""Exercise Markdown tool selection, ordering and rejection at local boundaries.

Make probes replace executables, never source recipes. The selection probe uses
an isolated Git repository and the installed mdtablefix binary, so it exercises
untracked and ignored files without rewriting this checkout.
"""

from __future__ import annotations

import copy
import json
import re
import os
import shutil
import subprocess
import sys
from pathlib import Path

import pytest

from workflow_loader import repository_workflows

REPOSITORY = Path(__file__).resolve().parents[2]
SELECTION = ["--git", "--include-untracked"]
RULES = ["--wrap", "--renumber", "--breaks", "--ellipsis", "--fences"]
PROBE = '''#!/usr/bin/env python3
import json
import os
import sys
from pathlib import Path
name = Path(sys.argv[0]).name
with Path(os.environ["MARKDOWN_PROBE_LOG"]).open("a") as log:
    log.write(json.dumps([name, *sys.argv[1:]]) + "\\n")
sys.exit(23 if name == os.environ.get("MARKDOWN_FAIL_TOOL") else 0)
'''


@pytest.fixture
def probes(tmp_path: Path) -> tuple[Path, dict[str, str]]:
    """Prepare executable probes and a child-only environment."""
    directory = tmp_path / "bin"
    directory.mkdir()
    for name in ("cargo", "mdtablefix", "markdownlint", "spelling"):
        executable = directory / name
        executable.write_text(PROBE.replace("/usr/bin/env python3", sys.executable))
        executable.chmod(0o755)
    environment = dict(os.environ)
    environment["MARKDOWN_PROBE_LOG"] = str(tmp_path / "calls.jsonl")
    environment.pop("MARKDOWN_FAIL_TOOL", None)
    # Parent Make invocations must not inject flags or an inherited jobserver.
    environment.pop("MAKEFLAGS", None)
    environment.pop("MFLAGS", None)
    return directory, environment


def _make(
    target: str,
    probes: tuple[Path, dict[str, str]],
    *,
    cwd: Path = REPOSITORY,
    mdtablefix: str | None = None,
) -> tuple[int, list[list[str]]]:
    """Run the real Make target with injectable tools and return its call log.

    For example, ``_make("fmt", probes)`` reports the formatter invocation
    sequence without formatting this repository.
    """
    directory, environment = probes
    result = subprocess.run(
        [
            "make", "--no-print-directory", "--file", str(REPOSITORY / "Makefile"), target,
            f"CARGO={directory / 'cargo'}",
            f"MDTABLEFIX={mdtablefix or directory / 'mdtablefix'}",
            f"MDLINT={directory / 'markdownlint'}",
            f"TYPOS_CONFIG_BUILDER={directory / 'spelling'}",
        ],
        cwd=cwd, env=environment, capture_output=True, text=True, check=False,
    )
    log = Path(environment["MARKDOWN_PROBE_LOG"])
    calls = [json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
    return result.returncode, calls


@pytest.mark.parametrize(
    ("target", "expected"),
    [
        ("fmt", [["mdtablefix", "--in-place", *SELECTION, *RULES],
                 ["markdownlint", "--fix", "**/*.md"]]),
        ("check-fmt", [["mdtablefix", "--check", *SELECTION, *RULES]]),
        ("markdownlint", [["markdownlint", "**/*.md"]]),
    ],
    ids=["make_fmt", "make_check_fmt", "make_markdownlint"],
)
def test_make_invokes_direct_markdown_tools(probes, target, expected) -> None:
    """Formatting fixes in order; formatting checks never receive fix flags."""
    status, calls = _make(target, probes)
    assert status == 0, f"make {target} failed: {calls}"
    markdown_calls = [call for call in calls if call[0] in {"mdtablefix", "markdownlint"}]
    assert markdown_calls == expected


@pytest.mark.parametrize(
    ("target", "tool"),
    [("check-fmt", "mdtablefix"), ("fmt", "mdtablefix"),
     ("fmt", "markdownlint"), ("markdownlint", "markdownlint")],
    ids=["check_rejection", "format_rejection", "fix_lint_rejection", "lint_rejection"],
)
def test_markdown_rejection_fails_make(probes, target, tool) -> None:
    """A failing formatter or linter rejects Make and stops subsequent tools."""
    probes[1]["MARKDOWN_FAIL_TOOL"] = tool
    status, calls = _make(target, probes)
    assert status != 0, f"make {target} discarded {tool}'s rejection"
    assert calls[-1][0] == tool, f"a command ran after {tool} failed: {calls}"


INSTALLER = "leynos/shared-actions/.github/actions/install-mdtablefix"
SELECTED_MDTABLEFIX_VERSION = "0.6.1"


def _action_input_matches(step: dict, name: str, expected: str) -> bool:
    """Compare an action input only when its enclosing value is a mapping.

    For example, a missing, null, scalar or sequence ``with`` value returns
    false, letting the calling contract reject it deliberately.
    """
    inputs = step.get("with")
    return isinstance(inputs, dict) and inputs.get(name) == expected


def _installer_before_check(steps: list[dict], index: int) -> bool:
    """Report pinned provisioning before a selected Markdown consumer.

    ``_installer_before_check([installer, check], 1)`` is true only for an
    unconditional full-SHA installer selecting the qualified binary version.
    """
    return any(
        re.fullmatch(rf"{re.escape(INSTALLER)}@[0-9a-f]{{40}}", str(step.get("uses", "")))
        and _action_input_matches(step, "version", SELECTED_MDTABLEFIX_VERSION)
        and "if" not in step
        and not step.get("continue-on-error", False)
        for step in steps[:index]
    )


def _assert_job_markdown_contract(job: dict) -> int:
    """Validate one job's Markdown steps and count its tool consumers.

    For example, an installer after ``make check-fmt`` raises ``AssertionError``;
    a job without Markdown steps returns zero.
    """
    checks = 0
    steps = job.get("steps", [])
    for index, step in enumerate(steps):
        if str(step.get("run", "")).strip() in {
            "make check-fmt", "make fmt", "make test-workflow-contracts",
        }:
            checks += 1
            assert _installer_before_check(steps, index), (
                "Markdown consumers require an unconditional full-SHA installer "
                f"selecting mdtablefix {SELECTED_MDTABLEFIX_VERSION} first"
            )
        if str(step.get("uses", "")).startswith("DavidAnson/markdownlint-cli2-action@"):
            assert _action_input_matches(step, "globs", "**/*.md"), (
                "markdownlint requires a mapping with globs exactly **/*.md"
            )
    return checks


def _assert_markdown_ci_contract(workflows: dict) -> None:
    """Check each consumer workflow, refusing an empty measurement.

    For example, passing parsed workflow documents with the installer moved
    after ``make check-fmt`` raises ``AssertionError``.
    """
    checks = sum(
        _assert_job_markdown_contract(job)
        for workflow in workflows.values()
        for job in workflow.get("jobs", {}).values()
    )
    assert checks, "no Make Markdown consumer was measured"


def test_ci_installs_before_each_make_markdown_consumer() -> None:
    """Formatting and real-tool contracts each need their own installer."""
    _assert_markdown_ci_contract(repository_workflows())


def _move_installer_last(steps: list[dict], installer: dict) -> None:
    """Move actual provisioning after the formatting step it protects.

    For example, ``[installer, check]`` becomes ``[check, installer]``.
    """
    steps.remove(installer)
    steps.append(installer)


@pytest.mark.parametrize("job_name", ["check-fmt", "lint"])
@pytest.mark.parametrize(
    "mutate",
    [
        pytest.param(lambda steps, installer: steps.remove(installer), id="remove"),
        pytest.param(_move_installer_last, id="after"),
        pytest.param(
            lambda steps, installer: installer.update({"if": "false"}),
            id="conditional",
        ),
        pytest.param(
            lambda steps, installer: installer.update({"continue-on-error": True}),
            id="soft-fail",
        ),
        pytest.param(
            lambda steps, installer: installer["with"].pop("version"),
            id="missing-version",
        ),
        pytest.param(
            lambda steps, installer: installer["with"].update({"version": "0.5.9"}),
            id="incompatible-version",
        ),
        pytest.param(lambda steps, installer: installer.pop("with"), id="missing-inputs"),
        pytest.param(
            lambda steps, installer: installer.update({"uses": INSTALLER}),
            id="missing-pin",
        ),
        pytest.param(
            lambda steps, installer: installer.update({"uses": INSTALLER + "@2ddf19b3"}),
            id="short-pin",
        ),
        pytest.param(
            lambda steps, installer: installer.update({"uses": INSTALLER + "@main"}),
            id="tag-pin",
        ),
    ],
)
def test_installation_contract_detects_workflow_mutations(mutate, job_name) -> None:
    """Mutations of parsed repository CI must fail its real Markdown checker."""
    workflows = copy.deepcopy(repository_workflows())
    _assert_markdown_ci_contract(workflows)
    steps = workflows["ci.yml"]["jobs"][job_name]["steps"]
    installer = next(
        step for step in steps if str(step.get("uses", "")).startswith(INSTALLER + "@")
    )
    mutate(steps, installer)
    with pytest.raises(AssertionError, match="installer"):
        _assert_markdown_ci_contract(workflows)


def test_git_selection_repairs_untracked_but_preserves_ignored_markdown(tmp_path, probes) -> None:
    """An untracked malformed table fails a check, is repaired, then passes."""
    binary = shutil.which("mdtablefix")
    assert binary, "install mdtablefix >=0.6.0 before running Markdown contracts"
    subprocess.run(["git", "init", "--quiet", str(tmp_path)], check=True)
    (tmp_path / ".gitignore").write_text("ignored.md\n")
    (tmp_path / "tracked.md").write_text("# Tracked\n")
    subprocess.run(["git", "add", ".gitignore", "tracked.md"], cwd=tmp_path, check=True)
    defect = "# New document\n\n| A | B |\n|---|---|\n|x|y|\n"
    untracked = tmp_path / "untracked.md"
    ignored = tmp_path / "ignored.md"
    untracked.write_text(defect)
    ignored.write_text(defect)
    before, _ = _make("check-fmt", probes, cwd=tmp_path, mdtablefix=binary)
    assert before != 0, "non-ignored untracked Markdown was not checked"
    repaired, _ = _make("fmt", probes, cwd=tmp_path, mdtablefix=binary)
    assert repaired == 0, "make fmt failed to repair the isolated Markdown table"
    after, _ = _make("check-fmt", probes, cwd=tmp_path, mdtablefix=binary)
    assert after == 0, "make check-fmt rejected the repaired Markdown table"
    assert untracked.read_text() != defect
    assert ignored.read_text() == defect
