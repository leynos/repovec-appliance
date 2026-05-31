"""End-to-end lifecycle tests for the Qdrant API-key provisioning helper.

These tests run the real helper inside a privileged Fedora container with
rootful Podman, exercising the full provisioning contract:

* the ``repovec`` system user is created with the documented home and shell;
* ``/etc/repovec`` and the key file land with the documented modes/ownership;
* the rootful Podman secret is created and refreshed correctly;
* a valid existing key survives idempotent re-runs untouched;
* a missing key is regenerated and the secret refreshed atomically;
* ``/etc/repovec`` itself has the documented mode and ownership.

The cmd-mox suite in ``test_qdrant_api_key_cmd_mox.py`` covers command intent
without requiring a privileged runtime; this file owns the lifecycle truths
that only show up when real ``useradd`` and real Podman are involved.
"""

from __future__ import annotations

import time
from pathlib import PurePosixPath

import pytest

from lib.assertions import (
    HEX_KEY_RE,
    assert_key_file_contract,
    assert_podman_secret_exists,
    assert_podman_secret_name,
    assert_repovec_user,
    get_passwd_entry,
    stat_file,
)
from lib.constants import (
    ETC_DIR_MODE,
    HELPER_SCRIPT,
    KEY_FILE,
    REPOVEC_ETC_DIR,
    REPOVEC_GROUP,
    REPOVEC_USER,
    SECRET_NAME,
)
from lib.container import ContainerSession

pytestmark = pytest.mark.integration


def _run_helper(session: ContainerSession) -> None:
    """Execute the provisioning helper inside the container."""

    session.must_run("/bin/sh", HELPER_SCRIPT)


def test_creates_repovec_system_user_when_absent(
    container_session: ContainerSession,
) -> None:
    """The helper must create the ``repovec`` system user from scratch."""

    # ``container_session`` cleanup already removed the user; double-check
    # the precondition so a failure here points at the fixture, not the helper.
    precheck = container_session.run("getent", "passwd", REPOVEC_USER)
    assert not precheck.ok, precheck.render()

    _run_helper(container_session)

    entry = assert_repovec_user(container_session)
    # ``useradd --system`` allocates a UID below the regular range; we only
    # care that the helper made the user exist with the documented attributes.
    assert entry.name == REPOVEC_USER, (
        f"unexpected passwd entry name: {entry.name!r} (full entry: {entry})"
    )


def test_creates_key_file_with_mode_0400_and_ownership_repovec(
    container_session: ContainerSession,
) -> None:
    """The key file must materialise with the documented mode and ownership."""

    precheck = container_session.run("test", "-e", KEY_FILE)
    assert not precheck.ok, "key file existed before the helper ran"

    _run_helper(container_session)

    contents = assert_key_file_contract(container_session)
    assert HEX_KEY_RE.fullmatch(contents.strip("\n")), contents


def test_creates_podman_secret_repovec_qdrant_api_key(
    container_session: ContainerSession,
) -> None:
    """The rootful Podman secret must be created and inspectable."""

    # Best-effort removal of any pre-existing secret; the cleanup fixture
    # already runs this, so a fresh container should be a no-op here.
    container_session.run_shell(
        f"podman secret rm {SECRET_NAME} >/dev/null 2>&1 || true",
    )

    _run_helper(container_session)

    secret_id = assert_podman_secret_exists(container_session)
    # A non-empty ID proves the secret is fully materialised; an empty string
    # back from ``podman secret inspect`` means the helper succeeded but the
    # secret entry is half-created.
    assert secret_id, f"podman secret {SECRET_NAME!r} returned an empty ID"
    assert_podman_secret_name(container_session)


def test_preserves_existing_key_file_on_rerun(
    container_session: ContainerSession,
) -> None:
    """A valid existing key file must survive a second helper run untouched."""

    _run_helper(container_session)

    first_stat = stat_file(container_session, KEY_FILE)
    first_contents = container_session.must_run("cat", KEY_FILE).stdout

    # Sleep long enough to make any subsequent ``mtime`` change observable on
    # filesystems that record seconds-precision timestamps.
    time.sleep(1.1)

    _run_helper(container_session)

    second_stat = stat_file(container_session, KEY_FILE)
    second_contents = container_session.must_run("cat", KEY_FILE).stdout

    assert second_contents == first_contents, (
        "key file contents changed across runs (helper is not idempotent)"
    )
    assert second_stat.mode == first_stat.mode, (
        f"key file mode changed across runs: "
        f"{first_stat.mode!r} -> {second_stat.mode!r}"
    )
    assert second_stat.user == first_stat.user, (
        f"key file owner changed across runs: "
        f"{first_stat.user!r} -> {second_stat.user!r}"
    )
    assert second_stat.group == first_stat.group, (
        f"key file group changed across runs: "
        f"{first_stat.group!r} -> {second_stat.group!r}"
    )
    # The helper's contract is "do not touch a valid existing key", so the
    # mtime must not have advanced even though the script ran end-to-end.
    assert second_stat.mtime == first_stat.mtime, (
        f"key file mtime advanced across idempotent runs "
        f"(before: {first_stat}, after: {second_stat})"
    )

    # The secret should still be present, even if its ID rolled over because
    # the helper rms+creates on every run when a stale secret is found.
    assert_podman_secret_exists(container_session)


def test_regenerates_key_file_when_absent_and_refreshes_secret(
    container_session: ContainerSession,
) -> None:
    """A missing key file must be regenerated and the secret refreshed."""

    _run_helper(container_session)
    original_contents = container_session.must_run("cat", KEY_FILE).stdout
    original_secret_id = assert_podman_secret_exists(container_session)

    container_session.must_run("rm", "-f", KEY_FILE)
    precheck = container_session.run("test", "-e", KEY_FILE)
    assert not precheck.ok, "key file was not removed by the test setup"

    _run_helper(container_session)

    new_contents = assert_key_file_contract(container_session)
    assert new_contents != original_contents, "helper did not regenerate the key"
    assert HEX_KEY_RE.fullmatch(new_contents.strip("\n")), (
        f"regenerated key is not a valid 64-hex string: {new_contents!r}"
    )

    new_secret_id = assert_podman_secret_exists(container_session)
    assert new_secret_id != original_secret_id, (
        "podman secret was not refreshed alongside the regenerated key"
    )


def test_creates_etc_repovec_with_mode_0750_and_ownership_root_repovec(
    container_session: ContainerSession,
) -> None:
    """``/etc/repovec`` must end up as ``0750 root:repovec``."""

    precheck = container_session.run("test", "-d", REPOVEC_ETC_DIR)
    assert not precheck.ok, f"{REPOVEC_ETC_DIR} existed before the helper ran"

    _run_helper(container_session)

    etc_stat = stat_file(container_session, REPOVEC_ETC_DIR)
    assert etc_stat.mode.zfill(4) == ETC_DIR_MODE, etc_stat
    assert etc_stat.user == "root", etc_stat
    assert etc_stat.group == REPOVEC_GROUP, etc_stat

    # The key file lives directly under this directory; sanity-check that the
    # path remains the documented constant so refactors of the helper that
    # accidentally move it are caught here.
    assert PurePosixPath(KEY_FILE).parent == PurePosixPath(REPOVEC_ETC_DIR), (
        f"KEY_FILE parent {PurePosixPath(KEY_FILE).parent!r} does not match "
        f"REPOVEC_ETC_DIR {REPOVEC_ETC_DIR!r}; constants drifted apart."
    )
    passwd_entry = get_passwd_entry(container_session, REPOVEC_USER)
    assert passwd_entry.name == REPOVEC_USER, (
        f"passwd lookup returned wrong name: {passwd_entry.name!r} (entry: {passwd_entry})"
    )
