"""Contract test for the build standard's failure path.

A prerequisite check that exits zero when the tool is missing is worse
than no check: it turns a clear installation hint into a linker failure
from deep inside Cargo, or, if the fallback linker happens to work, into
a successful build with the wrong tool. The preflight therefore has to
fail, and it has to fail with the command that fixes it.

This test drives ``make build`` in a controlled environment: a PATH
holding only the tools the preflight itself needs, no mold, and an empty
``BUILD_TOOLS_PREFIX``. The pinned toolchain is faked so the failure
isolates the linker half rather than reporting both missing pieces. The
real Cargo is passed explicitly so ``ensure-cargo`` succeeds and the
failure can only come from the preflight.

Linux only: mold is Linux-only by design, and the preflight deliberately
tolerates its absence elsewhere.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import platform
import re
import shutil
import subprocess
from pathlib import Path

import pytest

from build_standard_probe import (
    MOLD_VERSION_PATH,
    REPO_ROOT,
    SUBPROCESS_TIMEOUT_SECONDS,
    run_make_dry_run,
)

pytestmark = pytest.mark.skipif(
    platform.system() != "Linux",
    reason="mold is Linux-only; the preflight tolerates its absence elsewhere",
)

#: Tools the shared build-tools scripts and the Makefile invoke. The
#: preflight must be able to run far enough to report the missing linker,
#: so everything it calls is provided; mold is deliberately not.
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
    "uname",
)

_TOOLCHAIN_RE = re.compile(r'^[ \t]*channel[ \t]*=[ \t]*"([^"]+)"', re.MULTILINE)
_HOST_RE = re.compile(r"^host: (\S+)$", re.MULTILINE)


def _tool_paths() -> dict[str, str]:
    """Resolve every required tool, skipping when one is unavailable."""
    paths: dict[str, str] = {}
    for tool in REQUIRED_TOOLS:
        resolved = shutil.which(tool)
        if resolved is None:
            pytest.skip(f"{tool} is not available to build the controlled PATH")
        paths[tool] = resolved
    return paths


def _pinned_toolchain() -> str:
    """Read the repository's toolchain channel from its pin file."""
    text = (REPO_ROOT / "rust-toolchain.toml").read_text(encoding="utf-8")
    match = _TOOLCHAIN_RE.search(text)
    assert match is not None, "rust-toolchain.toml should declare a channel"
    return match.group(1)


def _host_triple() -> str:
    """Return the host triple the installed Rust compiler targets."""
    rustc = shutil.which("rustc")
    if rustc is not None:
        completed = subprocess.run(
            [rustc, "-vV"],
            capture_output=True,
            text=True,
            check=False,
            timeout=SUBPROCESS_TIMEOUT_SECONDS,
        )
        match = _HOST_RE.search(completed.stdout)
        if completed.returncode == 0 and match is not None:
            return match.group(1)
    pytest.skip("cannot resolve the host triple to fake the toolchain identity")


def _controlled_environment(
    tmp_path: Path, *, toolchain_identity: str | None = None
) -> tuple[dict[str, str], str]:
    """Build the isolated environment and return it with the make path."""
    bin_dir = tmp_path / "bin"
    bin_dir.mkdir()
    for tool, resolved in _tool_paths().items():
        (bin_dir / tool).symlink_to(resolved)

    # Fake only the toolchain lookup. The preflight must reach the mold
    # check and fail there; a real rustup would make both halves fail and
    # the assertion could pass on the wrong message. The fake reports the
    # complete installed identity, and the host triple separately, exactly
    # as rustup does.
    host = _host_triple()
    identity = toolchain_identity or f"{_pinned_toolchain()}-{host}"
    rustup = bin_dir / "rustup"
    rustup.write_text(
        "#!/bin/sh\n"
        'if [ "$1" = toolchain ] && [ "$2" = list ]; then\n'
        f"  printf '%s (active)\\n' \"{identity}\"\n"
        "  exit 0\n"
        "fi\n"
        'if [ "$1" = show ]; then\n'
        f"  printf 'Default host: %s\\n' \"{host}\"\n"
        "  exit 0\n"
        "fi\n"
        "exit 1\n",
        encoding="utf-8",
    )
    rustup.chmod(0o755)

    make = shutil.which("make")
    assert make is not None, "make is required to run this contract"

    # A Cargo that succeeds if it is ever reached. The sentinel file is
    # what proves the preflight failed *before* compiling: a recipe that
    # ignored the preflight's status would run this and exit zero.
    cargo = tmp_path / "cargo-sentinel"
    cargo.write_text(
        '#!/bin/sh\n: > "$CARGO_MARKER"\nexit 0\n',
        encoding="utf-8",
    )
    cargo.chmod(0o755)
    marker = tmp_path / "cargo-ran"

    environment = {
        "HOME": str(tmp_path / "home"),
        "PATH": str(bin_dir),
        "TMPDIR": str(tmp_path),
        "CARGO": str(cargo),
        "CARGO_MARKER": str(marker),
        "BUILD_TOOLS_PREFIX": str(tmp_path / "prefix"),
    }
    (tmp_path / "home").mkdir()
    (tmp_path / "prefix").mkdir()
    return environment, make


def test_the_preflight_fails_with_the_install_command(tmp_path: Path) -> None:
    """Scenario: mold is absent and a developer runs a standard route.

    Invariant: the build stops before compiling, exits non-zero, and names
    ``make install-build-tools``. The hint is the actionable part: a bare
    linker error sends the developer to Cargo's documentation instead.
    """
    environment, make = _controlled_environment(tmp_path)
    marker = Path(environment["CARGO_MARKER"])
    completed = subprocess.run(
        [
            make,
            "--no-print-directory",
            "-C",
            str(REPO_ROOT),
            "build",
            f"CARGO={environment['CARGO']}",
            f"BUILD_TOOLS_PREFIX={environment['BUILD_TOOLS_PREFIX']}",
        ],
        cwd=REPO_ROOT,
        env={**environment, "MAKEFLAGS": "", "MFLAGS": ""},
        capture_output=True,
        text=True,
        check=False,
        timeout=SUBPROCESS_TIMEOUT_SECONDS,
    )

    output = f"{completed.stdout}\n{completed.stderr}"
    assert completed.returncode != 0, (
        f"the preflight should fail when mold is absent:\n{output}"
    )
    assert "mold not found on PATH" in output, (
        f"the failure should name the missing linker:\n{output}"
    )
    assert "make install-build-tools" in output, (
        f"the failure should name the remediation:\n{output}"
    )
    assert not marker.exists(), (
        "the build reached Cargo even though the preflight failed; the "
        f"preflight's status was discarded:\n{output}"
    )


def test_the_preflight_passes_with_a_matching_mold(tmp_path: Path) -> None:
    """Scenario: a matching fake mold satisfies the real preflight script.

    Invariant: the failure above is caused by mold's absence, not by the
    controlled PATH being unable to run the preflight at all. The real
    ``scripts/check-build-tools.sh`` runs here: it executes the fake mold,
    compares the version it reports with the pin in ``tools/mold/VERSION``,
    and passes only when the two agree, which isolates the previous test's
    cause to mold's absence.

    The fake mold reports the pinned version read from that file, so a pin
    bump updates the fixture with the repository rather than silently
    leaving a hard-coded version behind.
    """
    environment, _ = _controlled_environment(tmp_path)
    pinned_version = MOLD_VERSION_PATH.read_text(encoding="utf-8").strip()
    # The real check runs ``mold --version``; provide a matching version
    # so the preflight's own logic is what is under test.
    fake_mold = Path(environment["PATH"]) / "mold"
    fake_mold.write_text(
        f"#!/bin/sh\nprintf 'mold %s (compatible with GNU ld)\\n' '{pinned_version}'\n",
        encoding="utf-8",
    )
    fake_mold.chmod(0o755)

    completed = subprocess.run(
        [
            "scripts/check-build-tools.sh",
        ],
        cwd=REPO_ROOT,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
        timeout=SUBPROCESS_TIMEOUT_SECONDS,
    )
    output = f"{completed.stdout}\n{completed.stderr}"
    assert completed.returncode == 0, (
        f"the preflight should pass with a matching mold and toolchain:\n{output}"
    )
    assert f"mold {pinned_version}" in output, output


def test_a_toolchain_that_only_extends_the_pin_is_not_accepted(tmp_path: Path) -> None:
    """Scenario: a custom toolchain's name starts with the pinned channel.

    Invariant: the check matches the complete installed identity, not a
    prefix. A toolchain such as ``nightly-2026-03-12-custom`` must not
    satisfy a pin that names the host build, because the parallel
    frontend would then come from an unvetted compiler.
    """
    prefix_only = f"{_pinned_toolchain()}-custom"
    environment, _ = _controlled_environment(tmp_path, toolchain_identity=prefix_only)

    completed = subprocess.run(
        [
            "scripts/check-build-tools.sh",
        ],
        cwd=REPO_ROOT,
        env=environment,
        capture_output=True,
        text=True,
        check=False,
        timeout=SUBPROCESS_TIMEOUT_SECONDS,
    )
    output = f"{completed.stdout}\n{completed.stderr}"
    assert completed.returncode != 0, (
        f"the preflight should reject a prefix-only toolchain match:\n{output}"
    )
    assert "is not installed" in output, (
        f"the failure should name the missing toolchain identity:\n{output}"
    )


def test_the_standard_routes_still_dry_run_cleanly() -> None:
    """Scenario: the preflight contract's environment leaks into others.

    Invariant: with the ordinary environment, every standard route still
    expands without Make rejecting it. This is the control for the
    isolated failure above: a broken Makefile would otherwise make the
    failure test pass for the wrong reason.
    """
    for target in (
        "build",
        "build-docs-gate-helper",
        "test",
        "lint",
        "typecheck",
        "validate-systemd",
    ):
        lines = run_make_dry_run(target)
        assert lines, f"the {target} dry run should print a recipe"
