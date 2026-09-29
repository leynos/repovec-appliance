"""Drive the strict workflow loader with documents the repository lacks.

Every workflow here is a mapping, so a loader that read a scalar or list
root as an empty workflow would pass every file-driven contract. These cases
build the roots directly.

Run via ``make test-workflow-contracts``.
"""

from __future__ import annotations

from pathlib import Path

import pytest
from workflow_loader import NotAMappingError, load_workflow, read_workflows


@pytest.mark.parametrize(
    "text",
    ["just a scalar\n", "", "~\n", "- push\n- pull_request\n"],
    ids=["scalar", "empty", "null", "list"],
)
def test_a_root_that_is_not_a_mapping_is_refused(text: str) -> None:
    """A scalar, empty, null or list root is refused, not read as empty.

    Read as empty, the file would declare no triggers, drop out of every
    trigger-scoped contract and pass them all.
    """
    with pytest.raises(NotAMappingError, match="root must be a mapping"):
        load_workflow(text)


def test_a_non_mapping_workflow_file_fails_the_directory_read(
    tmp_path: Path,
) -> None:
    """One invalid file beside a valid one fails the read of the directory."""
    (tmp_path / "ci.yml").write_text("on: pull_request\n", encoding="utf-8")
    (tmp_path / "broken.yml").write_text("- pull_request\n", encoding="utf-8")
    with pytest.raises(NotAMappingError):
        read_workflows(tmp_path)
