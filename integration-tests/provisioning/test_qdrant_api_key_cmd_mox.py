"""Command-contract tests for the Qdrant API-key provisioning helper.

These tests do **not** stand in for the full lifecycle suite. Their job is
to make the helper's command orchestration cheap to test, easier to diagnose,
and free from any reliance on rootful Podman or live system users. They run
the real helper script (with its path constants rewritten to a ``tmp_path``
sandbox) through a ``PATH`` populated with cmd-mox shims, then assert that
the expected commands were dispatched with the expected arguments.

The end-to-end behavioural contract is still owned by
``test_qdrant_api_key.py`` and the testcontainers harness; if you find
yourself replicating a lifecycle assertion here, push it back there instead.
"""

from __future__ import annotations

import secrets
from pathlib import Path

import pytest
from cmd_mox import CmdMox

from lib.commands import run_host
from lib.result import CommandResult
from lib.constants import (
    KEY_HEX_LENGTH,
    REPOVEC_GROUP,
    REPOVEC_HOME,
    REPOVEC_SHELL,
    REPOVEC_USER,
    SECRET_NAME,
)

pytest_plugins = ("cmd_mox.pytest_plugin",)

pytestmark = pytest.mark.cmd_mox


def _invoke_helper(helper: Path, env: dict[str, str]) -> CommandResult:
    """Run the patched helper through cuprum and return the structured result."""

    return run_host("sh", str(helper), env=env)


def test_invokes_useradd_when_repovec_user_is_absent(
    cmd_mox: CmdMox,
    patched_helper: Path,
    helper_env: dict[str, str],
) -> None:
    """Helper must create the ``repovec`` user when ``getent`` reports absence."""

    cmd_mox.mock("getent").with_args("passwd", REPOVEC_USER).returns(exit_code=2)
    # Mocks self-verify during cmd-mox teardown, so the act of declaring the
    # expectation is the assertion: if the helper omitted ``useradd`` (or
    # called it with different arguments) verification would fail.
    cmd_mox.mock("useradd").with_args(
        "--system",
        "--home-dir",
        REPOVEC_HOME,
        "--shell",
        REPOVEC_SHELL,
        REPOVEC_USER,
    ).returns(exit_code=0)
    cmd_mox.stub("install").returns(exit_code=0)
    cmd_mox.stub("chown").returns(exit_code=0)
    cmd_mox.stub("chmod").returns(exit_code=0)
    cmd_mox.stub("mv").returns(exit_code=0)
    cmd_mox.stub("podman").returns(exit_code=1)

    result = _invoke_helper(patched_helper, helper_env)

    # ``podman`` is stubbed to fail (no live secret store), which the helper
    # treats as a fatal error. We tolerate exit codes 0 (full success path)
    # or 1 (secret create failure) here because we only care about the
    # useradd contract for this test.
    assert result.exit_code in (0, 1), result.render()


def test_skips_useradd_when_repovec_user_exists(
    cmd_mox: CmdMox,
    patched_helper: Path,
    helper_env: dict[str, str],
) -> None:
    """When ``getent`` finds the user, ``useradd`` must not be invoked."""

    cmd_mox.mock("getent").with_args("passwd", REPOVEC_USER).returns(exit_code=0)
    useradd_spy = cmd_mox.spy("useradd")
    cmd_mox.stub("install").returns(exit_code=0)
    cmd_mox.stub("chown").returns(exit_code=0)
    cmd_mox.stub("chmod").returns(exit_code=0)
    cmd_mox.stub("mv").returns(exit_code=0)
    cmd_mox.stub("podman").returns(exit_code=1)

    _invoke_helper(patched_helper, helper_env)

    useradd_spy.assert_not_called()


def test_generates_key_when_key_file_is_absent(
    cmd_mox: CmdMox,
    patched_helper: Path,
    helper_env: dict[str, str],
    helper_key_file: Path,
) -> None:
    """Helper must perform the chown/chmod/mv sequence when no key exists.

    The script generates random bytes via ``od`` and validates them with
    ``grep`` and ``printf``; we let those run for real so the validation
    branch is exercised. Only the privilege-dependent commands are shimmed.
    """

    assert not helper_key_file.exists(), (
        f"precondition failed: helper key file already exists at {helper_key_file}"
    )

    cmd_mox.mock("getent").with_args("passwd", REPOVEC_USER).returns(exit_code=0)
    cmd_mox.stub("install").returns(exit_code=0)
    chown_spy = cmd_mox.spy("chown")
    chmod_spy = cmd_mox.spy("chmod")
    mv_spy = cmd_mox.spy("mv")
    cmd_mox.stub("podman").returns(exit_code=1)

    _invoke_helper(patched_helper, helper_env)

    chown_spy.assert_called()
    chmod_spy.assert_called()
    mv_spy.assert_called()

    # ``chown`` must target the documented owner:group pair.
    chown_targets = [
        call.args[0]
        for call in chown_spy.invocations
        if call.args
    ]
    assert f"{REPOVEC_USER}:{REPOVEC_GROUP}" in chown_targets, chown_targets

    # The final ``chmod`` invocation should set the contract's mode.
    chmod_modes = [
        call.args[0] for call in chmod_spy.invocations if call.args
    ]
    assert "0400" in chmod_modes, chmod_modes


def test_creates_or_refreshes_podman_secret_from_key_file(
    cmd_mox: CmdMox,
    patched_helper: Path,
    helper_env: dict[str, str],
    helper_key_file: Path,
) -> None:
    """When a stale secret exists, the helper must rm-then-create it."""

    helper_key_file.write_text("a" * KEY_HEX_LENGTH, encoding="utf-8")

    cmd_mox.mock("getent").with_args("passwd", REPOVEC_USER).returns(exit_code=0)
    cmd_mox.stub("install").returns(exit_code=0)
    cmd_mox.stub("chown").returns(exit_code=0)
    cmd_mox.stub("chmod").returns(exit_code=0)
    cmd_mox.stub("mv").returns(exit_code=0)

    podman_spy = cmd_mox.spy("podman")
    podman_spy.returns(exit_code=0)

    _invoke_helper(patched_helper, helper_env)

    invocations = list(podman_spy.invocations)
    sub_commands = [tuple(inv.args[:2]) for inv in invocations if len(inv.args) >= 2]

    assert ("secret", "inspect") in sub_commands, sub_commands
    assert ("secret", "rm") in sub_commands, sub_commands
    assert ("secret", "create") in sub_commands, sub_commands

    rm_index = next(i for i, sub in enumerate(sub_commands) if sub == ("secret", "rm"))
    create_index = next(
        i for i, sub in enumerate(sub_commands) if sub == ("secret", "create")
    )
    assert rm_index < create_index, sub_commands

    create_args = next(
        inv.args for inv in invocations if tuple(inv.args[:2]) == ("secret", "create")
    )
    assert SECRET_NAME in create_args, create_args


def test_does_not_regenerate_valid_existing_key(
    cmd_mox: CmdMox,
    patched_helper: Path,
    helper_env: dict[str, str],
    helper_key_file: Path,
) -> None:
    """A valid hex key of the contract-mandated length must survive a rerun.

    The key is derived from :data:`KEY_HEX_LENGTH` rather than hard-coded so
    that a future contract change to the key length is picked up
    automatically. ``secrets.token_hex`` makes the test self-describing
    (the value is plainly a key) and avoids accidentally landing on a
    fixed string a future helper might special-case.
    """

    original = secrets.token_hex(KEY_HEX_LENGTH // 2)
    assert len(original) == KEY_HEX_LENGTH, original
    helper_key_file.write_text(original, encoding="utf-8")
    original_mtime = helper_key_file.stat().st_mtime_ns

    cmd_mox.mock("getent").with_args("passwd", REPOVEC_USER).returns(exit_code=0)
    cmd_mox.stub("install").returns(exit_code=0)
    cmd_mox.stub("chown").returns(exit_code=0)
    cmd_mox.stub("chmod").returns(exit_code=0)
    mv_spy = cmd_mox.spy("mv")
    podman_spy = cmd_mox.spy("podman")
    podman_spy.returns(exit_code=0)

    _invoke_helper(patched_helper, helper_env)

    # Generation always ends with `mv tmp KEY_FILE`; if `mv` was not called,
    # the helper did not regenerate the key.
    assert mv_spy.call_count == 0, list(mv_spy.invocations)

    assert helper_key_file.read_text(encoding="utf-8") == original, (
        "key content changed unexpectedly on idempotent rerun"
    )
    assert helper_key_file.stat().st_mtime_ns == original_mtime, (
        f"key mtime changed unexpectedly: "
        f"{helper_key_file.stat().st_mtime_ns} != {original_mtime}"
    )

    sub_commands = [
        tuple(inv.args[:2]) for inv in podman_spy.invocations if len(inv.args) >= 2
    ]
    assert ("secret", "create") in sub_commands, sub_commands
