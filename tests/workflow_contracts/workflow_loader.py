"""Strict loading for every workflow contract in this directory.

The pull-request cancel contract loads the tree through here, and any other
contract that moves here shares the same refusal. PyYAML's ``safe_load`` keeps the last value of a duplicated
mapping key and says nothing, so a lane declaring ``runs-on`` twice parses
into a document that has silently discarded half of what GitHub was asked to
run; the loader here refuses the document instead. It also reads both
spellings of the extension, compared case-insensitively, because GitHub runs
``CI.YML`` as readily as ``ci.yml``.

Use ``repository_workflows`` for this repository's tree, ``read_workflows``
for any other directory and ``load_workflow`` for one document. Parse
workflow files through nothing else in this directory.
"""

from __future__ import annotations

import functools
import typing as typ
from pathlib import Path

import yaml

#: Both spellings GitHub accepts for a workflow file's extension, compared
#: lowercased so that ``CI.YML`` is not skipped in silence.
WORKFLOW_SUFFIXES: typ.Final = frozenset({".yml", ".yaml"})

#: This repository's workflow directory.
WORKFLOW_DIR: typ.Final = (
    Path(__file__).resolve().parents[2] / ".github" / "workflows"
)

Document = dict[object, object]


class DuplicateKeyError(yaml.constructor.ConstructorError):
    """A mapping declared the same key twice."""


class NotAMappingError(yaml.YAMLError):
    """A workflow document's root is not a mapping.

    A scalar, list or empty root would otherwise read as a workflow with no
    triggers, drop out of every trigger-scoped contract, and pass them all.
    """


class StrictLoader(yaml.SafeLoader):
    """A ``SafeLoader`` that refuses duplicate mapping keys.

    Examples
    --------
    >>> yaml.load("a: 1\\nb: 2\\n", Loader=StrictLoader)
    {'a': 1, 'b': 2}
    """

    def construct_mapping(
        self, node: yaml.MappingNode, deep: bool = False
    ) -> dict[object, object]:
        """Build a mapping, refusing a key that appears twice."""
        seen: set[object] = set()
        for key_node, _ in node.value:
            key = self.construct_object(key_node, deep=deep)
            if key in seen:
                raise DuplicateKeyError(
                    "while constructing a mapping",
                    node.start_mark,
                    f"found duplicate key {key!r}",
                    key_node.start_mark,
                )
            seen.add(key)
        return super().construct_mapping(node, deep=deep)


def load_workflow(text: str) -> Document:
    """Parse one workflow strictly.

    Parameters
    ----------
    text
        The workflow's YAML source.

    Returns
    -------
    Document
        The parsed document.

    Raises
    ------
    DuplicateKeyError
        When any mapping in the document declares a key twice.
    NotAMappingError
        When the document's root is a scalar, a list or empty.

    Examples
    --------
    >>> load_workflow("on: push\\njobs: {}\\n")
    {True: 'push', 'jobs': {}}
    """
    document = yaml.load(text, Loader=StrictLoader)
    if not isinstance(document, dict):
        message = f"a workflow's root must be a mapping, found {type(document).__name__}"
        raise NotAMappingError(message)
    return document


def read_workflows(directory: Path) -> dict[str, Document]:
    """Parse every workflow in a directory, keyed by file name.

    Parameters
    ----------
    directory
        The directory holding the workflow files, usually
        ``.github/workflows``.

    Returns
    -------
    dict[str, Document]
        Each ``.yml`` or ``.yaml`` file's parsed document, keyed by file name
        and in file-name order; the extension is matched case-insensitively.

    Raises
    ------
    DuplicateKeyError
        When any workflow declares a mapping key twice.
    """
    paths = sorted(
        path
        for path in directory.iterdir()
        if path.is_file() and path.suffix.lower() in WORKFLOW_SUFFIXES
    )
    return {
        path.name: load_workflow(path.read_text(encoding="utf-8"))
        for path in paths
    }


@functools.cache
def repository_workflows() -> dict[str, Document]:
    """Read and parse this repository's workflows once.

    Returns
    -------
    dict[str, Document]
        Every workflow under ``.github/workflows``, keyed by file name. The
        mapping is cached, so callers must not mutate it.

    Raises
    ------
    DuplicateKeyError
        When any workflow declares a mapping key twice.
    """
    documents = read_workflows(WORKFLOW_DIR)
    assert documents, "the repository should define at least one workflow"
    return documents
