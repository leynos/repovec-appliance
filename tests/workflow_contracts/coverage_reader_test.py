"""Prove the coverage reader refuses what it cannot judge.

Every coverage rule below is a refusal over a subject set. A reader that finds
no workflow, skips a spelling GitHub runs, or keeps half of a duplicated
mapping would leave a document outside every assertion while the suite stayed
green. Those failures are the reader's to raise, and they are proved here
rather than inferred from a rule that happens to pass.
"""

from __future__ import annotations

import typing as typ
from pathlib import Path

import pytest
from coverage_workflow_reader import WorkflowError, read_documents, triggers
from workflow_loader import DuplicateKeyError, NotAMappingError, load_workflow


def test_reader_refuses_an_empty_directory(tmp_path: Path) -> None:
    """Finding no workflow is the reader failing, not the repository passing."""
    with pytest.raises(WorkflowError, match="no workflows were read"):
        read_documents(tmp_path)


def test_reader_refuses_a_missing_directory(tmp_path: Path) -> None:
    """A directory that cannot be listed fails at the reader, naming it."""
    with pytest.raises(WorkflowError, match="cannot list workflows"):
        read_documents(tmp_path / "missing")


def test_reader_refuses_a_file_that_is_not_utf8(tmp_path: Path) -> None:
    """Undecodable bytes must not escape as a traceback from somewhere else."""
    (tmp_path / "bad.yml").write_bytes(b"\xff\xfe")
    with pytest.raises(WorkflowError, match="cannot be read as UTF-8"):
        read_documents(tmp_path)


def test_reader_refuses_a_duplicated_mapping_key(tmp_path: Path) -> None:
    """PyYAML would keep the second value and discard the first in silence."""
    (tmp_path / "dup.yml").write_text(
        "on: push\njobs:\n  a:\n    runs-on: x\n    runs-on: y\n    steps: []\n",
        encoding="utf-8",
    )
    with pytest.raises(WorkflowError, match="cannot be read strictly"):
        read_documents(tmp_path)


def test_loader_names_the_duplicated_key() -> None:
    """A duplicate is refused at the loader with the key it repeated."""
    text = "on: push\njobs:\n  a:\n    runs-on: x\n    runs-on: y\n    steps: []\n"
    with pytest.raises(DuplicateKeyError, match="duplicate key 'runs-on'"):
        load_workflow(text)


@pytest.mark.parametrize(
    "text",
    ["[]", "", "a scalar"],
)
def test_loader_refuses_a_non_mapping_root(text: str) -> None:
    """A root without jobs reads as a workflow with no triggers and passes."""
    with pytest.raises(NotAMappingError, match="root must be a mapping"):
        load_workflow(text)


def test_reader_reads_every_suffix_and_case(tmp_path: Path) -> None:
    """GitHub runs ``.yaml`` and upper-case suffixes as readily as ``.yml``."""
    for name in ("a.YML", "b.yaml"):
        (tmp_path / name).write_text("on: push\njobs: {}\n", encoding="utf-8")
    found = sorted(read_documents(tmp_path))
    assert found == ["a.YML", "b.yaml"], f"unexpected workflows read: {found}"


@pytest.mark.parametrize(
    "text",
    ["on: push\njobs: {}\n", "'on': {pull_request: {}}\njobs: {}\n"],
)
def test_triggers_reads_both_spellings(text: str) -> None:
    """YAML 1.1 reads a bare ``on`` as true and a quoted one as text."""
    events = triggers("probe.yml", load_workflow(text))
    assert events, f"no event was read from {text!r}"


def test_triggers_refuses_both_spellings_together() -> None:
    """GitHub merges the spellings; a reader of one would be blind to the other."""
    text = "on: push\n'on': pull_request\njobs: {}\n"
    with pytest.raises(WorkflowError, match="declares `on` 2 times"):
        triggers("both.yml", typ.cast("dict[object, object]", load_workflow(text)))
