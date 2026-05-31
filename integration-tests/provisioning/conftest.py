"""Provisioning-specific fixtures shared by integration and cmd-mox tests."""

from __future__ import annotations

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

    # Fail loud if a future rename in the helper script makes the str.replace
    # calls a no-op: silently running the unpatched helper would try to write
    # to the real /etc/repovec and (with sufficient privileges) mutate the
    # host. Asserting the originals are gone catches the refactor at fixture
    # setup, not deep in a confusing test failure.
    for literal in (
        "CONFIG_DIR=/etc/repovec",
        "KEY_FILE=/etc/repovec/qdrant-api-key",
    ):
        if literal in patched:
            msg = (
                f"patched_helper fixture failed: '{literal}' is still present "
                "in the helper script after the rewrite. The helper's path "
                "constants likely changed; update the rewrite in "
                "integration-tests/provisioning/conftest.py to match."
            )
            raise RuntimeError(msg)

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
    """Return a minimal environment overlay for running the patched helper.

    cmd-mox mutates ``os.environ["PATH"]`` in-place when its fixture enters,
    prepending the shim directory so intercepted commands resolve to mocks
    before the real binaries. Snapshotting ``PATH`` here would freeze a
    pre-mutation value and silently bypass the shims; if pytest set this
    fixture up before ``cmd_mox``, the helper script would then run against
    the real ``getent`` / ``useradd`` / ``podman`` and could mutate the host
    when invoked with sufficient privileges.

    Instead we deliberately omit ``PATH``. Cuprum's ``ExecutionContext.env``
    overlays atop the live ``os.environ`` at invocation time
    (see :func:`cuprum._process_lifecycle._merge_env`), so ``PATH`` is
    sourced from whatever cmd-mox has installed by then — regardless of the
    order pytest happens to evaluate the per-test fixtures.
    """

    return {
        "HOME": str(patched_helper.parent),
        "LANG": "C.UTF-8",
    }
