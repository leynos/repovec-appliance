"""Unit tests for the parsing helpers in :mod:`lib.assertions`.

The ``PasswdEntry.parse`` and ``FileStat.parse`` classmethods are the
only pieces of :mod:`lib.assertions` that have non-trivial branching
that does not require a live container. These tests pin the happy
path and the documented failure modes (wrong field count, non-numeric
size) so a future change to either parser cannot silently degrade
the assertion helpers that consume them.
"""

from __future__ import annotations

import pytest

from lib.assertions import FileStat, PasswdEntry


# ----- PasswdEntry --------------------------------------------------------


def test_passwd_parse_returns_all_seven_columns() -> None:
    raw = "repovec:x:996:993::/var/lib/repovec:/usr/sbin/nologin"

    entry = PasswdEntry.parse(raw)

    assert entry.name == "repovec", "passwd name column should round-trip"
    assert entry.uid == "996", "passwd uid column should round-trip"
    assert entry.gid == "993", "passwd gid column should round-trip"
    assert entry.gecos == "", "passwd gecos column should round-trip (empty here)"
    assert entry.home == "/var/lib/repovec", "passwd home column should round-trip"
    assert entry.shell == "/usr/sbin/nologin", (
        "passwd shell column should round-trip"
    )


def test_passwd_parse_strips_trailing_newline() -> None:
    raw = "root:x:0:0:root:/root:/bin/bash\n"

    entry = PasswdEntry.parse(raw)

    assert entry.name == "root", "passwd name should not absorb the newline"
    assert entry.shell == "/bin/bash", (
        "passwd shell should be stripped of the trailing newline"
    )


@pytest.mark.parametrize(
    "broken",
    [
        "",
        "only_one_field",
        "a:b:c:d:e:f",  # 6 fields
        "a:b:c:d:e:f:g:h",  # 8 fields
    ],
)
def test_passwd_parse_rejects_wrong_field_count(broken: str) -> None:
    with pytest.raises(AssertionError, match="unexpected passwd format"):
        PasswdEntry.parse(broken)


# ----- FileStat -----------------------------------------------------------


def test_filestat_parse_returns_expected_columns() -> None:
    raw = "0400\trepovec\trepovec\t64\t1700000000"

    stat = FileStat.parse(raw)

    assert stat.mode == "0400", "stat mode column should round-trip"
    assert stat.user == "repovec", "stat user column should round-trip"
    assert stat.group == "repovec", "stat group column should round-trip"
    assert stat.size == 64, "stat size column should round-trip as int"
    assert stat.mtime == "1700000000", "stat mtime column should round-trip"


def test_filestat_parse_strips_trailing_newline() -> None:
    raw = "0750\troot\trepovec\t4096\t1700000000\n"

    stat = FileStat.parse(raw)

    assert stat.size == 4096, "trailing newline must not break int(size) parsing"


@pytest.mark.parametrize(
    "broken",
    [
        "",
        "0400\trepovec",  # 2 fields
        "0400\trepovec\trepovec\t64",  # 4 fields
        "0400\trepovec\trepovec\t64\t1700000000\textra",  # 6 fields
    ],
)
def test_filestat_parse_rejects_wrong_field_count(broken: str) -> None:
    with pytest.raises(AssertionError, match="unexpected stat format"):
        FileStat.parse(broken)


def test_filestat_parse_raises_value_error_for_non_numeric_size() -> None:
    raw = "0400\trepovec\trepovec\tnot-a-number\t1700000000"

    # ``int(size)`` propagates ``ValueError`` rather than wrapping it in an
    # ``AssertionError``; tests should see the raw failure so a future
    # change to GNU stat's output format surfaces clearly.
    with pytest.raises(ValueError, match="invalid literal for int"):
        FileStat.parse(raw)


def test_filestat_parse_passes_through_blank_user_field() -> None:
    # The script's stat invocation always emits a username (or numeric
    # fallback), but the parser should not assume it is non-empty.
    raw = "0644\t\trepovec\t64\t1700000000"

    stat = FileStat.parse(raw)

    assert stat.user == "", "empty user column must be preserved verbatim"
    assert stat.group == "repovec", (
        "subsequent columns must still parse when an earlier column is empty"
    )
