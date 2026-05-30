"""Domain-level assertion helpers for provisioning lifecycle tests.

The helpers in this module wrap raw container shell-outs in named predicates
so that test failures point at the contract being violated, rather than the
shell incantation used to check it. Each helper returns the relevant value so
callers can layer further assertions (for example, capturing a Podman secret
ID and comparing it across runs).
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from typing import TYPE_CHECKING

from .constants import (
    KEY_FILE,
    KEY_FILE_MODE,
    KEY_HEX_LENGTH,
    REPOVEC_GROUP,
    REPOVEC_HOME,
    REPOVEC_SHELL,
    REPOVEC_USER,
    SECRET_NAME,
)

if TYPE_CHECKING:
    from .container import ContainerSession


HEX_KEY_RE = re.compile(rf"^[0-9a-fA-F]{{{KEY_HEX_LENGTH}}}$")


@dataclass(frozen=True)
class PasswdEntry:
    """Parsed columns of a ``getent passwd`` row."""

    name: str
    uid: str
    gid: str
    gecos: str
    home: str
    shell: str

    @classmethod
    def parse(cls, raw: str) -> PasswdEntry:
        fields = raw.rstrip("\n").split(":")
        if len(fields) != 7:
            msg = f"unexpected passwd format: {raw!r}"
            raise AssertionError(msg)
        name, _password, uid, gid, gecos, home, shell = fields
        return cls(name=name, uid=uid, gid=gid, gecos=gecos, home=home, shell=shell)


@dataclass(frozen=True)
class FileStat:
    """Subset of ``stat`` output used in provisioning assertions."""

    mode: str
    user: str
    group: str
    size: int
    mtime: str

    @classmethod
    def parse(cls, raw: str) -> FileStat:
        fields = raw.rstrip("\n").split("\t")
        if len(fields) != 5:
            msg = f"unexpected stat format: {raw!r}"
            raise AssertionError(msg)
        mode, user, group, size, mtime = fields
        return cls(
            mode=mode,
            user=user,
            group=group,
            size=int(size),
            mtime=mtime,
        )


def stat_file(session: ContainerSession, path: str) -> FileStat:
    """Return the mode/owner/size/mtime of ``path`` inside the container.

    Uses GNU ``stat`` format codes that are stable across Fedora releases.
    """

    result = session.must_run(
        "stat",
        "-c",
        "%a\t%U\t%G\t%s\t%Y",
        path,
    )
    return FileStat.parse(result.stdout)


def get_passwd_entry(session: ContainerSession, name: str) -> PasswdEntry:
    """Return the ``getent passwd`` entry for ``name`` or fail."""

    result = session.must_run("getent", "passwd", name)
    return PasswdEntry.parse(result.stdout)


def assert_repovec_user(session: ContainerSession) -> PasswdEntry:
    """Verify the ``repovec`` user exists with the documented attributes."""

    entry = get_passwd_entry(session, REPOVEC_USER)
    assert entry.name == REPOVEC_USER, entry
    assert entry.home == REPOVEC_HOME, entry
    assert entry.shell == REPOVEC_SHELL, entry
    return entry


def assert_key_file_contract(session: ContainerSession) -> str:
    """Verify the key file exists with the mandated mode, owner, and shape.

    Returns the captured key contents so callers can use them for cross-run
    comparisons (idempotence, refresh, etc.).
    """

    stat = stat_file(session, KEY_FILE)
    # ``stat -c %a`` may emit ``400`` or ``0400`` depending on libc; normalise.
    assert stat.mode.zfill(4) == KEY_FILE_MODE, stat
    assert stat.user == REPOVEC_USER, stat
    assert stat.group == REPOVEC_GROUP, stat

    contents = session.must_run("cat", KEY_FILE).stdout
    assert HEX_KEY_RE.fullmatch(contents.strip("\n")), (
        f"key file contents are not {KEY_HEX_LENGTH} hex chars: {contents!r}"
    )
    return contents


def assert_podman_secret_exists(session: ContainerSession) -> str:
    """Assert the Podman secret exists and return its rootful inspect ID.

    The ID changes whenever the secret is removed and re-created; tests rely
    on this to prove that a refresh ran end-to-end.
    """

    result = session.must_run(
        "podman",
        "secret",
        "inspect",
        "--format",
        "{{.ID}}",
        SECRET_NAME,
    )
    secret_id = result.stdout.strip()
    assert secret_id, f"empty secret ID for {SECRET_NAME}"
    return secret_id


def assert_podman_secret_name(session: ContainerSession) -> None:
    """Assert that the secret's ``Name`` field round-trips correctly."""

    result = session.must_run(
        "podman",
        "secret",
        "inspect",
        "--format",
        "{{.Spec.Name}}",
        SECRET_NAME,
    )
    assert result.stdout.strip() == SECRET_NAME, result.stdout
