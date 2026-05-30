"""Shared constants describing the provisioning helper's contract.

These values must stay in sync with ``packaging/libexec/repovec-qdrant-api-key``
and ``packaging/sysusers.d/repovec.conf``. They are duplicated here so tests
can assert against the contract without parsing the shell helper.
"""

from __future__ import annotations

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
