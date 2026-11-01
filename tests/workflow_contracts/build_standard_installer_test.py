"""Hermetic coverage for the build-tools installer.

``scripts/install-build-tools.sh`` downloads a pinned mold release, verifies
it against ``tools/mold/SHA256SUMS``, unpacks it under a prefix, and installs
the pinned toolchain. Nothing in that list is safe to exercise against the
network or a developer's real toolchain, so these tests drive the real
script with the real ``sha256sum`` and ``tar`` over a tarball the test
builds, and put stubs for ``curl``, ``uname`` and ``rustup`` on PATH.

Covering the real checksum and extraction steps is deliberate: a stub
``sha256sum`` would accept any digest, and a stub ``tar`` would accept any
archive layout, so the two properties the installer exists to guarantee --
that a wrong artefact is refused and that the archive's root is stripped --
would go untested.

The installer's scratch directory is reported through ``TMPDIR``, which
each test points at an otherwise empty directory. That makes cleanup
observable: a successful run and a rejected run must both leave nothing
behind.
"""

from __future__ import annotations

import hashlib
import io
import os
import shutil
import subprocess
import tarfile
from dataclasses import dataclass
from pathlib import Path
from typing import Final

import pytest

REPO_ROOT: Final = Path(__file__).resolve().parents[2]
INSTALLER_PATH: Final = REPO_ROOT / "scripts" / "install-build-tools.sh"
BASH: Final = shutil.which("bash") or "/usr/bin/bash"

MOLD_VERSION: Final = "9.9.9"
TOOLCHAIN: Final = "nightly-2026-11-01"
ARCHIVE_ROOT: Final = f"mold-{MOLD_VERSION}-x86_64-linux"
ARCHIVE_NAME: Final = f"{ARCHIVE_ROOT}.tar.gz"

#: The payload the test puts at the archive's ``bin/mold``. Its content is
#: asserted after extraction, so a run that unpacks nothing, or unpacks to
#: the wrong depth, fails here rather than passing on an empty prefix.
MOLD_PAYLOAD: Final = b"#!/bin/sh\necho 'mold 9.9.9 (fake)'\n"


@dataclass(frozen=True)
class Installer:
    """One prepared hermetic installer environment.

    Attributes
    ----------
    root
        The test's private directory; every other path lives below it.
    prefix
        The ``BUILD_TOOLS_PREFIX`` the installer unpacks into.
    scratch
        The ``TMPDIR`` the installer's ``mktemp -d`` must clean up.
    log
        Where the stubs record the invocations they receive.
    """

    root: Path
    prefix: Path
    scratch: Path
    log: Path

    def environment(self) -> dict[str, str]:
        """Return the child environment, pinned to stubs and test paths."""
        environment = os.environ.copy()
        environment.update(
            {
                "PATH": f"{self.root / 'stub'}:/usr/bin:/bin",
                "HOME": str(self.root / "home"),
                "TMPDIR": str(self.scratch),
                "BUILD_TOOLS_PREFIX": str(self.prefix),
                "MOLD_VERSION_FILE": str(self.root / "VERSION"),
                "MOLD_SHA256SUMS_FILE": str(self.root / "SHA256SUMS"),
                "RUST_TOOLCHAIN_FILE": str(self.root / "rust-toolchain.toml"),
                "MOLD_RELEASE_BASE_URL": "https://mold.example.invalid/releases",
                "CURL_CONNECT_TIMEOUT": "1",
                "CURL_STALL_SECONDS": "1",
                "CURL_RETRIES": "0",
                "CURL_RETRY_DELAY_SECONDS": "0",
                "CURL_RETRY_MAX_SECONDS": "1",
                "STUB_TARBALL": str(self.root / ARCHIVE_NAME),
                "STUB_LOG": str(self.log),
            }
        )
        return environment

    def run(self) -> subprocess.CompletedProcess[str]:
        """Run the installer and return its completed process."""
        return subprocess.run(
            [BASH, str(INSTALLER_PATH)],
            cwd=self.root,
            env=self.environment(),
            capture_output=True,
            text=True,
            check=False,
        )

    def recorded(self) -> list[str]:
        """Return the stub invocations recorded so far."""
        if not self.log.exists():
            return []
        return self.log.read_text(encoding="utf-8").splitlines()


def write_stub(path: Path, body: str) -> None:
    """Write one executable stub script."""
    path.write_text(f"#!/usr/bin/env bash\nset -euo pipefail\n{body}", encoding="utf-8")
    path.chmod(0o755)


def build_archive(path: Path) -> str:
    """Write the release tarball and return its digest.

    The archive mirrors the real release's layout: a single versioned root
    holding ``bin/mold``. The installer must strip that root, so the
    extracted path is ``<prefix>/bin/mold``.
    """
    with tarfile.open(path, "w:gz") as archive:
        member = tarfile.TarInfo(f"{ARCHIVE_ROOT}/bin/mold")
        member.size = len(MOLD_PAYLOAD)
        member.mode = 0o755
        archive.addfile(member, io.BytesIO(MOLD_PAYLOAD))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def install_stubs(root: Path) -> None:
    """Put the three stubs the installer calls on PATH.

    ``curl`` copies a prebuilt archive instead of downloading it, so the
    checksum and extraction steps still run against real tools. ``uname``
    reports Linux x86_64 regardless of the host, which keeps the chosen
    archive name deterministic. ``rustup`` records its arguments and
    succeeds, since installing a real toolchain is the one step that must
    not happen here.
    """
    stub = root / "stub"
    stub.mkdir()
    write_stub(
        stub / "curl",
        """out=
while [ "$#" -gt 0 ]; do
  case $1 in
    --output) out=$2; shift 2 ;;
    *) shift ;;
  esac
done
printf 'curl %s\\n' "$*" >> "$STUB_LOG"
cp -- "$STUB_TARBALL" "$out"
""",
    )
    write_stub(
        stub / "uname",
        """case ${1:-} in
  -m) printf 'x86_64\\n' ;;
  *) printf 'Linux\\n' ;;
esac
""",
    )
    write_stub(stub / "rustup", 'printf \'rustup %s\\n\' "$*" >> "$STUB_LOG"\n')


def build_installer(tmp_path: Path) -> Installer:
    """Seed every file the installer reads and return the environment."""
    prefix = tmp_path / "prefix"
    scratch = tmp_path / "scratch"
    scratch.mkdir()
    (tmp_path / "home").mkdir()

    digest = build_archive(tmp_path / ARCHIVE_NAME)
    (tmp_path / "VERSION").write_text(f"{MOLD_VERSION}\n", encoding="utf-8")
    (tmp_path / "SHA256SUMS").write_text(
        f"{digest}  {ARCHIVE_NAME}\n", encoding="utf-8"
    )
    (tmp_path / "rust-toolchain.toml").write_text(
        f'[toolchain]\nchannel = "{TOOLCHAIN}"\n', encoding="utf-8"
    )
    install_stubs(tmp_path)

    return Installer(
        root=tmp_path, prefix=prefix, scratch=scratch, log=tmp_path / "log"
    )


@pytest.fixture
def installer(tmp_path: Path) -> Installer:
    """A prepared installer environment for one test."""
    return build_installer(tmp_path)


def test_installs_the_pinned_mold_release(installer: Installer) -> None:
    """Scenario: the pinned release downloads and verifies.

    Invariant: the archive's single root is stripped, so ``bin/mold``
    lands directly under the prefix with the archive's own content. A run
    that unpacked the whole tree would put the binary one level deeper and
    leave ``mold`` off PATH.
    """
    completed = installer.run()

    assert completed.returncode == 0, completed.stderr
    extracted = installer.prefix / "bin" / "mold"
    assert extracted.is_file(), (
        f"expected {extracted}, found {list(installer.prefix.rglob('*'))}"
    )
    assert extracted.read_bytes() == MOLD_PAYLOAD
    assert not (installer.prefix / ARCHIVE_ROOT).exists(), (
        "the archive root should be stripped, not preserved"
    )
    assert f"verified {ARCHIVE_NAME}" in completed.stderr, completed.stderr
    assert f"installed mold {MOLD_VERSION} into {installer.prefix}" in completed.stderr


def test_installs_the_pinned_toolchain(installer: Installer) -> None:
    """Scenario: the toolchain pin is read and installed.

    Invariant: ``rustup`` receives the channel named in
    ``rust-toolchain.toml`` with the minimal profile. Installing any other
    channel would leave the gate running on a toolchain the repository
    does not select.
    """
    completed = installer.run()

    assert completed.returncode == 0, completed.stderr
    assert (
        f"rustup toolchain install {TOOLCHAIN} --profile minimal"
        in installer.recorded()
    )


def tamper_with_checksums(installer: Installer, case: str) -> None:
    """Make the recorded checksums unusable in the named way."""
    sums = installer.root / "SHA256SUMS"
    if case == "mismatch":
        sums.write_text(f"{'0' * 64}  {ARCHIVE_NAME}\n", encoding="utf-8")
    else:
        sums.write_text(f"{'0' * 64}  another-archive.tar.gz\n", encoding="utf-8")


@pytest.mark.parametrize(
    ("case", "expected"),
    (
        ("mismatch", f"checksum mismatch for {ARCHIVE_NAME}"),
        ("unlisted", f"no checksum recorded for {ARCHIVE_NAME}"),
    ),
)
def test_rejects_an_unverifiable_archive(
    installer: Installer, case: str, expected: str
) -> None:
    """Scenario: the download does not match its recorded digest.

    Invariant: the installer fails before unpacking anything and before
    spending time on the toolchain. A wrong artefact must never reach the
    prefix, and a rejected run must not install a toolchain as a partial
    success.
    """
    tamper_with_checksums(installer, case)

    completed = installer.run()

    assert completed.returncode != 0, completed.stdout + completed.stderr
    assert expected in completed.stderr, completed.stderr
    assert not (installer.prefix / "bin" / "mold").exists(), (
        "a rejected archive must not be unpacked"
    )
    assert not any("rustup" in line for line in installer.recorded()), (
        "the toolchain should not be installed after a rejected archive"
    )
    assert list(installer.scratch.iterdir()) == [], (
        "the scratch directory should be removed on the failure path"
    )


@pytest.mark.parametrize("succeeds", (True, False))
def test_removes_its_scratch_directory(installer: Installer, succeeds: bool) -> None:
    """Scenario: the installer exits, successfully or not.

    Invariant: ``TMPDIR`` is empty afterwards. The trap removes the
    download working directory on both paths, so a failing run does not
    leak the archive it refused.
    """
    if not succeeds:
        tamper_with_checksums(installer, "mismatch")

    completed = installer.run()

    assert (completed.returncode == 0) is succeeds, completed.stdout + completed.stderr
    assert list(installer.scratch.iterdir()) == [], (
        f"scratch directory should be empty, found {list(installer.scratch.iterdir())}"
    )
