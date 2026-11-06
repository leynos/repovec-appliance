"""Contract tests for the Cargo configuration and the pinned mold release.

``.cargo/config.toml`` is what makes the development build standard the
default for a bare ``cargo build``. Cargo auto-discovers it from the
workspace, so the file is not optional documentation: it decides the
linker and the frontend for every developer and every CI step that does
not assign ``RUSTFLAGS`` itself.

The contract has three jobs. It pins the standard's flags and the Linux
gate that keeps the Linux-only linker off macOS and Windows. It holds
the two flag sources equal where Cargo would otherwise replace one with
the other: a matching ``[target.*]`` table replaces ``[build]
rustflags``, so ``-Zthreads=8`` must be repeated there. And it refuses a
codegen backend anywhere in the document, because the standard is a
linker and a parallel frontend, not a Cranelift adoption.

The pin files are checked here too. A version and a checksum list that
drift apart would make the installer download one release and verify it
against another release's digest, so neither is asserted alone.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

import re

import pytest

from build_standard_probe import (
    CARGO_CONFIG_PATH,
    MOLD_FLAG,
    MOLD_SHA256SUMS_PATH,
    MOLD_VERSION_PATH,
    THREADS_FLAG,
    iter_keys,
    iter_strings,
    load_cargo_config,
)

pytestmark = pytest.mark.skipif(
    not CARGO_CONFIG_PATH.exists(),
    reason="Cargo configuration not present in this working copy (e.g. "
    "inside mutmut's mutants/ sandbox, which does not copy dotfiles)",
)

PINNED_VERSION = "2.41.0"

#: The two published digests, exactly as rui314/mold publishes them.
EXPECTED_ARCHIVES = {
    f"mold-{PINNED_VERSION}-x86_64-linux.tar.gz": (
        "a3696680d99e692970590a178bc3a33d78d60d1c6dc9db7a11b557b02b751f5d"
    ),
    f"mold-{PINNED_VERSION}-aarch64-linux.tar.gz": (
        "946de2774b06a71346bd59b55fddba610b65b8d93c3a4a1559cc84e103472710"
    ),
}

_SHA256_RE = re.compile(r"^[0-9a-f]{64}$")


def _config() -> dict[str, object]:
    """Parse the Cargo configuration."""
    return load_cargo_config()


def test_build_rustflags_carry_the_parallel_frontend_for_every_platform() -> None:
    """Scenario: the default frontend flag is dropped or altered.

    Invariant: ``[build]`` applies to every platform and names
    ``-Zthreads=8`` and nothing else. Anything platform-specific belongs
    in a target table, where a matching table replaces this one.
    """
    build = _config().get("build")
    assert isinstance(build, dict), "the configuration should define [build]"
    assert build.get("rustflags") == [THREADS_FLAG], (
        "the [build] rustflags should be exactly "
        f"[{THREADS_FLAG!r}], found {build.get('rustflags')!r}"
    )


def test_linux_target_table_repeats_the_frontend_and_adds_mold() -> None:
    """Scenario: the Linux table drops a flag or grows a second target.

    Invariant: exactly one target table exists, for Linux, and it repeats
    ``-Zthreads=8`` because a matching table replaces ``[build]
    rustflags``. The linker flag appears only there, so no other platform
    is told to load a linker it does not have.
    """
    target = _config().get("target")
    assert isinstance(target, dict), "the configuration should define [target]"
    assert list(target) == ['cfg(target_os = "linux")'], (
        "the configuration should define exactly one target table, for "
        f"Linux; found {sorted(target)}"
    )

    linux = target['cfg(target_os = "linux")']
    assert isinstance(linux, dict), "the Linux target table should be a table"
    assert linux.get("rustflags") == [THREADS_FLAG, MOLD_FLAG], (
        "the Linux rustflags should be "
        f"[{THREADS_FLAG!r}, {MOLD_FLAG!r}], found {linux.get('rustflags')!r}"
    )


def test_no_codegen_backend_is_configured_anywhere() -> None:
    """Scenario: a codegen backend, such as Cranelift, is reintroduced.

    Invariant: no key names a codegen backend and no value mentions
    Cranelift. The standard is the linker and the parallel frontend; a
    backend would change code generation rather than build plumbing.
    """
    config = _config()
    forbidden_keys = [
        path
        for path in iter_keys(config)
        if str(path[-1]).lower().replace("-", "").replace("_", "")
        == "codegenbackend"
    ]
    assert not forbidden_keys, (
        f"the configuration should not set a codegen backend; found {forbidden_keys}"
    )

    cranelift_values = [
        value for value in iter_strings(config) if "cranelift" in value.lower()
    ]
    assert not cranelift_values, (
        "the configuration should not mention Cranelift; "
        f"found {cranelift_values}"
    )


def test_every_table_defining_rustflags_defines_the_frontend() -> None:
    """Scenario: a new target table replaces ``[build]`` but omits a flag.

    Invariant: any table that sets ``rustflags`` includes
    ``-Zthreads=8``. Cargo replaces rather than merges, so a table that
    forgets it silently loses the parallel frontend for that target.
    """
    config = _config()

    def rustflag_tables(value: object, path: tuple[object, ...] = ()) -> list[
        tuple[tuple[object, ...], object]
    ]:
        """Return the path and value of every table that defines rustflags."""
        tables: list[tuple[tuple[object, ...], object]] = []
        if isinstance(value, dict):
            if "rustflags" in value:
                tables.append((path, value["rustflags"]))
            for key, child in value.items():
                tables.extend(rustflag_tables(child, (*path, key)))
        return tables

    tables = rustflag_tables(config)
    assert tables, "the configuration should define at least one rustflags table"
    for path, flags in tables:
        assert isinstance(flags, list), f"rustflags at {path} should be a list"
        assert THREADS_FLAG in flags, (
            f"rustflags at {path or ('root',)} omit {THREADS_FLAG!r}; Cargo "
            "replaces tables rather than merging them, so the frontend is lost"
        )


def test_the_mold_version_pin_is_a_single_release() -> None:
    """Scenario: the version pin is empty, multi-line, or drifts.

    Invariant: ``tools/mold/VERSION`` names exactly the release the
    checksum list describes.
    """
    assert MOLD_VERSION_PATH.exists(), "tools/mold/VERSION should exist"
    version = MOLD_VERSION_PATH.read_text(encoding="utf-8")
    assert version == f"{PINNED_VERSION}\n", (
        f"tools/mold/VERSION should be {PINNED_VERSION!r} plus a newline, "
        f"found {version!r}"
    )


def test_the_checksum_list_names_both_archives_for_the_pinned_version() -> None:
    """Scenario: a digest is mistyped, dropped, or belongs to another release.

    Invariant: the checksum list holds exactly the two published archives
    for the pinned version, each with a well-formed digest. The installer
    resolves one row by artefact name, so a missing or extra row is a
    parse failure rather than a tolerant match.
    """
    assert MOLD_SHA256SUMS_PATH.exists(), "tools/mold/SHA256SUMS should exist"
    rows = [
        line.split()
        for line in MOLD_SHA256SUMS_PATH.read_text(encoding="utf-8").splitlines()
        if line.strip()
    ]
    assert all(len(row) == 2 for row in rows), (
        f"every checksum row should be '<digest>  <name>', found {rows}"
    )
    assert len(rows) == len(EXPECTED_ARCHIVES), (
        "tools/mold/SHA256SUMS should hold one row per published archive; "
        f"expected {len(EXPECTED_ARCHIVES)}, found {len(rows)} in {rows}"
    )

    digests = {name: digest for digest, name in rows}
    assert digests == EXPECTED_ARCHIVES, (
        "tools/mold/SHA256SUMS should name exactly "
        f"{EXPECTED_ARCHIVES}, found {digests}"
    )
    for digest in digests.values():
        assert _SHA256_RE.match(digest), (
            f"checksum {digest!r} is not a lowercase 64-character SHA-256"
        )

