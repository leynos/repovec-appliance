"""Provisioning-specific fixtures shared by integration and cmd-mox tests."""

from __future__ import annotations

import shlex
import stat
from pathlib import Path

import pytest

REPO_ROOT = Path(__file__).resolve().parents[2]
HELPER_SOURCE = REPO_ROOT / "packaging" / "libexec" / "repovec-qdrant-api-key"


@pytest.fixture
def patched_helper(tmp_path: Path) -> Path:
    """Return a tmp-rooted copy of the helper script.

    The helper hard-codes ``/etc/repovec`` paths. For cmd-mox tests we cannot
    write there (and would not want to), so we rewrite the three path
    constants (``CONFIG_DIR``, ``KEY_FILE``, and ``LOCK_FILE``) near the top
    of the script to point into ``tmp_path``. The rest of the file -
    including the logic we are trying to exercise - is left verbatim, so the
    contract under test is unchanged. ``/var/lib/repovec`` is intentionally
    left untouched so cmd-mox tests can assert that the canonical home
    directory propagates to ``useradd``.

    ``LOCK_FILE`` must be rewritten too because the helper opens it with
    ``exec 9>"${LOCK_FILE}"`` before any external command is invoked, so a
    real-``/etc/repovec`` path would fail to open in the cmd-mox sandbox
    long before any shim could be observed.

    Returns
    -------
    Path
        Path to the executable, patched helper script under ``tmp_path``.
    """

    config_dir = tmp_path / "etc" / "repovec"
    config_dir.mkdir(parents=True, exist_ok=True)

    # Fail loud if a future rename in the helper script makes the rewrite
    # below a no-op. Silently running the unpatched helper would try to
    # write to the real /etc/repovec and (with sufficient privileges)
    # mutate the host, so we assert each literal is present in ``raw``
    # *before* substituting and verify it's gone from ``patched`` *after*.
    # The pre-check catches a literal that has been renamed away (where
    # ``replace`` is a no-op and the post-check would also pass
    # incorrectly); the post-check catches the rare case where the
    # substituted form itself still embeds the original literal.
    raw = HELPER_SOURCE.read_text(encoding="utf-8")
    expected_literals = (
        "CONFIG_DIR=/etc/repovec",
        "KEY_FILE=/etc/repovec/qdrant-api-key",
        "LOCK_FILE=/etc/repovec/repovec-qdrant-api-key.lock",
    )
    for literal in expected_literals:
        if literal not in raw:
            msg = (
                f"patched_helper fixture failed: '{literal}' not found in "
                "the helper script. The helper's path constants likely "
                "changed; update the rewrite in "
                "integration-tests/provisioning/conftest.py to match."
            )
            raise RuntimeError(msg)

    # Shell-quote the substituted paths so a custom pytest ``basetemp``
    # containing spaces or shell metacharacters can't break the helper's
    # ``set -eu`` parse (or, worse, smuggle commands into it).
    quoted_config_dir = shlex.quote(str(config_dir))
    quoted_key_file = shlex.quote(str(config_dir / "qdrant-api-key"))
    quoted_lock_file = shlex.quote(
        str(config_dir / "repovec-qdrant-api-key.lock"),
    )
    patched = (
        raw.replace(
            "CONFIG_DIR=/etc/repovec", f"CONFIG_DIR={quoted_config_dir}"
        )
        .replace(
            "KEY_FILE=/etc/repovec/qdrant-api-key",
            f"KEY_FILE={quoted_key_file}",
        )
        .replace(
            "LOCK_FILE=/etc/repovec/repovec-qdrant-api-key.lock",
            f"LOCK_FILE={quoted_lock_file}",
        )
    )

    for literal in expected_literals:
        if literal in patched:
            msg = (
                f"patched_helper fixture failed: '{literal}' is still "
                "present after the rewrite. The substituted form likely "
                "re-embeds the original literal; update the rewrite in "
                "integration-tests/provisioning/conftest.py to match."
            )
            raise RuntimeError(msg)

    dest = tmp_path / "repovec-qdrant-api-key"
    dest.write_text(patched, encoding="utf-8")
    dest.chmod(dest.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    return dest


@pytest.fixture
def helper_config_dir(patched_helper: Path) -> Path:
    """Return the ``CONFIG_DIR`` used by :func:`patched_helper`.

    Returns
    -------
    Path
        The tmp-rooted ``etc/repovec`` directory the patched helper
        writes into. Tests use this to populate the simulated config
        tree before invoking the helper.
    """

    return patched_helper.parent / "etc" / "repovec"


@pytest.fixture
def helper_key_file(helper_config_dir: Path) -> Path:
    """Return the ``KEY_FILE`` path used by :func:`patched_helper`.

    Returns
    -------
    Path
        The tmp-rooted ``qdrant-api-key`` path inside
        :func:`helper_config_dir`. Tests read or pre-populate this to
        exercise specific helper branches (absent key, valid key, …).
    """

    return helper_config_dir / "qdrant-api-key"


@pytest.fixture
def helper_env(patched_helper: Path) -> dict[str, str]:
    """Build a minimal environment overlay for running the patched helper.

    Returns
    -------
    dict[str, str]
        Overlay supplying ``HOME`` (rooted in the tmp tree) and
        ``LANG``. ``PATH`` is deliberately omitted; see Notes.

    Notes
    -----
    cmd-mox mutates ``os.environ["PATH"]`` in-place when its fixture
    enters, prepending the shim directory so intercepted commands
    resolve to mocks before the real binaries. Snapshotting ``PATH``
    here would freeze a pre-mutation value and silently bypass the
    shims; if pytest set this fixture up before ``cmd_mox``, the
    helper script would then run against the real ``getent`` /
    ``useradd`` / ``podman`` and could mutate the host when invoked
    with sufficient privileges.

    Instead we omit ``PATH``. Cuprum's ``ExecutionContext.env``
    overlays atop the live ``os.environ`` at invocation time (see
    :func:`cuprum._process_lifecycle._merge_env`), so ``PATH`` is
    sourced from whatever cmd-mox has installed by then — regardless
    of the order pytest happens to evaluate the per-test fixtures.
    """

    return {
        "HOME": str(patched_helper.parent),
        "LANG": "C.UTF-8",
    }
