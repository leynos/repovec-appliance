"""Shared fixtures for the build-tools installer contracts.

``scripts/install-build-tools.sh`` downloads a pinned mold release, verifies
it against ``tools/mold/SHA256SUMS``, unpacks it under a prefix, and installs
the pinned toolchain. Nothing in that list is safe to exercise against the
network or a developer's real toolchain, so the contracts drive the real
script with the real ``sha256sum`` and ``tar`` over a tarball the test builds,
and put stubs for ``curl``, ``uname`` and ``rustup`` on PATH.

Covering the real checksum and extraction steps is deliberate: a stub
``sha256sum`` would accept any digest, and a stub ``tar`` would accept any
archive layout, so the two properties the installer exists to guarantee --
that a wrong artefact is refused and that the archive's root is stripped --
would go untested.

The installer's scratch directory is reported through ``TMPDIR``, which each
test points at an otherwise empty directory. That makes cleanup observable: a
successful run and a rejected run must both leave nothing behind.
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

from build_standard_probe import SUBPROCESS_TIMEOUT_SECONDS

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

    def environment(self, overrides: dict[str, str] | None = None) -> dict[str, str]:
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
                "STUB_CURL_COUNTER": str(self.root / "curl-attempts"),
                "STUB_CURL_FAIL_TIMES": "0",
                "STUB_CURL_FAIL_STATUS": "22",
                "STUB_UNAME_FAIL": "0",
                "STUB_UNAME_MACHINE": "x86_64",
                "STUB_UNAME_OS": "Linux",
            }
        )
        if overrides:
            environment.update(overrides)
        return environment

    def run(
        self, overrides: dict[str, str] | None = None
    ) -> subprocess.CompletedProcess[str]:
        """Run the installer and return its completed process."""
        return subprocess.run(
            [BASH, str(INSTALLER_PATH)],
            cwd=self.root,
            env=self.environment(overrides),
            capture_output=True,
            text=True,
            check=False,
            timeout=SUBPROCESS_TIMEOUT_SECONDS,
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

    ``curl`` records its original argv, then copies a prebuilt archive
    instead of downloading it, so the checksum and extraction steps still run
    against real tools. It can be told to fail a bounded number of attempts,
    which is what makes the retry boundary observable. ``uname`` reports
    Linux x86_64 by default, and can be told to fail or report another
    machine. ``rustup`` records its arguments and succeeds, since installing
    a real toolchain is the one step that must not happen here.
    """
    stub = root / "stub"
    stub.mkdir()
    write_stub(
        stub / "curl",
        """printf 'curl %s\\n' "$*" >> "$STUB_LOG"
out=
while [ "$#" -gt 0 ]; do
  case $1 in
    --output) out=$2; shift 2 ;;
    *) shift ;;
  esac
done
attempt=0
if [ -f "$STUB_CURL_COUNTER" ]; then
  attempt=$(cat -- "$STUB_CURL_COUNTER")
fi
attempt=$((attempt + 1))
printf '%s\\n' "$attempt" > "$STUB_CURL_COUNTER"
if [ "$attempt" -le "${STUB_CURL_FAIL_TIMES:-0}" ]; then
  exit "${STUB_CURL_FAIL_STATUS:-22}"
fi
cp -- "$STUB_TARBALL" "$out"
""",
    )
    write_stub(
        stub / "uname",
        """if [ "${STUB_UNAME_FAIL:-0}" = 1 ]; then
  exit 1
fi
case ${1:-} in
  -m) printf '%s\\n' "${STUB_UNAME_MACHINE:-x86_64}" ;;
  *) printf '%s\\n' "${STUB_UNAME_OS:-Linux}" ;;
esac
""",
    )
    write_stub(stub / "rustup", 'printf \'rustup %s\\n\' "$*" >> "$STUB_LOG"\n')


def build_installer(root: Path) -> Installer:
    """Seed every file the installer reads and return the environment."""
    prefix = root / "prefix"
    scratch = root / "scratch"
    scratch.mkdir()
    (root / "home").mkdir()

    digest = build_archive(root / ARCHIVE_NAME)
    (root / "VERSION").write_text(f"{MOLD_VERSION}\n", encoding="utf-8")
    (root / "SHA256SUMS").write_text(f"{digest}  {ARCHIVE_NAME}\n", encoding="utf-8")
    (root / "rust-toolchain.toml").write_text(
        f'[toolchain]\nchannel = "{TOOLCHAIN}"\n', encoding="utf-8"
    )
    install_stubs(root)

    return Installer(root=root, prefix=prefix, scratch=scratch, log=root / "log")


@pytest.fixture
def installer(tmp_path: Path) -> Installer:
    """A prepared installer environment for one test."""
    return build_installer(tmp_path)
