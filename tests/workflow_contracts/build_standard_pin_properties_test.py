"""Property-based contracts for pin parsing and checksum validation.

The installer refuses a pin it cannot read and a checksum file it cannot
resolve to exactly one digest. Example-based tests cover the cases the
repository has seen; these properties generate the shapes a malformed file
could take and assert the same accept/reject decision and diagnostic.

The pin cases run the real ``read_pin`` helper from
``scripts/build-tools-common.sh``. The checksum cases drive the real installer
over a tarball the test builds, so the genuine ``awk`` and ``sha256sum`` logic
is what decides each case.

Each example gets its own temporary directory rather than the ``tmp_path``
fixture: Hypothesis does not reset function-scoped fixtures between generated
inputs, and these cases write files.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import hashlib
import subprocess
import tempfile
from pathlib import Path

from hypothesis import given, settings, strategies as st
from hypothesis.strategies import DrawFn

from build_standard_installer_support import (
    ARCHIVE_NAME,
    BASH,
    REPO_ROOT,
    build_installer,
)
from build_standard_probe import SUBPROCESS_TIMEOUT_SECONDS

#: Each checksum case runs a full installer pass, so keep the count modest.
SETTINGS = settings(max_examples=20, deadline=None)

COMMON_SCRIPT = REPO_ROOT / "scripts" / "build-tools-common.sh"

_VERSION = st.from_regex(r"[0-9]{1,3}\.[0-9]{1,3}\.[0-9]{1,3}", fullmatch=True)
_PADDING = st.text(alphabet=" \t", max_size=3)


def read_pin(path: Path) -> subprocess.CompletedProcess[str]:
    """Run the real ``read_pin`` helper against one on-disk pin file."""
    return subprocess.run(
        [BASH, "-c", '. "$1"; read_pin "$2"', "_", str(COMMON_SCRIPT), str(path)],
        capture_output=True,
        text=True,
        check=False,
        timeout=SUBPROCESS_TIMEOUT_SECONDS,
    )


@st.composite
def valid_pins(draw: DrawFn) -> tuple[str, str]:
    """Generate a single-line pin with trim-only surrounding whitespace."""
    version = draw(_VERSION)
    return f"{draw(_PADDING)}{version}{draw(_PADDING)}\n", version


@SETTINGS
@given(valid_pins())
def test_read_pin_accepts_a_single_clean_line(case: tuple[str, str]) -> None:
    """Check that a single clean line is read as the pin."""
    content, expected = case
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "VERSION"
        path.write_text(content, encoding="utf-8")

        completed = read_pin(path)

    assert completed.returncode == 0, completed.stderr
    assert completed.stdout == expected, (
        f"expected the trimmed pin {expected!r}, got {completed.stdout!r}"
    )


@st.composite
def invalid_pins(draw: DrawFn) -> tuple[str, str | None, str]:
    """Generate an unusable pin and the diagnostic it should produce."""
    kind = draw(st.sampled_from(("missing", "empty", "multiline", "whitespace")))
    if kind == "missing":
        return kind, None, "missing version pin"
    if kind == "empty":
        content = draw(
            st.one_of(
                st.just(""),
                st.text(alphabet=" \t", min_size=1, max_size=4).map(
                    lambda text: f"{text}\n"
                ),
            )
        )
        return kind, content, "empty version pin"
    if kind == "multiline":
        versions = draw(st.lists(_VERSION, min_size=2, max_size=4))
        return kind, "\n".join(versions) + "\n", "expected one line in version pin"
    version = draw(_VERSION)
    separator = draw(st.sampled_from((" ", "\t", "  ")))
    return kind, f"{version}{separator}{version}\n", "version pin contains whitespace"


@SETTINGS
@given(invalid_pins())
def test_read_pin_rejects_invalid_files(case: tuple[str, str | None, str]) -> None:
    """Check that invalid pin files are rejected."""
    kind, content, expected = case
    with tempfile.TemporaryDirectory() as directory:
        path = Path(directory) / "VERSION"
        if content is not None:
            path.write_text(content, encoding="utf-8")

        completed = read_pin(path)

    assert completed.returncode != 0, (
        f"the {kind} pin should be rejected, got stdout {completed.stdout!r}"
    )
    assert expected in completed.stderr, completed.stderr


@st.composite
def checksum_counts(draw: DrawFn) -> tuple[int, int, int]:
    """Generate how many correct, wrong and unrelated rows a file holds."""
    return (
        draw(st.integers(min_value=0, max_value=2)),
        draw(st.integers(min_value=0, max_value=2)),
        draw(st.integers(min_value=0, max_value=2)),
    )


@SETTINGS
@given(checksum_counts())
def test_checksum_rows_are_validated(counts: tuple[int, int, int]) -> None:
    """Scenario: a checksum file holds an arbitrary set of rows.

    Invariant: the installer accepts exactly one genuine row and refuses any
    file that resolves to no row, several rows, or one wrong row. The
    diagnostic names the reason rather than leaving a bare checksum failure.
    """
    correct_count, wrong_count, other_count = counts
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory) / "installer"
        root.mkdir()
        installer = build_installer(root)
        digest = hashlib.sha256((root / ARCHIVE_NAME).read_bytes()).hexdigest()

        rows = [f"{digest}  {ARCHIVE_NAME}"] * correct_count
        rows += [f"{'0' * 64}  {ARCHIVE_NAME}"] * wrong_count
        rows += [f"{'0' * 64}  other-archive.tar.gz"] * other_count
        (root / "SHA256SUMS").write_text(
            "\n".join(rows) + ("\n" if rows else ""), encoding="utf-8"
        )

        completed = installer.run()

        matching = correct_count + wrong_count
        context = f"counts={counts} stderr={completed.stderr!r}"
        if matching == 0:
            assert completed.returncode != 0, context
            assert f"no checksum recorded for {ARCHIVE_NAME}" in completed.stderr, (
                context
            )
        elif matching > 1:
            assert completed.returncode != 0, context
            assert (
                f"{matching} checksums recorded for {ARCHIVE_NAME}" in completed.stderr
            ), context
        elif correct_count == 1:
            assert completed.returncode == 0, context
            unpacked = installer.prefix / "bin" / "mold"
            assert unpacked.is_file(), (
                f"expected {unpacked} from {ARCHIVE_NAME}: {context}"
            )
        else:
            assert completed.returncode != 0, context
            assert f"checksum mismatch for {ARCHIVE_NAME}" in completed.stderr, context
