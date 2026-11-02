"""Contract tests for fail-closed host detection.

Both build-tools entry points and the Makefile decide whether to use the
pinned Linux-only linker by asking ``uname -s``. A failed ``uname`` must
abort with a diagnosis rather than look like a non-Linux host: the latter
would silently skip the linker and build with the platform default.

These tests put a failing ``uname`` stub ahead of the real tools and drive
the real scripts and Makefile. They assert the exact diagnostic and that no
non-Linux fallback ran.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

from build_standard_probe import REPO_ROOT, SUBPROCESS_TIMEOUT_SECONDS

#: Tools the shared build-tools scripts invoke before they reach the host
#: probe. The failing ``uname`` is added separately, so it is absent here.
REQUIRED_TOOLS = (
    "awk",
    "bash",
    "cat",
    "dirname",
    "env",
    "grep",
    "mkdir",
    "pwd",
    "rm",
    "sed",
    "sh",
)

DIAGNOSTIC = "cannot determine the host operating system: uname -s failed"


def _tool_paths() -> dict[str, str]:
    """Resolve every required tool, skipping when one is unavailable."""
    paths: dict[str, str] = {}
    for tool in REQUIRED_TOOLS:
        resolved = shutil.which(tool)
        if resolved is None:
            pytest.skip(f"{tool} is not available to build the controlled PATH")
        paths[tool] = resolved
    return paths


def _controlled_environment(tmp_path: Path) -> dict[str, str]:
    """Build a PATH whose ``uname`` always fails."""
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    for tool, resolved in _tool_paths().items():
        (bin_dir / tool).symlink_to(resolved)
    uname = bin_dir / "uname"
    uname.write_text("#!/bin/sh\nexit 1\n", encoding="utf-8")
    uname.chmod(0o755)
    (tmp_path / "home").mkdir()
    return {
        "HOME": str(tmp_path / "home"),
        "PATH": f"{bin_dir}:/usr/bin:/bin",
        "TMPDIR": str(tmp_path),
    }


def _run_script(
    script: str, environment: dict[str, str]
) -> subprocess.CompletedProcess[str]:
    """Run one build-tools entry point in the controlled environment."""
    return subprocess.run(
        [str(REPO_ROOT / "scripts" / script)],
        cwd=REPO_ROOT,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
        timeout=SUBPROCESS_TIMEOUT_SECONDS,
    )


@pytest.mark.parametrize("script", ("check-build-tools.sh", "install-build-tools.sh"))
def test_a_failed_uname_aborts_the_entry_points(
    tmp_path: Path, script: str
) -> None:
    """Scenario: ``uname -s`` fails while a build-tools entry point runs.

    Invariant: the script exits non-zero with the host-detection diagnostic
    and does not fall through to the Linux-only skip. A skip would report
    success and leave the pinned linker unselected.
    """
    environment = _controlled_environment(tmp_path)

    completed = _run_script(script, environment)

    output = f"{completed.stdout}\n{completed.stderr}"
    assert completed.returncode != 0, output
    assert DIAGNOSTIC in output, output
    assert "mold is Linux-only" not in output, output


def test_a_failed_uname_aborts_make_at_parse_time(tmp_path: Path) -> None:
    """Scenario: ``uname -s`` fails while Make evaluates the standard.

    Invariant: Make stops with the same diagnostic instead of evaluating an
    empty host OS as non-Linux and selecting the platform linker. The error
    is raised at parse time, so no recipe runs.
    """
    environment = _controlled_environment(tmp_path)
    make = shutil.which("make")
    assert make is not None, "make is required to run this contract"

    completed = subprocess.run(
        [make, "--no-print-directory", "-C", str(REPO_ROOT), "--dry-run", "build"],
        cwd=REPO_ROOT,
        env={**environment, "MAKEFLAGS": "", "MFLAGS": ""},
        capture_output=True,
        text=True,
        check=False,
        timeout=SUBPROCESS_TIMEOUT_SECONDS,
    )

    output = f"{completed.stdout}\n{completed.stderr}"
    assert completed.returncode != 0, output
    assert DIAGNOSTIC in output, output
    assert "mold is Linux-only" not in output, output
