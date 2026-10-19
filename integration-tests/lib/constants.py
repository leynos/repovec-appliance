"""Shared constants describing the provisioning helper's contract.

These values must stay in sync with ``packaging/libexec/repovec-qdrant-api-key``
and ``packaging/sysusers.d/repovec.conf``. They are duplicated here so tests
can assert against the contract without parsing the shell helper.
"""

from __future__ import annotations

from pathlib import PurePosixPath
from typing import Final

HELPER_SCRIPT: Final = "/usr/libexec/repovec/repovec-qdrant-api-key"
KEY_FILE: Final = "/etc/repovec/qdrant-api-key"
SECRET_NAME: Final = "repovec-qdrant-api-key"

REPOVEC_USER: Final = "repovec"
REPOVEC_GROUP: Final = "repovec"
REPOVEC_HOME: Final = "/var/lib/repovec"
REPOVEC_SHELL: Final = "/usr/sbin/nologin"
REPOVEC_ETC_DIR: Final = "/etc/repovec"

KEY_FILE_MODE: Final = "0400"
ETC_DIR_MODE: Final = "0750"
KEY_HEX_LENGTH: Final = 64

# Directory-layout contract (packaging/tmpfiles.d/repovec.conf). These must
# stay in sync with that asset and with the RuntimePaths-derived spec table in
# crates/repovec-core/src/appliance/directory_layout. Only the DATA_DIRS
# entries are owned `repovec:repovec` with mode `0700` (SI-1); the
# QDRANT_DATA_DIR child is owned `root:root` with mode `0700` (SI-2), which is
# why it is listed separately below rather than as a DATA_DIRS member.
ASSUMED_TMPFILES_SOURCE: Final = "/usr/lib/tmpfiles.d/repovec.conf"
ASSUMED_SYSUSERS_SOURCE: Final = "/usr/lib/sysusers.d/repovec.conf"
DATA_ROOT: Final = "/var/lib/repovec"
DATA_DIR_MODE: Final = "0700"
_DATA_ROOT_PATH: Final = PurePosixPath(DATA_ROOT)
DATA_DIRS: Final = (
    DATA_ROOT,
    str(_DATA_ROOT_PATH / "git-mirrors"),
    str(_DATA_ROOT_PATH / "worktrees"),
    str(_DATA_ROOT_PATH / ".grepai"),
)
QDRANT_DATA_DIR: Final = str(_DATA_ROOT_PATH / "qdrant-storage")
QDRANT_DATA_DIR_MODE: Final = "0700"
