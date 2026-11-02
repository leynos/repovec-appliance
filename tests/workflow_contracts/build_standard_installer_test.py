"""Hermetic coverage for the build-tools installer.

The shared fixtures live in ``build_standard_installer_support``: they build
a real tarball, seed the pin files, and put stubs for ``curl``, ``uname`` and
``rustup`` on PATH. These tests use them to exercise the installer's success
and failure paths without touching the network or a developer's toolchain.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import hashlib

import pytest

from build_standard_installer_support import (
    ARCHIVE_NAME,
    ARCHIVE_ROOT,
    MOLD_PAYLOAD,
    MOLD_VERSION,
    TOOLCHAIN,
    Installer,
    installer,
)

__all__ = ["installer"]


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
    assert extracted.read_bytes() == MOLD_PAYLOAD, (
        f"{extracted} should hold the mold {MOLD_VERSION} payload, "
        f"found {extracted.read_bytes()!r}"
    )
    assert not (installer.prefix / ARCHIVE_ROOT).exists(), (
        "the archive root should be stripped, not preserved"
    )
    assert f"verified {ARCHIVE_NAME}" in completed.stderr, completed.stderr
    assert (
        f"installed mold {MOLD_VERSION} into {installer.prefix}" in completed.stderr
    ), (
        f"expected the installer to report installing mold {MOLD_VERSION} "
        f"into {installer.prefix}, got: {completed.stderr}"
    )


def test_downloads_the_pinned_url(installer: Installer) -> None:
    """Scenario: the download URL is built from the pin and the host arch.

    Invariant: curl receives the complete URL, including the pinned version
    and the architecture ``uname -m`` selected. The stub records its original
    argv, so a URL lost behind ``--output`` parsing would fail here.
    """
    completed = installer.run()

    assert completed.returncode == 0, completed.stderr
    expected_url = (
        f"https://mold.example.invalid/releases/v{MOLD_VERSION}/"
        f"mold-{MOLD_VERSION}-x86_64-linux.tar.gz"
    )
    curl_lines = [line for line in installer.recorded() if line.startswith("curl ")]
    assert len(curl_lines) == 1, (
        f"expected one curl invocation, recorded: {installer.recorded()}"
    )
    assert "--output" in curl_lines[0], (
        f"the recorded argv should retain --output: {curl_lines[0]}"
    )
    assert curl_lines[0].endswith(expected_url), (
        f"expected the download URL {expected_url!r}, recorded: {curl_lines[0]}"
    )


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
    ), (
        f"expected rustup to install the pinned toolchain {TOOLCHAIN} with the "
        f"minimal profile, recorded: {installer.recorded()}"
    )


def tamper_with_checksums(installer: Installer, case: str) -> None:
    """Make the recorded checksums unusable in the named way."""
    sums = installer.root / "SHA256SUMS"
    if case == "mismatch":
        sums.write_text(f"{'0' * 64}  {ARCHIVE_NAME}\n", encoding="utf-8")
    elif case == "ambiguous":
        # A wrong digest filed ahead of the genuine row. Reading only the
        # first match would verify against the wrong digest, and reading every
        # match would let `sha256sum --check` pass on the genuine row while
        # warning about the other, so the installer has to refuse the file.
        genuine = hashlib.sha256(
            (installer.root / ARCHIVE_NAME).read_bytes()
        ).hexdigest()
        sums.write_text(
            f"{'0' * 64}  {ARCHIVE_NAME}\n{genuine}  {ARCHIVE_NAME}\n",
            encoding="utf-8",
        )
    else:
        sums.write_text(f"{'0' * 64}  another-archive.tar.gz\n", encoding="utf-8")


@pytest.mark.parametrize(
    ("case", "expected_parts"),
    (
        ("mismatch", (f"checksum mismatch for {ARCHIVE_NAME}",)),
        ("unlisted", (f"no checksum recorded for {ARCHIVE_NAME}",)),
        (
            "ambiguous",
            (f"2 checksums recorded for {ARCHIVE_NAME}", "refusing to guess"),
        ),
    ),
)
def test_rejects_an_unverifiable_archive(
    installer: Installer, case: str, expected_parts: tuple[str, ...]
) -> None:
    """Scenario: the checksum file cannot identify the download.

    The download either does not match its recorded digest, is not listed at
    all, or is listed twice. Invariant: the installer fails before unpacking
    anything and before spending time on the toolchain. A wrong artefact must
    never reach the prefix, and a rejected run must not install a toolchain
    as a partial success.
    """
    tamper_with_checksums(installer, case)

    completed = installer.run()

    assert completed.returncode != 0, completed.stdout + completed.stderr
    missing = [part for part in expected_parts if part not in completed.stderr]
    assert not missing, (
        f"the {case} rejection should explain itself, missing {missing}: "
        f"{completed.stderr}"
    )
    assert not (installer.prefix / "bin" / "mold").exists(), (
        "a rejected archive must not be unpacked"
    )
    assert not any("rustup" in line for line in installer.recorded()), (
        "the toolchain should not be installed after a rejected archive"
    )
    assert list(installer.scratch.iterdir()) == [], (
        "the scratch directory should be removed on the failure path"
    )


def test_fails_when_the_download_fails(installer: Installer) -> None:
    """Scenario: curl cannot fetch the release.

    Invariant: the installer exits non-zero with the download diagnostic,
    unpacks nothing, does not install a toolchain, and removes its scratch
    directory. A failed download must not look like a partial success.
    """
    completed = installer.run({"STUB_CURL_FAIL_TIMES": "99"})

    assert completed.returncode != 0, completed.stdout + completed.stderr
    assert "failed to download" in completed.stderr, completed.stderr
    assert not (installer.prefix / "bin" / "mold").exists(), (
        "a failed download must not reach the prefix"
    )
    assert not any("rustup" in line for line in installer.recorded()), (
        "the toolchain should not be installed after a failed download"
    )
    assert list(installer.scratch.iterdir()) == [], (
        "the scratch directory should be removed on the failure path"
    )


def test_fails_when_the_archive_cannot_be_unpacked(installer: Installer) -> None:
    """Scenario: the checksum matches a file that is not a gzip tarball.

    Invariant: verification passes, extraction fails, and the installer
    reports the unpack failure without leaving a binary in the prefix or
    installing a toolchain. A checksum proves the bytes are the expected
    download, not that they are a usable archive.
    """
    tarball = installer.root / ARCHIVE_NAME
    tarball.write_bytes(b"this is not a gzip tarball")
    digest = hashlib.sha256(tarball.read_bytes()).hexdigest()
    (installer.root / "SHA256SUMS").write_text(
        f"{digest}  {ARCHIVE_NAME}\n", encoding="utf-8"
    )

    completed = installer.run()

    assert completed.returncode != 0, completed.stdout + completed.stderr
    assert "failed to unpack" in completed.stderr, completed.stderr
    assert not (installer.prefix / "bin" / "mold").exists(), (
        "a failed extraction must not leave a binary in the prefix"
    )
    assert not any("rustup" in line for line in installer.recorded()), (
        "the toolchain should not be installed after a failed extraction"
    )
    assert list(installer.scratch.iterdir()) == [], (
        "the scratch directory should be removed on the failure path"
    )


def test_fails_when_rustup_is_missing(installer: Installer) -> None:
    """Scenario: mold installs but rustup is absent.

    Invariant: the installer exits non-zero with the rustup diagnostic. The
    linker step runs first, so mold is already installed; that ordering is
    deliberate, because a checksum failure should abort before spending time
    on a toolchain download. The scratch directory is still removed.
    """
    (installer.root / "stub" / "rustup").unlink()

    completed = installer.run()

    assert completed.returncode != 0, completed.stdout + completed.stderr
    assert "rustup not found on PATH" in completed.stderr, completed.stderr
    assert (installer.prefix / "bin" / "mold").is_file(), (
        "the linker step runs before the toolchain step, so mold should be "
        "installed even when rustup is missing"
    )
    assert list(installer.scratch.iterdir()) == [], (
        "the scratch directory should be removed on the failure path"
    )


@pytest.mark.parametrize(
    ("content", "expected"),
    (
        (None, "missing version pin"),
        ("", "empty version pin"),
        ("1.2.3\n4.5.6\n", "expected one line in version pin"),
        ("1.2 3\n", "version pin contains whitespace"),
    ),
)
def test_rejects_an_invalid_pin(
    installer: Installer, content: str | None, expected: str
) -> None:
    """Scenario: the mold version pin is unusable.

    Invariant: a missing, empty, multi-line, or internally spaced pin aborts
    before any download. Silently trimming or concatenating such a file would
    build a nonsensical URL and hide the broken pin.
    """
    version_file = installer.root / "VERSION"
    if content is None:
        version_file.unlink()
    else:
        version_file.write_text(content, encoding="utf-8")

    completed = installer.run()

    assert completed.returncode != 0, completed.stdout + completed.stderr
    assert expected in completed.stderr, completed.stderr
    assert not (installer.prefix / "bin" / "mold").exists(), (
        "an invalid pin must not reach the download step"
    )
    assert not any("rustup" in line for line in installer.recorded()), (
        "the toolchain should not be installed after an invalid pin"
    )
    assert list(installer.scratch.iterdir()) == [], (
        "the scratch directory should be removed on the failure path"
    )


def test_rejects_an_unsupported_architecture(installer: Installer) -> None:
    """Scenario: ``uname -m`` reports a machine mold does not publish for.

    Invariant: the installer names the unsupported machine and aborts before
    downloading. Falling through with an empty architecture would request a
    tarball that cannot exist.
    """
    completed = installer.run({"STUB_UNAME_MACHINE": "riscv64"})

    assert completed.returncode != 0, completed.stdout + completed.stderr
    assert (
        "unsupported architecture for the pinned mold release: riscv64"
        in completed.stderr
    ), completed.stderr
    assert not (installer.prefix / "bin" / "mold").exists(), (
        "an unsupported architecture must not reach the download step"
    )
    assert not any("rustup" in line for line in installer.recorded()), (
        "the toolchain should not be installed after an unsupported architecture"
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
