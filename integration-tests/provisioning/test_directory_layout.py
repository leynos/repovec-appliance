"""End-to-end provisioning tests for the repovec directory-layout assets.

These tests install the checked-in sysusers.d and tmpfiles.d assets
into the integration container, run systemd-sysusers then
systemd-tmpfiles --create (the exact pair repovec-provision.service
runs), and assert the resulting /var/lib/repovec tree satisfies the
appliance confidentiality contract:

* the repovec account exists with the documented home and nologin shell
  (proving sysusers-before-tmpfiles resolves ownership by name, R-2);
* every data directory is repovec:repovec with mode 0700 (SI-1),
  including qdrant-storage as root:root 0700 (SI-2).

This container does not run systemd as PID 1 (CMD ["sleep",
"infinity"]), so the roadmap systemctl start repovec.target acceptance
criterion and the qdrant.service activeness assertion cannot be
exercised here; the static contract (repovec-ci systemd-gate plus the
pure directory_layout validator) and this lifecycle pairing are the CI
proxy, matching the 1.2.2 precedent. The full systemd run is documented
for a capable host in docs/execplans/1-3-3-... Milestone 3.
"""

from __future__ import annotations

import pytest

from lib.assertions import (
    assert_directory_contract,
    assert_repovec_user,
    stat_file,
)
from lib.constants import (
    ASSUMED_SYSUSERS_SOURCE,
    ASSUMED_TMPFILES_SOURCE,
    REPOVEC_HOME,
    REPOVEC_USER,
)
from lib.container import ContainerSession

pytestmark = pytest.mark.integration

REPOVEC_PROVISION_SYSUSERS = "/usr/bin/systemd-sysusers"
REPOVEC_PROVISION_TMPFILES = "/usr/bin/systemd-tmpfiles"
REPOVEC_PROVISION_TMPFILES_CREATE = "--create"


def _provision(session: ContainerSession) -> None:
    """Apply the sysusers then tmpfiles assets, mirroring the oneshot."""

    session.must_run(REPOVEC_PROVISION_SYSUSERS, ASSUMED_SYSUSERS_SOURCE)
    session.must_run(
        REPOVEC_PROVISION_TMPFILES,
        REPOVEC_PROVISION_TMPFILES_CREATE,
        ASSUMED_TMPFILES_SOURCE,
    )


def test_provisions_repovec_user_and_private_directory_tree(
    container_session: ContainerSession,
) -> None:
    """The ordered pair must materialize the user, then chown by name."""

    _provision(container_session)

    # R-2: ownership resolves by name, which requires the sysusers line
    # to have run before tmpfiles chowns.
    entry = assert_repovec_user(container_session)
    assert entry.home == REPOVEC_HOME, entry
    assert entry.name == REPOVEC_USER, entry

    # SI-1/SI-2: every data directory is 0700 with documented owner/group.
    assert stat_file(container_session, entry.home).mode.zfill(4) == "0700", entry
    assert_directory_contract(container_session)


def test_repovec_provisioning_is_idempotent(
    container_session: ContainerSession,
) -> None:
    """Re-running the ordered pair must converge without changing the tree."""

    _provision(container_session)
    root_stat = stat_file(container_session, REPOVEC_HOME)
    storage_stat = stat_file(container_session, f"{REPOVEC_HOME}/qdrant-storage")

    _provision(container_session)

    assert stat_file(container_session, REPOVEC_HOME) == root_stat, "root stat changed"
    assert (
        stat_file(container_session, f"{REPOVEC_HOME}/qdrant-storage") == storage_stat
    ), "qdrant-storage stat changed"
    assert_directory_contract(container_session)
