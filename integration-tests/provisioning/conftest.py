"""Provisioning-specific fixtures shared by integration and cmd-mox tests."""

from __future__ import annotations

import os
import shutil
import stat
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
HELPER_SOURCE = REPO_ROOT / "packaging" / "libexec" / "repovec-qdrant-api-key"


@pytest.fixture()
def patched_helper(tmp_path: Path) -> Path:
    """Return a tmp-rooted copy of the helper script.

    The helper hard-codes ``/etc/repovec`` paths. For cmd-mox tests we cannot
    write there (and would not want to), so we rewrite the four path constants
    near the top of the script to point into ``tmp_path``. The rest of the
    file – including the logic we are trying to exercise – is left verbatim,
    so the contract under test is unchanged.
    """

    config_dir = tmp_path / "etc" / "repovec"
    config_dir.mkdir(parents=True, exist_ok=True)

    # ``/var/lib/repovec`` is only referenced as a ``useradd --home-dir``
    # argument, which the cmd-mox suite stubs out. We deliberately leave the
    # path literal intact so tests can assert that the canonical home dir
    # propagates through to ``useradd``.
    raw = HELPER_SOURCE.read_text(encoding="utf-8")
    patched = raw.replace(
        "CONFIG_DIR=/etc/repovec", f"CONFIG_DIR={config_dir}"
    ).replace(
        "KEY_FILE=/etc/repovec/qdrant-api-key",
        f"KEY_FILE={config_dir / 'qdrant-api-key'}",
    )

    dest = tmp_path / "repovec-qdrant-api-key"
    dest.write_text(patched, encoding="utf-8")
    dest.chmod(dest.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return dest


@pytest.fixture()
def helper_config_dir(patched_helper: Path) -> Path:
    """Return the ``CONFIG_DIR`` used by :func:`patched_helper`."""

    return patched_helper.parent / "etc" / "repovec"


@pytest.fixture()
def helper_key_file(helper_config_dir: Path) -> Path:
    """Return the ``KEY_FILE`` path used by :func:`patched_helper`."""

    return helper_config_dir / "qdrant-api-key"


@pytest.fixture()
def helper_env(patched_helper: Path) -> dict[str, str]:
    """Return a clean environment for running the patched helper.

    cmd-mox mutates ``PATH`` to point at its shim directory; this fixture
    starts from a minimal baseline so tests do not accidentally rely on the
    developer's interactive ``PATH``.
    """

    base = {
        "PATH": os.environ.get("PATH", "/usr/local/bin:/usr/bin:/bin"),
        "HOME": str(patched_helper.parent),
        "LANG": "C.UTF-8",
    }
    return base
