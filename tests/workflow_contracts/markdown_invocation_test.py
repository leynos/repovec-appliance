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
from collections.abc import Callable
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
def test_make_invokes_direct_markdown_tools(
    probes: tuple[Path, dict[str, str]],
    target: str,
    expected: list[list[str]],
) -> None:
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
def test_markdown_rejection_fails_make(
    probes: tuple[Path, dict[str, str]],
    target: str,
    tool: str,
) -> None:
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
        if str(step.get("run", "")).strip() == "make test-workflow-contracts":
            _assert_linter_provisioning(steps, index)
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
def test_installation_contract_detects_workflow_mutations(
    mutate: Callable[[list[dict[str, object]], dict[str, object]], object],
    job_name: str,
) -> None:
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


def test_git_selection_repairs_untracked_but_preserves_ignored_markdown(
    tmp_path: Path,
    probes: tuple[Path, dict[str, str]],
) -> None:
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

NODE_SETUP = "actions/setup-node@249970729cb0ef3589644e2896645e5dc5ba9c38"
LINTER_PATH = "${{ runner.temp }}/repovec-markdownlint/node_modules/.bin/markdownlint-cli2"
LINTER_PROVENANCE = {
    "MARKDOWNLINT_ACTION_REVISION": "21c1be1b93ad9ed58fa840aacc3f279cde2a72ff",
    "MARKDOWNLINT_PACKAGE_SHA256": "c5d29548af9601591933259daac76c09b85d8c6788654c5651a94a26c22b33b7",
    "MARKDOWNLINT_LOCK_SHA256": "9951527515b50ea7aa3d574272bfc8cd2490e1668d99c3316731329f81154a2b",
    "MARKDOWNLINT_CLI_VERSION": "0.23.2",
    "MARKDOWNLINT_CLI_INTEGRITY": "sha512-eUhcnkSpzURo/o4htSqc7LPDszgOOTknhU4eY/sPHvMCLxnTCYscv1gw1/js/idmaZPisv9ECVEIORcllqjTUw==",
}


def _assert_linter_provisioning(steps: list[dict], index: int) -> None:
    """Require locked real-tool provisioning before its integration contract.

    Moving provisioning after the test or changing its release rejects CI.
    """
    ids = {step.get("id"): (i, step) for i, step in enumerate(steps)}
    assert {"setup_markdownlint", "provision_markdownlint"} <= ids.keys(), "missing linter provisioning"
    node_index, node = ids["setup_markdownlint"]
    install_index, install = ids["provision_markdownlint"]
    assert node_index < install_index < index, "linter provisioning order"
    assert node.get("uses") == NODE_SETUP, "linter Node action pin"
    assert _action_input_matches(node, "node-version", "22.23.2"), "linter Node version"
    assert node["with"].get("package-manager-cache") is False, "linter cache must be disabled"
    assert not any("if" in step or step.get("continue-on-error", False) for step in (node, install)), "linter provisioning must fail closed"
    assert install.get("env") == LINTER_PROVENANCE, "linter release provenance"
    script = install.get("run", "")
    assert all(token in script for token in ("set -euo pipefail", "sha256sum --check --strict", "npm ci", "--ignore-scripts", "--omit=dev")), "linter locked installation"
    assert steps[index].get("env", {}).get("MDLINT") == LINTER_PATH, "linter executable route"


@pytest.mark.parametrize(("step_id", "group", "key", "value"), [
    ("setup_markdownlint", None, "uses", "actions/setup-node@main"),
    ("setup_markdownlint", "with", "node-version", "20"),
    ("setup_markdownlint", "with", "package-manager-cache", True),
    ("provision_markdownlint", "env", "MARKDOWNLINT_ACTION_REVISION", "main"),
    ("provision_markdownlint", "env", "MARKDOWNLINT_PACKAGE_SHA256", "0" * 64),
    ("provision_markdownlint", "env", "MARKDOWNLINT_LOCK_SHA256", "0" * 64),
    ("provision_markdownlint", "env", "MARKDOWNLINT_CLI_VERSION", "0.22.1"),
    ("provision_markdownlint", "env", "MARKDOWNLINT_CLI_INTEGRITY", "sha512-invalid"),
    ("provision_markdownlint", None, "continue-on-error", True),
    ("workflow_contracts", "env", "MDLINT", "/unqualified/markdownlint-cli2"),
])
def test_linter_provisioning_detects_mutations(
    step_id: str, group: str | None, key: str, value: str | bool,
) -> None:
    """Parsed CI must reject moving pins, cache, soft failure and wrong tools."""
    workflows = copy.deepcopy(repository_workflows())
    _assert_markdown_ci_contract(workflows)
    steps = workflows["ci.yml"]["jobs"]["lint"]["steps"]
    step = next(step for step in steps if step.get("id") == step_id)
    (step if group is None else step[group])[key] = value
    with pytest.raises(AssertionError, match="linter"):
        _assert_markdown_ci_contract(workflows)


@pytest.mark.parametrize("step_id", ["setup_markdownlint", "provision_markdownlint"])
def test_linter_provisioning_rejects_late_steps(step_id: str) -> None:
    """The actual CI document must reject provisioning moved after tests."""
    workflows = copy.deepcopy(repository_workflows())
    _assert_markdown_ci_contract(workflows)
    steps = workflows["ci.yml"]["jobs"]["lint"]["steps"]
    step = next(step for step in steps if step.get("id") == step_id)
    steps.remove(step)
    steps.append(step)
    with pytest.raises(AssertionError, match="linter"):
        _assert_markdown_ci_contract(workflows)


def test_real_markdownlint_respects_gitignore(tmp_path: Path) -> None:
    """Real pinned linting ignores a defect until identical bytes are exposed."""
    binary = os.environ.get("MDLINT") or shutil.which("markdownlint-cli2")
    assert binary, "provide markdownlint-cli2 0.23.2 through MDLINT"
    subprocess.run(["git", "init", "--quiet", str(tmp_path)], check=True)
    shutil.copyfile(REPOSITORY / ".markdownlint-cli2.jsonc", tmp_path / ".markdownlint-cli2.jsonc")
    (tmp_path / ".gitignore").write_text("ignored.md\n")
    (tmp_path / "README.md").write_text("# Probe\n")
    subprocess.run(["git", "add", ".gitignore", ".markdownlint-cli2.jsonc", "README.md"], cwd=tmp_path, check=True)
    defect = "# Probe\n\n" + " ".join(["word"] * 20) + "\n"
    (tmp_path / "ignored.md").write_text(defect)
    command = [binary, "**/*.md"]
    ignored = subprocess.run(command, cwd=tmp_path, capture_output=True, text=True, check=False)
    assert "markdownlint-cli2 v0.23.2 (markdownlint v0.41.1)" in ignored.stdout, "unqualified linter"
    assert ignored.returncode == 0, ignored.stdout + ignored.stderr
    (tmp_path / "visible.md").write_text(defect)
    visible = subprocess.run(command, cwd=tmp_path, capture_output=True, text=True, check=False)
    assert visible.returncode != 0, "unignored line-length violation was not checked"
    assert "MD013" in visible.stdout + visible.stderr, "expected the real line-length diagnostic"
